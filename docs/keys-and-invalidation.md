# Query Keys and Invalidation

Design reference for the query compiler's caching model. This documents the *model*, not the
implementation status; it stays valid (and should be kept) after the implementation is done.

See also: [hashing.md](hashing.md) (hash widths and collision budgets),
[deterministic-hashing.md](deterministic-hashing.md) (how key/fingerprint inputs are kept
deterministic), [inverse-dependency-graph.md](inverse-dependency-graph.md) (reverse edges).

## Summary

Every query (compiler step instance) has **three identifiers**, each with a different job:

| | What | Width / form | Job |
|---|---|---|---|
| **Logical id** | (query kind, args), e.g. `type of X` | process-local, internable to small int | persistent *identity* of a query across time and edits |
| **Content key** | hash(kind, stable args, dep result fingerprints) | 128-bit (xxh3-128) | lookup key into the persistent cache; portable across runs and machines |
| **Result fingerprint** | hash(direct output) | 64-bit (xxh3-64) | ingredient of dependents' content keys; enables early cutoff |

Recomputation is driven from two directions:

* **From the root (pull, correctness):** re-derive content keys top-down; cache hits stop the
  recursion. Always correct on its own, no invalidation state needed.
* **From the leafs (push, optimization):** on file change, walk reverse edges to mark a dirty
  cone, so unchanged subgraphs are skipped without re-deriving their keys. Pure optimization;
  never affects correctness.

## The Three Identifiers

### Logical id

`(query kind, args)` — "parse file F", "type of X". This is how a query is *named*: it stays
the same when file contents change. Everything session-scoped hangs off it: the memo of the
last known content key and result fingerprint, dirty bits, and the dependency edges (both
directions). It is process-local and may use interned indices (`Sym`-style) freely — it never
leaves the process. A leaf's logical id is the file path; its changing *content* is the whole
point.

### Content key

`hash128(kind, stable form of args, result fingerprint of each direct dependency)`.

* **Transitive by recursion, one hop at a time.** The key contains only *direct* deps'
  result fingerprints; those fingerprints were themselves computed downstream of *their*
  deps. This forms a Merkle DAG over answers — crucially over answers, not over raw inputs.
  Folding transitive raw input into keys would make every downstream key change on any
  whitespace edit, destroying early cutoff.
* **Leaf content keys** hash the external input itself (source byte digest), which is where
  `input_state` enters the system.
* **Portable:** built exclusively from stable data (see
  [deterministic-hashing.md](deterministic-hashing.md)), so the same key means the same
  computation on any machine — this is what makes the persistent cache shareable.
* **128-bit** because keys identify a value among all values ever seen (birthday territory,
  see [hashing.md](hashing.md)).

### Result fingerprint

`hash64(direct output)` — and *only* the direct output. Given deterministic steps, the output
is a pure function of the content key, so there is nothing transitive left to add; anything
tempting is already reachable through the dep fingerprints inside the key.

64 bits suffices because of where fingerprints are compared: a collision only matters between
two fingerprints that could occupy the *same dependency slot* of the same dependent — i.e.
among the historical outputs of one logical query. That collision domain is bounded by edit
count per query (Σ n_q² / 2⁶⁵ over the project lifetime, negligible), not by cache size.
This argument holds only under invariant 2 below.

## Per-Phase Keyspaces

The query kind tags every identifier, so keys of different phases (parse, resolve, type
check, …) live in disjoint keyspaces — a parse key can never alias a type-check key. Beyond
collision hygiene this enables:

* **Typed, separate stores per kind** — the answer type is fixed per kind, so each kind can
  have its own table (no dynamic downcasts), its own eviction/GC policy (parse results are
  big and cheap to redo; type results are small and expensive), and its own disk layout.
* **The ordering constraint** from the query architecture (kinds may only call downward or
  sideways in the kind order) is naturally checkable at the boundary between keyspaces.

## Where Each Piece Lives

| Layer | Keyed by | Contains | Lifetime |
|---|---|---|---|
| **Session memo** (in-memory) | logical id | last content key, last result fingerprint, dirty bit, forward + reverse dep edges, `Ready/Pending` engine state | one process; rebuilt on start |
| **Persistent cache** (disk / shared) | content key | result value + its result fingerprint | across runs and machines; append + GC only |

