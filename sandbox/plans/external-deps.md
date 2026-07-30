# External dependencies as sealed leaves — design

Status: **direction decided 2026-07-10; implementation started 2026-07-26.**
Slice 1 (foundational) landed: `ContentDigest::sealed` (keys.rs, domain-tagged,
`SCHEMA_VERSION` 4→5), and a `deps.rs` module with `LeafSource`/`SealedCoord`,
a JSON `Lockfile`, XDG store-path resolution, and a *temporary* hash-at-lock.
Not yet wired into the parse leaf or invalidation — see "Sketch of the work".
`deps` is still a private module referenced by nothing but its own unit tests.

Of the remaining slices, **only Slice 2 gates the swap**; Slices 3–4 are
deferable optimizations (see "Slicing and granularity decisions" below).

An optimization
for external (published, vendored) dependencies: because their source is
**immutable by contract**, they escape the invalidation machinery entirely — no
per-compile read+hash, no watcher, no dirty bits — and their entries become
shareable across projects.

Related: [keys and fingerprints](../../doc/book/src/19a-compiler-internals/03-keys-and-fingerprints.md) (the
content-addressed leaf model this specializes),
[concurrency-and-eviction.md](concurrency-and-eviction.md) (sealed entries are
prime eviction/sharing candidates), [daemon.md](daemon.md) (a shared tier
across roots). Roadmap Phase 3.

---

## The distinction: mutable leaves vs sealed leaves

A **local** source file is a *mutable* leaf. The engine cannot trust it between
compiles, so it re-reads and re-hashes it every batch compile to derive the
lookup digest (or, in the watch stance, trusts a clean cone only because the
watcher promised to report changes), registers it with the file monitor, and
carries dirty bits + reverse edges so an edit can mark its cone.

An **external** dependency is a *sealed* leaf: **immutable by contract.** A
published release's bytes never change; a new version is a *different*
dependency, not a mutation of the old one. That single property lets a sealed
leaf skip the whole apparatus:

- **No per-compile read+hash** — its content digest is a fixed coordinate, not
  something re-derived from the filesystem each run.
- **No watcher registration** — it cannot change, so nothing to watch (and
  inotify has an OS ceiling that vendored trees blow through).
- **No dirty bit / reverse edges** — it is permanently clean; it is never in a
  marking cone.
- **Entries valid forever and across projects** (see sharing, below).

## Key on the release hash, not bare semver

`semver` alone is a *claim*, not integrity: a registry can re-publish a version
with different bytes (supply-chain risk), so trusting `1.4.2` to mean fixed
bytes would break the content-addressed invariant. Key instead on the
**lockfile-pinned release hash** (the `Cargo.lock`-style checksum), with semver
as human-facing metadata.

This keeps the invariant *exactly* intact — the key is still a hash of immutable
bytes. The optimization is only about *when* the hash is taken: **once, at
fetch/lock time (recorded in the lockfile), instead of re-deriving it from the
filesystem every compile.** A sealed leaf's content key is built from the
recorded coordinate hash directly; the artifact is never read to compute a
digest.

(The answer is still keyed with `SCHEMA_VERSION` like every content key: an
immutable *source* whose *answer format* changed under a compiler bump correctly
re-keys. Sealed means the source is frozen, not that the compiler is.)

## Provenance: what is allowed to be sealed

Only a step whose **entire transitive leaf cone is sealed** may skip
invalidation. A dependency that also reads a local config file, or is
monomorphised at a *local* type, mixes provenance and must follow the normal
tracked path.

So every step carries a cheap **provenance bit**:

```
sealed(step) = AND over inputs of sealed(input)
```

Leaves set it directly (`Sealed` coordinate = true, `Local` file = false);
derived steps compute it as the AND of their inputs. Only `sealed == true`
steps are exempted from dirty tracking, reverse-edge bookkeeping, and re-read.
A mixed cone behaves exactly as today.

## The lockfile is the tracked leaf that gates sealing

Sealed leaves are immutable, but *which* sealed coordinates are in play is not —
a dependency bump changes it. That change flows through the **lockfile**, which
is an ordinary **local, tracked** leaf:

