# Package Registry

TODO: review

The registry (or *package index*) is the named, networked source from which
crates are fetched. Tel's stability commitment makes the registry's
*retention* and *reproducibility* rules part of the design, not an operational
detail. Tel does **not** intend to design a bespoke index — the goal is to reuse
an existing open-source one that meets the requirements below.

## What

A registry maps `<namespace>/<name>@<version>` to an immutable, content-addressed
source artifact (a zip of the crate directory — see
[Crates](04-packages.md#packaging-format-a-zip-of-a-directory)) plus its
manifest.

## Requirements

The properties an off-the-shelf index must satisfy, in brief:

- **Immutable versions.** Once published, a version's bytes never change.
  Re-publishing the same version is an error.
- **Yank, never delete.** A version can be *retired* with a reason
  (security / broken / wrong-results) so the resolver stops selecting it, but it
  **stays fetchable** so old lockfiles still resolve. Build warns, does not break.
- **Source-only.** The artifact is the source tree; no compiled binaries, no
  compile farm. "Release matches source" is then a tautology, not a check.
- **No install-time / publish-time code execution.** A crate is data the
  compiler reads, never a program the index runs (no `postinstall`, no
  `build.rs`). See [antifeatures](../02-philosophy/04-antifeatures.md).
- **Content-addressed, verified against the lockfile.** Each version has a hash
  of its *canonicalised crate tree* (not the zip bytes), pinned in the lockfile
  and checked on every fetch/mirror.
- **Takedown-resistant — the index keeps its own bytes.** Authors must not be
  able to make a published version *disappear* (force-pushed tag, deleted repo,
  repo made private). The index **forks and stores every published version's
  source** in an append-only store, independent of the upstream VCS.
- **Namespaced.** Crates live under `<namespace>/<name>` (see below).
- **Resolver-enabling, not resolver-deciding.** The index exposes *all* versions
  and *each version's declared dependency ranges*; it must not bake in a single
  resolved version. Resolution is the
  [package manager's](../18-tooling/04-package-manager.md) job (one version per
  [connected component](06-versioning.md#decided-one-version-per-connected-component)).
- **Mirrorable / self-hostable / offline.** An org can review and host its own
  artifacts; CI builds from a vendored/cached set. Content-addressing makes any
  mirror serving the same bytes resolve identically.
- **Carries Tel metadata faithfully.** Per-dependency
  [capability declarations](04-packages.md#per-dependency-capability-declarations)
  and re-export flags travel with the crate so the package manager can diff
  capability requests across versions. The index need not *understand* this
  metadata — carrying it opaquely inside the source tree is enough.
- **Bounded artifact size.** The index enforces a per-version cap on the
  uncompressed tree — **10 MB, warn at 7 MB, increase on request** — to keep
  crates reasonable and hosting manageable. See
  [Size limits](04-packages.md#size-limits).

## Why

A script written today should still build a decade from now —
[stability is priority #1](../02-philosophy/01-priorities.md). A registry that
mutates, forgets, or lets authors withdraw versions breaks that promise. The same
reasoning that freezes the language makes the registry rules conservative.

The registry is *not* where the host application is distributed — it is for
Tel-to-Tel reuse only. The host owns its own deployment.

## Hosting model

The instinct *"GitHub-hosting is nice, but releases must be immutable, so hash
the whole tree and pin the hash in the index"* is essentially the Go-module
model. Tel adopts that shape, with one addition forced by takedown-resistance:

- A **thin index** maps `<ns>/<name>@<version> → {source ref, tree hash,
  manifest}`. It can be as small as a git repo (free history, free mirroring).
- **Publishing** pulls bytes from a VCS tag (nice UX, no upload tooling), but at
  publish time the index **copies the source into its own append-only,
  content-addressed store** keyed by the tree hash. From then on the upstream VCS
  is irrelevant to resolution: a changed tag is detected by hash mismatch, a
  deleted repo is survived because the bytes are ours.
- The store is source-only, deduplicated, never deleted — cheap to run forever.

A *pure* "point at GitHub" model is therefore ruled out (an author can delete the
repo); the byte store is **mandatory**, not optional.

TODO(open): a takedown/exemption policy for genuine legal (DMCA) cases — Go has
one. "The author changed their mind" is explicitly **not** grounds for removal.

## Crate identity: the tree hash

A version's identity is a **tree hash** over its canonicalised source tree,
computed in the **Go `h1:` shape** — deliberately boring and easy to reimplement:

1. For each file, compute `sha256(file bytes)`.
2. Build an index of `"<hex-hash>  <path>\n"` lines and **sort** by path.
3. The tree hash is `sha256(sorted index)`.

Go ships exactly this as `golang.org/x/mod/sumdb/dirhash`, so it is reusable off
the shelf if our tooling is in Go and a few dozen lines otherwise. (Nix's NAR
hash is the alternative but encodes file modes and symlinks we do not want.)

What is canonicalised is the **tree structure**, not file *contents*:

- **Paths:** UTF-8, NFC-normalised, `/` separators, sorted; case is significant.
- **Excluded:** file modes, timestamps, owner/group, archive ordering — none
  affect identity (a source tree needs no executable bit).
- **Symlinks:** forbidden in a published crate (avoids tree escapes and a
  serialization corner case).
- **Contents hashed verbatim.** This is the one subtlety, and the reason *not* to
  follow the tempting instinct of normalising line endings/encoding *inside* the
  hash: rewriting bytes at hash time would corrupt static/binary assets and make
  "what we hash" differ from "what we serve." Text canonicalisation (UTF-8, LF,
  final newline, no BOM) is instead enforced at publish time by the
  [formatter](../18-tooling/06-formatter.md), so source bytes are already
  canonical *before* they are hashed; binary assets are hashed as-is.

This keeps the identity function total and trivial while still giving
cross-platform-stable hashes.

## Index transport: sparse and on-demand

The index is **not** shipped as one downloadable file — a git checkout of the
whole registry does not scale. Following Cargo's **sparse-index** move:

- The resolver fetches **per-crate metadata on demand**: `GET
  /index/<ns>/<name>` returns that crate's versions, each version's declared
  dependency ranges, tree hash, and yank status — then **caches it client-side**
  with conditional requests (ETag) for cheap freshness. The resolver only pulls
  metadata for crates it actually considers.
- **Search / discovery is a separate service**, not part of resolution — you
  never need the whole index to resolve a build, only to browse.
- **Source bytes** come from the content-addressed store keyed by tree hash;
  metadata responses are small and compressible, artifacts immutable and
  cacheable forever.

Plain cacheable **HTTPS** (HTTP/2 or /3) is the transport — simpler to serve,
mirror, and cache than a git index, with no diffing to reason about. The one
thing a git index gave for free, tamper-evident history, is deferred along with
the [transparency log](#deferred-post-10); signed index responses can supply it
later.

## Namespaces, scarcity, and ownership

Crates are namespaced (`<namespace>/<name>`) to avoid the flat-namespace
land-rush that crates.io and PyPI regret. Two problems follow: a namespace anyone
can mint for free just moves squatting up one level, and namespaces need real
account security (2FA, recovery, shared/team ownership). Tel solves both by **not
building identity itself**.

**Namespaces are flat; names within one form a prefix-antichain.** A namespace
does not nest (no `acme.pricing` under `acme`), and within a namespace no
published crate name may be a prefix of another — a dotted name is either a
*leaf* (a real crate) or a *grouping prefix*, never both, so `user.auth` and
`user.auth.google` cannot both be published. This is what keeps a
[dotted import root unambiguous](02-imports.md#how-a-dotted-root-stays-unambiguous):
one crate can never grow into another's import path. The registry checks it at
publish time; because a namespace is owner-scoped, the check is local to one
owner.

**Reuse an identity provider — do not roll our own auth.** Delegate identity to
an OAuth2/OIDC provider: a **VCS host** (GitHub/GitLab — the lean) or a
self-hosted OSS identity server (Keycloak, Zitadel, Ory). That supplies off the
shelf the whole list we would otherwise have to build *and secure*:

- **2FA, account recovery, session management** — the IdP's job, not ours.
- **Shared / team ownership** — an org with roles *is* shared ownership; reuse
  org membership instead of inventing a permissions model.
- **Multi-namespace** — one identity can own several orgs/scopes.

For the **publish** path, adopt **OIDC trusted publishing** (the PyPI/npm 2023+
model): CI publishes with a short-lived OIDC token, so there are no long-lived
registry credentials to leak.

**Scarcity** (discouraging claims on namespaces you will not use):

- **Identity-backed claims (primary):** a namespace must match an org/account you
  control on the IdP. Sybil cost is inherited from the host — creating many
  GitHub orgs is rate-limited and identity-bound — at zero cost to us. The cheap
  "good enough" scarcity.
- **Use-it-or-lose-it (secondary):** a namespace that has **published nothing**
  after a grace period is reclaimable automatically, after notice. Targets
  hoarding directly.
- *Optional later:* a domain-proof tier (Maven-style) for high-assurance vanity
  roots; economic deposits. Avoid payment infrastructure for v1.

**Ownership lifecycle:**

- **Transfer / shared ownership** ride the IdP — transfer the backing org, or
  add/remove publishers via org roles. Require more than one owner (or a cool-off)
  so a single compromised account cannot seize or empty a namespace.
- **Reclaim is restricted by a hard safety rule:** **once a namespace has
  published any version, its names are effectively permanent.** Only
  *never-published* namespaces are garbage-collected. A reclaimed name must
  **never** let a new owner publish a *new* version under an existing crate's
  identity — that would be a republish / supply-chain vector and break the
  immutability and takedown-resistance guarantees above. Disputes over published
  names (trademark, impersonation) go through a **manual** process, never
  automatic reclamation.

**Reserved namespaces.** A small set is reserved up front and never claimable
through the normal flow:

- `tel`, `core`, `std` — reserved for the language and standard library.
- `apivolve`, `mverleg` — reserved for the project's author.

Reserving the standard-library roots also forecloses a confusion/typosquat
vector (a third party publishing under `std`).

## Reusing an existing index

No off-the-shelf index fits perfectly, but **Hex** (the Erlang/Elixir index) is
the closest and is the current lean. It already provides immutable versions,
retire-with-reasons, checksums, its own byte storage, a first-class mirror
protocol, and — crucially — an **open-source, self-hostable server**, which the
mirror/offline requirement makes mandatory. Hex is not BEAM-locked in practice:
Gleam, a separate language, already reuses it.

Two gaps and how they close:

- **Tel-specific metadata** (capabilities, Tel-as-data manifest) rides *inside*
  the source tree, which Hex stores opaquely — so the wire protocol needs no
  change.
- **Namespaces.** Hex's public registry is flat, but its
  *organizations/repositories* mechanism already models a namespace axis; turning
  that into free, identity-backed public scopes is a server-side policy change,
  not a protocol redesign.

Patterns worth stealing regardless of which index we adopt: Go's
`retract`-in-source yank (a yank is declared by a later version, so the index
never mutates), Go's `h1:` tree hash, Hex's retire-reason taxonomy, JSR's scope
shape.

### Building it ourselves (a Tel showcase)

Implementing the thin index in Tel is an attractive **demo** of the language —
the index is mostly a content-addressed store plus a small publish/verify path,
and all the genuinely hard logic (per-component resolution, capability diffing)
lives in the package manager, which we build anyway. If we go this route, the one
piece we **must** specify ourselves regardless of tooling is the **tree-hash
canonicalisation** (it defines crate identity): copy Go's `h1:` or Nix's NAR.

We would **not** roll our own **authentication** — reuse an open-source auth
system (e.g. an OAuth/OIDC provider, or delegate identity to the VCS host the
namespaces are already tied to). Auth is exactly the kind of security-sensitive
wheel not worth reinventing.

## Open questions

- TODO(open): adopt Hex (and bend it) vs. build the thin index in Tel. Lean: Hex
  for speed; the Tel build is a tempting showcase if effort allows.
- TODO(open): yank/retire reason taxonomy — adopt Hex's set
  (`security`/`invalid`/`deprecated`)?
- TODO(open): how the index interacts with capability declarations — does it
  surface a warning when a dependency starts requesting new capabilities between
  versions, or is that purely the package manager's job? (Lean: package manager.)
- TODO(open): which OIDC/identity provider to require or bundle (delegate to a
  VCS host vs. self-host Keycloak/Zitadel/Ory), and the exact grace period for
  use-it-or-lose-it namespace reclamation.

## Deferred (post-1.0)

Two heavier registry features — **binary mirrors** and a **stronger
supply-chain layer** (signing, attestations, build provenance, a transparency
log, vulnerability auditing) — are **deferred**. The committed 1.0 baseline
(immutable versions, content-addressed artifacts verified against the lockfile,
source-only distribution, per-dependency capability declarations) is enough, and
most scripts lean on `std` rather than a large third-party surface. The rationale
lives in
[Deferred Features → Advanced package-registry features](../20-appendix/06-deferred-features.md#advanced-package-registry-features).

## See also

- [Crates](04-packages.md) — what is being published.
- [Package Manifest](07-package-manifest.md) — what each published artifact
  carries with it.
- [Versioning](06-versioning.md) — one version per connected component, and how
  cross-version types stay distinct.
- [Dependency Graph and Locking](08-dependency-graph-and-locking.md) — how
  resolved versions are recorded for reproducibility.
- [Package Manager](../18-tooling/04-package-manager.md) — the tool that talks to
  the registry.
