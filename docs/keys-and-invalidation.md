# Query Keys and Invalidation

Design reference for the query compiler's caching model. This documents the *model*, not the
implementation status; it stays valid (and should be kept) after the implementation is done.

This doc covers the *data* model: the three identifiers, the two cache layers, and pull/push
recomputation, plus the authoritative invariants list. The *runtime* model — concurrency,
cancellation, cycle detection, failure handling, and recovery from lost events — lives in
[execution-and-recovery.md](execution-and-recovery.md).

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
| **Session memo** (in-memory) | logical id | last content key, last result fingerprint, forward + reverse dep edges, node state (`Unknown / Dirty / Pending / Verified`) | one process; rebuilt on start |
| **Persistent cache** (disk / shared) | content key | result value + its result fingerprint | across runs and machines; append + GC only |

The persistent cache is **never invalidated**. An entry under a content key is valid forever
by construction — if any input changed, the key would be different. Superseded entries are
garbage, not hazards; reclaim them with GC/LRU on their own schedule.

**Cutoff is driven by fingerprints (memo), not values (store) — so a reclaimed intermediate
never blocks it.** Early cutoff propagates on result *fingerprints*, which live in the session
memo; a step's *value* lives only in the persistent store under its content key, and the two
have independent lifetimes. In a chain `A → B → C`, if C's output is unchanged, B rebuilds an
unchanged content key from C's *memoized* fingerprint and reports its own unchanged fingerprint
up to A — **without B's value being present in the store at all.** A GC'd intermediate value
therefore cannot stall a cutoff. A value is fetched only when some dependent must actually
*execute*; if it was reclaimed, that is an ordinary store miss → recompute, and determinism
(invariants 1–2) guarantees the recompute reproduces the *identical* fingerprint, so nothing
upstream is disturbed. The only thing that must be present for B to count as clean is B's
*fingerprint in the memo*: lose that and B is `Unknown` (re-pulled), never falsely clean.

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
   as *dirty* (conservative: it may verify unchanged later). Marking is cheap; do no hashing
   here. The walk stops at nodes that are already dirty — sound because of the marking
   invariant below, and it makes the very common IDE case free: a file that changes *again*
   before any compile ran is already dirty, so re-marking is O(1) (nothing to do beyond
   coalescing the event by path; the new content is picked up when the next wave reads the
   leaf digest).
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

Rejected alternative: pure *verifying traces* (rustc/salsa — in "Build Systems à la Carte"
terms; ours is the *constructive* variant): look up by logical key, store the previous dep
list with fingerprints, revalidate on each run. Cheaper bookkeeping single-machine, but its
cache entries are only meaningful relative to one machine's revision history — it cannot be
shared across runs and machines, which content-addressed keys give us for free. We keep the
verifying idea only as the session-memo optimization described above.

**Marking invariant and resumability.** Stopping the walk at already-dirty nodes is only
sound if *dirty implies all transitive dependents are dirty*. A marking walk that dies
halfway (panic in the mutation phase — see
[execution-and-recovery.md](execution-and-recovery.md) for the phase model — in a process
that survives) would break that invariant
— nodes marked, their dependents not — and the retry would stop early at the half-marked
frontier, leaving green nodes wrongly trusted. Two rules keep marking safe without requiring
panics to be fatal:

* **Flip post-order:** mark a node only after all its dependents are marked (recurse up the
  reverse edges first, flip on the way back; the leaf flips last). Then the invariant holds
  at every intermediate state, and an interrupted walk is resumable: re-walking from the
  still-unflipped leaf revisits and finishes exactly the unmarked remainder.
* **Consume the change event only after the walk completes.** A panic mid-walk leaves the
  event queued; the next mutation phase retries.

Belt-and-braces fallback: any unexpected panic in the mutation phase may simply discard the
entire session memo (never the persistent cache) — the next wave does one full pull-from-root,
which is always correct. Query-phase panics need none of this: dirty bits are untouched by
cleaning failures (invariant 8), so a half-cleaned wave is already a correct restart state.