- Editing the lockfile (bumping a dep) is a normal tracked-file edit. It yields
  a *different* coordinate → a *different* content key for the affected sealed
  cone. Nothing is "invalidated"; the old key is simply no longer demanded and
  the new key is a cold (or shared-disk-warm) lookup.
- So "an external dependency changed" is handled entirely by the one file we
  *do* watch and hash, while the dependency artifacts themselves stay frozen and
  untouched.

## Cross-project sharing

Because a sealed leaf's key is `hash(coordinate hash, SCHEMA_VERSION, …)` with
no project-local ingredient, **`dep@hash` produces the same content key in any
workspace.** Therefore:

- The disk tier can be **shared across roots** — a sealed entry computed for one
  project serves every other project on the machine (and warm CI starts). This
  is the first real motivation for a cache tier *above* the per-root
  `<root>/out/cache` (a per-user or global sealed store); the daemon (one per
  root) would read a shared sealed tier and write through to it.
- Sealed entries are the **best memory-eviction candidates** in
  [concurrency-and-eviction.md](concurrency-and-eviction.md): re-load is a pure,
  shared disk hit and they can never be stale — so they can be dropped from a
  process's hot memory the most aggressively.

## Not the same as "pluggable source backends"

The dropped "pluggable source backends" item was about *where bytes come from*
(disk vs in-memory / web-IDE buffer). This is about the *immutability contract*
that lets us skip invalidation — orthogonal, and independently valuable.

## Sketch of the work

- **Leaf classification** — a `LeafSource` split (`Local` vs
  `Sealed(SealedCoord)`); the parse key builder takes the digest from the
  coordinate for sealed leaves instead of reading+hashing. **[Slice 1, done]**
  `SealedCoord = (release_hash, path_within_package, semver)`;
  `ContentDigest::sealed` folds `(release_hash, path)`, domain-tagged apart from
  the local `of()` digest (leading `0` vs `1` byte) so a sealed coordinate can
  never alias a local file. Cost a `SCHEMA_VERSION` bump (4→5, cold cache).
- **Provenance bit** — one bit threaded through the graph/binding record;
  leaves seed it, derived steps AND it; sealed steps are excluded from the
  marking pass and never registered with the monitor. **[Slice 3]**
- **Lockfile leaf** — parse the lockfile (a tracked local leaf) into the set of
  `(package → coordinate_hash)`; resolving an external import maps to a sealed
  leaf via this table. **[Slice 1 partial / Slice 2 wiring]** Format is
  versioned JSON (`{version, packages: {name: {hash, version}}}`, `serde_json`,
  no new dep); `Lockfile::coord(pkg, path)` returns `Some(SealedCoord)` for a
  locked package, `None` (→ not sealed) otherwise. Wiring the resolver's
  import→coordinate lookup and the parse leaf's digest-from-coordinate is
  Slice 2, under the import form decided below.
- **Shared sealed disk tier** (later) — a read-through tier above the per-root
  cache, keyed purely on sealed content keys, safe to share across roots by
  content-addressing. **[Slice 4]**

### Storage & hashing decisions (2026-07-26)

- **Dep source store**: `$XDG_CACHE_HOME/tel/deps/<release-hash>/<path>`
  (default `~/.cache/tel/deps/…`), honoring `XDG_CACHE_HOME`. Chosen to match Go
  (`~/.cache/go-build`) and Deno (`~/.cache/deno`): everything here is
  reconstructible, which is what the XDG cache dir is for. If dep sources ever
  become authoritative (no re-fetch path), promote *just the source store* to
  `$XDG_DATA_HOME/tel/store` (Nix/pnpm-style); the answer tier stays in cache.
  Only a cold-miss parse reads this store — a warm run never touches it.
