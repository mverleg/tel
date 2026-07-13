# External dependencies as sealed leaves — design

Status: **direction decided 2026-07-10; not yet implemented.** An optimization
for external (published, vendored) dependencies: because their source is
**immutable by contract**, they escape the invalidation machinery entirely — no
per-compile read+hash, no watcher, no dirty bits — and their entries become
shareable across projects.

Related: [keys-and-invalidation.md](../../docs/keys-and-invalidation.md) (the
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

- **Leaf classification** — a `LeafSource` split (`Local { path }` vs
  `Sealed { coordinate_hash, semver }`); the parse key builder takes the digest
  from the coordinate for sealed leaves instead of reading+hashing.
- **Provenance bit** — one bit threaded through the graph/binding record;
  leaves seed it, derived steps AND it; sealed steps are excluded from the
  marking pass and never registered with the monitor.
- **Lockfile leaf** — parse the lockfile (a tracked local leaf) into the set of
  `(package → coordinate_hash)`; resolving an external import maps to a sealed
  leaf via this table.
- **Shared sealed disk tier** (later) — a read-through tier above the per-root
  cache, keyed purely on sealed content keys, safe to share across roots by
  content-addressing.

## Invariants preserved

- Content-addressing holds: sealed keys are still hashes of immutable bytes
  (taken from the lockfile), never mtimes or bare version strings.
- A mixed (local + sealed) cone is treated exactly as today — sealing is a
  conservative AND, never assumed.
- Compiler-version safety is unchanged: `SCHEMA_VERSION` still keys sealed
  answers, so a format change re-keys them like any other.
</content>