**Why marking never cleans eagerly.** Eager bottom-up propagation ("recompute upward from
the changed leaf, stop where the fingerprint stops changing") degenerates into pulls anyway —
with dynamic deps, recomputing a node needs the current fingerprints of *all* its deps, which
is a pull rooted at that node. Worse, it executes *zombies*: the old edges are its only
guide, so it faithfully re-runs queries the new code no longer reaches (delete a function and
"type of f" re-runs against a definition that no longer exists, producing an error that must
then be distinguished from a live diagnostic). Lazy cleaning executes only
`dirty cone ∩ live cone` — what any correct scheme must execute — while marking is bit flips
only. Over-marking of dead subgraphs is inherent (yesterday's edges) but nearly free: marked
nodes nobody pulls are never executed, they just sit dirty until GC. The eager behavior is
still available as pure scheduling policy: mark, then immediately re-pull the root.

Reverse edges from the *last* execution are sound for marking: a node can only acquire a new
dependency by executing differently, which (by determinism) requires one of its old deps'
fingerprints to have changed — a path the old edges cover. Marking is thus conservative.

How concurrent targets coordinate over this model, how waves are cancelled and resumed, how
failures are classified, and how lost dirty events are recovered is covered in
[execution-and-recovery.md](execution-and-recovery.md).

## Where a step's dependencies come from — recorded edges, not a re-run body

A natural worry when authoring a step: if a step discovers its dependencies *by executing*
(calling `ctx.parse`, `ctx.resolve`, … inline), how can the engine ever *skip* that execution
on a cache hit? Building the content key needs the deps' fingerprints, and "the deps" seem to
be knowable only by running the very body you hoped to skip. This is a false tension: a step is
authored as **one flat body that pulls dependencies inline**, and each pull records a forward
edge (plus its reverse). That remembered edge set — not a re-run of the body — is what the next
run reuses:

* **Cold (no memo record):** nothing to skip. Run the body, discover deps, record edges +
  fingerprints. You "pay for the body," but only when there was no cached answer to serve
  anyway.
* **Warm + clean (push mode):** served from the memo with **zero recursion** — the recorded key
  and fingerprint are returned without touching deps at all. Re-deriving is never triggered by
  merely *reading* a clean node.
* **Warm + dirty:** rebuild the content key from the **recorded** forward edges' current
  fingerprints (refreshing each recursively). The body runs *only* if that key changed. The
  recorded edge set is self-correcting: whatever determines the dep set is itself a recorded
  dep, so if the true deps changed, some recorded fingerprint changed, the key mismatches, and
  the re-executed body re-discovers the correct deps.

So "you'd have already paid for the body to find out you didn't need it" holds *only* on the
cold path, where there is no hit to miss. On every warm path the deps come from last run's
edges, never from re-running the body — and in push mode a clean node is not even walked.

**Corollary — the driver/body split is an implementation artifact, not this model.**
`parse_impl` already *is* the flat form (grabs its leaf inline, registers the edge, serves clean
hits from the memo). The explicit "async driver gathers deps, then a restricted sync body
computes" shape at resolve and the backend (`gather_backend_inputs`, `ResolveContext`,
`BackendCtx`) is **not** required by content-addressing: the backend body is fenced behind a
bare `fn` over a borrowed context purely for leak-safety (roadmap item 16), and the backend is
uncached anyway; resolve builds its key in `resolve_one` ahead of a deliberately non-pulling
body. That the flat form is viable is shown *within this same crate*: `parse_impl` uses it while
resolve/backend do not, over the identical memo + recorded-edge machinery. Nothing in this
document requires the split, and authors need not — and should not — hand-write a deps-up-front
phase to get correct incrementality; the memo plus recorded edges deliver it for a flat body.

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
   `Pending(waker)`/`Verified` node states — otherwise a dependent can build its key from a
   stale fingerprint mid-run.
5. **The persistent cache is append-only** (plus GC). Any urge to "invalidate" a persistent
   entry means a determinism bug is being papered over — fix the input capture instead.
6. **Only terminal deterministic answers are persisted.** A deterministic `Err` is a terminal
   answer; a panic/cancellation/transient failure is not, and nothing derived from an aborted
   subtree may reach the persistent cache. Entries are written atomically (temp + rename or
   transactional) and checksum-verified on read.
7. **Abort reverts `Pending`** (extends invariant 4): on cancellation or panic, `Pending`
   returns to not-started, waiters are woken with "aborted", and abort-ness is sticky within
   the run.
8. **Dirty bits are cleared only atomically with a successful verification or recompute** —
   never at scheduling time. This is the one rule whose violation yields stale results with
   no hash collision involved.
9. **Each compile wave runs on a pinned input generation.** The first read of a leaf in a
   wave fixes its digest for the whole wave; mid-wave file events queue for the next
   generation (optionally cancelling the current wave).
10. **File-change events are hints, never truth.** Any trust boundary (startup, watcher
    overflow) re-derives dirtiness by comparing current leaf digests against recorded ones.

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
