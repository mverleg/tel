# Compilation Pipeline (notes)

Scratchpad for how the incremental compiler is actually built. User-facing
material lives in [`../18-tooling/01-compiler.md`](../18-tooling/01-compiler.md)
(the query-compiler / content-addressed-cache commitment) and
[`../11-modules-and-packages/08-dependency-graph-and-locking.md`](../11-modules-and-packages/08-dependency-graph-and-locking.md)
(the lockfile). Do not link to this file from chapters.

A working prototype of everything below lives in the repo under
`tel/tel/sandbox` (a minimal Lisp-ish language exercising the query engine);
the design write-up is `tel/tel/sandbox/plans/external-deps.md`.

## The content-addressed query model (recap)

Every step — "parse file F", "resolve F", "type-check function G" — is a query
with a **content key** = `hash(schema, kind, stable args, answer fingerprints
of direct deps)`. The cache maps content keys → answers; a hit is valid *by
construction* (any transitive change would change the key). No invalidation
logic in the hit path, and answers are shareable across runs and machines. This
is the ch-18.1 commitment; the notes here are about the **leaf** of that Merkle
DAG, where the recursion bottoms out in external input.

## Mutable leaves vs sealed leaves

The leaf query (parse) is where the cache reads *external* state. There are two
kinds, and the distinction is the whole optimization:

- **Local source file — a *mutable* leaf.** The engine cannot trust it between
  compiles, so every compile it re-reads the bytes and hashes them to derive
  the parse key's digest, registers the path with the file watcher, and carries
  dirty bits + reverse edges so an edit can mark its cone.
- **External dependency file — a *sealed* leaf.** A published version's bytes
  never change (registry contract, ch 11.9: *immutable, content-addressed
  versions verified against the lockfile*). That single property lets a sealed
  leaf skip the entire apparatus:
  - **No per-compile read+hash** — its digest is a fixed coordinate, not
    something re-derived from the filesystem.
  - **No watcher registration** — it cannot change (and OS inotify ceilings
    make watching vendored trees infeasible anyway).
  - **No dirty bit / reverse edges** — permanently clean; never in a marking
    cone.
  - **Entries valid forever and shareable across projects** (see below).

## Where the sealed digest comes from

Key on the **lockfile-pinned release hash**, not bare semver: a registry could
re-publish a version with different bytes (supply-chain), so trusting `1.4.2` to
mean fixed bytes would break content-addressing. The lockfile already records a
per-version content hash (ch 11.8), taken **once at fetch/lock time** — the
sealed leaf just folds that in instead of reading the artifact.

Refinement the chapters don't yet spell out: the registry addresses a **whole
package** by tree hash, but a parse leaf is **one file**. So a sealed file's
coordinate is `(release_hash, path-within-package)` — a deterministic name for
immutable bytes (the tree hash pins the package; the path selects within it)
that needs no read to compute. The parse key is then `hash(schema, Parse,
digest(coordinate))`, and everything above the leaf is byte-identical to the
local case, because resolve/type-check chain through the parse *answer
fingerprint*, not the digest. The sealed-vs-local distinction is invisible
above the leaf.

Accepted consequence: a sealed file whose bytes happen to equal a local file's
do **not** dedup (different digest domains). That is the deliberate price of
never reading the artifact.

## Provenance: what may be sealed

Only a step whose **entire transitive leaf cone is sealed** may skip
invalidation. A step that also reads a local file, or is monomorphised at a
*local* type, mixes provenance and takes the normal tracked path. So every step
carries a cheap **provenance bit**: `sealed(step) = AND over inputs of
sealed(input)`. Leaves seed it (sealed coordinate = true, local file = false);
derived steps AND it. Only `sealed == true` steps are excluded from dirty
tracking, reverse-edge bookkeeping, and re-read. A mixed cone behaves exactly
as today — the conservative AND, never assumed.

## The lockfile is the tracked leaf that gates sealing

Sealed leaves are immutable, but *which* sealed coordinates are in play is not —
a dep bump changes it. That change flows through the **lockfile**, which is an
ordinary local, tracked leaf. Editing it yields a different coordinate → a
different key for the affected sealed cone; nothing is "invalidated", the old
key is simply no longer demanded and the new key is a cold (or shared-warm)
lookup. So "a dependency changed" is handled entirely through the one file the
engine does watch and hash, while the dependency artifacts stay frozen.

## Cross-project sharing

A sealed leaf's key has **no project-local ingredient**, so `dep@hash` produces
the same content key in any workspace. That is the first real reason for a cache
tier *above* the per-project one: a per-user / global **sealed store** that
serves every project on the machine (and warm CI starts). Sealed entries are
also the best eviction candidates — reload is a pure, shared disk hit that can
never be stale, so they can be dropped from hot memory most aggressively.

Store layout (prototype): sealed dependency *source* under
`$XDG_CACHE_HOME/tel/deps/<release-hash>/…` (Go/Deno-style — reconstructible,
so the XDG cache dir; promote to `$XDG_DATA_HOME` only if sources ever become
authoritative), and the shared sealed *answer* tier under
`$XDG_CACHE_HOME/tel/sealed`.

`TODO(open): the sandbox uses a temporary "hash-at-lock" (hash the vendored
tree) to stand in for a registry-provided checksum. Once the registry story
(ch 11.9) is real, the release hash is the registry's, and hash-at-lock is only
for local/vendored path deps.`