The persistent cache is **never invalidated**. An entry under a content key is valid forever
by construction — if any input changed, the key would be different. Superseded entries are
garbage, not hazards; reclaim them with GC/LRU on their own schedule.

All dirtiness, edges, and "what changed" reasoning live exclusively in the session layer.
Keeping the two layers from contaminating each other is the core of the design.

## Recomputation From the Root (pull)

To answer the root query:

1. Recurse into deps to obtain their result fingerprints (leafs: digest the file bytes).
2. Form this query's content key.
3. Look it up in the persistent cache. Hit → done, record fingerprint in memo. Miss →
   execute the step, store result + fingerprint under the key.

**Early cutoff falls out automatically:** if a leaf changed but some intermediate step
produced byte-identical output (e.g. whitespace edit, same AST), that step's result
fingerprint is unchanged, so every key above it is unchanged and lookups are pure hits.

Cost note: even a fully-cached run walks the whole graph doing hash-chain lookups (you cannot
know the root's key without the leafs). Acceptable for batch compiles; too slow per keystroke
— hence the push direction.

## Invalidation From the Leafs (push)

With a file watcher and the reverse edges:

1. A leaf changes → mark its logical id and (transitively, via reverse edges) its dependents
   as *maybe dirty*. Marking is conservative and cheap; do no hashing here.
2. Queries **not** marked reuse their memoized content key and fingerprint with zero
   recursion — this is the entire payoff: untouched subgraphs cost nothing.
3. To clean a marked query: refresh its deps (recursively, same rules), rebuild its content
   key from their fingerprints, and compare with the memo.
   * Key unchanged → **un-dirty without executing**; stop propagating. (Early cutoff again,
     now saving even the cache lookup for everything above.)
   * Key changed → persistent-cache lookup, execute on miss, update memo, and dependents
     stay dirty until cleaned the same way.

This is rustc's red-green algorithm transplanted onto content-addressed keys: green =
memoized key still derivable, red = key changed. The difference from rustc is that the
backing store is a pure content-addressed map, so it needs no revision counters and can be
shared across machines.

## Invariants

1. **Everything hashed into a content key or result fingerprint is deterministic** —
   enforced by the `StableHash` trait
   ([deterministic-hashing.md](deterministic-hashing.md)). Interner indices, map iteration
   order, spans-in-fast-mode etc. must never leak in.
2. **Result fingerprints are never storage or lookup keys.** Results are stored only *under*
   content keys. The tempting future violation: deduplicating identical large answers by
   their fingerprint — that re-globalizes the collision domain and breaks the 64-bit budget.
   If answer dedup is ever wanted, give the blob store its own 128-bit content digest.
3. **Dependent key preimages pin dependency identity** (kind + args / position, with length
   prefixes), so fingerprints of *different* queries can never collide into the same slot.
4. **Memo updates are atomic per query** (key + fingerprint together), gated by the
   `Ready/Pending(waker)` engine states — otherwise a dependent can build its key from a
   stale fingerprint mid-run.
5. **The persistent cache is append-only** (plus GC). Any urge to "invalidate" a persistent
   entry means a determinism bug is being papered over — fix the input capture instead.

## Worked Example

Dependency chain: `exec A → type of f → resolve util.tel → parse util.tel → util.tel bytes`.

* **Comment-only edit to util.tel:** watcher marks the leaf's cone dirty. Parse's content
  key (byte digest) changed → parse re-runs → AST identical (fast mode, no spans) → parse
  fingerprint unchanged → resolve's content key unchanged → resolve un-dirtied without
  executing, propagation stops. One parse, nothing else.
* **Edit to f's body, signature unchanged:** parse and resolve re-run with new outputs;
  `type of f` re-runs but produces the same type → same fingerprint → `exec A`'s key
  unchanged → cutoff. Type checking of *other* items that only used f's signature never
  even got dirty (no reverse edge to them from this leaf's cone… or they clean via
  unchanged keys).
* **Colleague pulls the same source:** identical bytes → identical leaf digests → identical
  content keys all the way up → their compile is pure cache hits from the shared store,
  no invalidation protocol needed between machines.