- **Release hash**: **temporary** hash-at-lock (`deps::lock_package`) — hashes a
  package's source tree deterministically (sorted, length-prefixed `(path,
  bytes)`, xxh3-128 → hex). Stands in for a registry checksum until a real fetch
  story exists; explicitly a placeholder.

### Slicing and granularity decisions (2026-07-30)

- **Only Slice 2 gates the swap.** Slices 3 (provenance bit) and 4 (shared
  sealed tier) move to *deferable*, alongside the Phase-4 performance items in
  plans/swap-to-real-language.md §4. Neither changes semantics: without the
  provenance bit a sealed leaf is merely treated like a local one (re-read,
  re-hashed, watched — slower, not wrong), and the shared tier has no customer
  until a machine hosts more than one Tel project. Slice 3's value is entirely
  about scale (inotify ceilings, vendored trees), which the toy suite cannot
  exercise — so "cheaper against the toy language", the reason 13b was a gate,
  does not hold for it.
- **The lockfile enters the graph per dependency, not wholesale.** Two kinds,
  both ordered below `Resolve`:
  - `Lock` — a leaf: read+hash the lockfile once, answer is the parsed table.
  - `LockCoord(package)` — a projection over `Lock`, answer is that one
    package's coordinate.

  A resolve step depends only on the `LockCoord`s of the packages it imports.
  Bumping dep A leaves `LockCoord(B)`'s fingerprint unchanged, so B's importers
  cut off early instead of re-resolving. The coarse alternative (depend on
  `Lock` directly) was rejected: **one lockfile is shared by several entry
  points**, so its fan-out is wide rather than rare, and a one-package bump
  would re-resolve every root. Making `LockCoord` the leaf itself — skipping
  `Lock` — was also rejected: the batch stance re-reads every demanded leaf, so
  that is one read+hash of the same file *per package* per compile.
- **Release hash stays the hash-at-lock placeholder.** There is no registry,
  fetch path, or ecosystem to provide a real checksum, and inventing a package
  manager to unblock a compiler refactor is the wrong order. `lock_package` is a
  sound *pin*; what it lacks is *provenance* (a publisher-signed checksum). Two
  hooks keep the replacement cheap: `Lockfile::version` must actually reject
  formats this build cannot read (today it is parsed and then `#[allow(dead_code)]`),
  and the lockfile should record which hash algorithm produced the digest, so a
  registry checksum later is not a format break.

### Import form decision (2026-07-30)

An import must **spell an absolute path** — that is the actual surface syntax
for now, not sugar over a search. It replaces the sandbox's bare-name sibling
convention (`(import module_name)` → sibling `module_name.telsb`, `language.md`
"Imports"), which has to change with Slice 2. It may be extended later (a real
package/coordinate syntax is still open); until then there is nothing else.

The point is that **an import names exactly one candidate by construction**, so
there is no search path, no precedence, and no shadowing rule to get wrong:

- No local-sibling-vs-lockfile ordering, in either direction. Precedence was
  considered and **rejected** — it decides a language question silently, and
  whichever side is picked becomes precedent before the real import syntax is
  designed.
- **Duplicates are a hard error**, not a resolved ambiguity: two imports naming
  the same file, or one path claimed by both a local file and a locked package.
  Uniqueness is *enforced*, not assumed (earlier drafts of this plan assumed
  "file paths are unique" — this is what replaces that assumption).

**Consequence for fixtures — open.** A filesystem-absolute path cannot be
committed: `examples/**.telsb` has ~41 import lines whose repo path differs per
machine, and the Rust tests write sources into a `TempDir` whose path is not
known until runtime. Runtime-generated fixtures are fine (the absolute path is
formatted in). The committed corpus needs one of:

1. **root-anchored resolution** — an import path is absolute *within the project
   root* (and within a package root for sealed leaves), which is the only
   absolute form that survives being committed and still has no search path; or
2. **a harness rewrite** — commit a `@ROOT@`-style placeholder that the example
   runner substitutes before compiling.

(1) is the recommendation: it keeps fixtures portable and the corpus
Rust-agnostic (plans/swap-to-real-language.md §5), where (2) makes every
non-Rust backend reimplement the substitution. Not yet decided.

## Invariants preserved

- Content-addressing holds: sealed keys are still hashes of immutable bytes
  (taken from the lockfile), never mtimes or bare version strings.
- A mixed (local + sealed) cone is treated exactly as today — sealing is a
  conservative AND, never assumed.
- Compiler-version safety is unchanged: `SCHEMA_VERSION` still keys sealed
  answers, so a format change re-keys them like any other.
</content>
