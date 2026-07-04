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
| **Session memo** (in-memory) | logical id | last content key, last result fingerprint, forward + reverse dep edges, node state (`Unknown / Dirty / Pending / Verified`) | one process; rebuilt on start |
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

**Marking invariant and resumability.** Stopping the walk at already-dirty nodes is only
sound if *dirty implies all transitive dependents are dirty*. A marking walk that dies
halfway (panic in the mutation phase, in a process that survives) would break that invariant
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

## Concurrent Builds

Multiple end-targets can be pulled concurrently in one process; overlap coordinates through
per-query single-flight.

* Memo nodes move through `Unknown / Dirty / Pending(owner, wakers) / Verified(key, fp)`.
  The first task to need a query claims it (atomic transition to `Pending`) and computes;
  later arrivals await the waker. Cleaning a `Dirty` node happens under the same claim,
  so overlapping targets coordinate identically whether a node is fresh or dirty — leaf-driven
  mode is not a separate build mode, just pull over a memo that carries dirty flags.
* Determinism makes lost races benign: if two tasks ever do compute the same query, they
  produce byte-identical keys and results, and content-addressed writes are idempotent.
  Locking discipline affects throughput, never correctness.
* Within a query phase, `Verified` nodes never regress and result storage is append-only, so
  results can be borrowed by any number of tasks without read locks.
* Everything on the engine path must be async and `&self`: a lock held across an `.await` or
  a sync block on a result can wedge the executor (starvation that looks like deadlock).

### Mutation vs query phases

File-change events are never applied while queries are in flight; they queue. Applying them —
updating leaf digests, the marking walk, the generation bump — happens in a *mutation phase*
mutually exclusive with the query phase. To interleave (IDE keystrokes), first cancel the
in-flight wave cooperatively: everything completed before the cancel is already memoized and
persisted, so the restarted pull is mostly hits and cancelled work is never wasted. This
keeps per-node transitions simple (marker runs only in mutation phase, cleaner only in query
phase). Multi-version snapshots (queries and mutations overlapping) were considered and
rejected: large complexity, small win for a compiler.

**Interruption mechanics.** No JVM-style safepoint machinery is needed: in async Rust every
`.await` is a natural safepoint. Cancellation is cooperative — abort the wave's task group
(futures dropped at their next await; drop guards such as the `Pending` claim revert restore
state) or signal a `CancellationToken` that tasks check at query boundaries. Cost during
normal execution is zero for drop-based cancellation and one atomic load per check for
token-based (the JVM pays <1% for safepoint polls in arbitrary code; we need less, since
yield points already exist). Granularity is one query step: a long CPU-bound kernel between
awaits cannot be interrupted until it finishes or explicitly checks the token every N
iterations — steps are small by design, so wave cancellation lands within milliseconds.

### Cycle detection

The kind-ordering rule eliminates cross-kind wait cycles, but sideways (same-kind) calls can
cycle: `resolve A → resolve B → resolve A`. Under single-flight this is a *silent hang* (a
task awaits a `Pending` whose owner transitively awaits it) — worse than the single-threaded
symptom (stack overflow), so it must be detected. See also
[cycle-detection.md](cycle-detection.md).

Performance: detection sits on the slow path by construction.

* Same-task re-entry (the common case: one call chain recursing into itself) is O(1) — each
  `Pending` records its owning task; re-entering a node you own is an immediate cycle error.
* Cross-task cycles are checked only immediately before *parking* on someone else's
  `Pending`, by walking the owner chain (bounded by the depth of blocked tasks, typically
  single digits). Parking already costs a waker registration and a scheduler round trip; the
  walk is noise next to it. Memo hits, cache hits, and uncontended claims never pay anything.
* Fallback if contention ever shows in profiles: rustc's parallel-mode approach — no check on
  await at all, cycle scan run only from a deadlock watchdog (all workers parked, no
  progress). Zero cost until an actual deadlock, at the price of delayed diagnostics.

Diagnosis follows the fast/IDE mode split: in fast mode, detection only flags "cycle
involving query Q" and aborts the wave; the IDE-mode retry re-encounters the cycle with full
metadata and produces the real diagnostic (cycle path with source locations). Detection is
cheap and always-on; explanation is slow and on-demand.

## Interruption and Failure

The classifying question for any outcome: **is it a function of the content key, or an
accident of this run?**

* **Deterministic compile errors are answers.** A type error *is* the answer to `type of X`
  for those inputs. `Err` answers are fingerprinted, cached, and persisted like successes
  (reopening a broken project shows its errors instantly from cache). Dependents recover
  deterministically (poisoned type, rustc's `TyKind::Error` pattern) so one error does not
  cascade into thousands. The error type must itself satisfy
  [deterministic-hashing.md](deterministic-hashing.md).
* **Panics, cancellation, OOM, and transient IO failures are not answers.** The `Pending`
  claim reverts to not-started; waiters are woken with "aborted" (never handed a fabricated
  answer), and abort-ness is sticky within the run: no step that observed an aborted dep may
  produce an answer. Nothing derived from an aborted subtree is written to the persistent
  cache — a transient failure laundered into a deterministic-looking `Err` would be
  permanent, shared poison, since invariant 5 means it would never be evicted for
  correctness reasons.

**Interrupting a compile is safe by construction.** A persistent entry is a self-contained
fact — "these inputs produce this output" — whose validity does not depend on the enclosing
compile finishing (the same reason a git object store tolerates killed pushes). An
interrupted compile leaves the store valid and strictly richer; partial compiles are free
checkpointing and no resume protocol exists. The required discipline is per-entry only:
atomic visibility (write-temp + rename, or a transactional store), a checksum over the
serialized bytes verified on read (torn write ⇒ treat as miss, delete), and no in-progress
state ever persisted. Two runs racing on the same key write identical bytes, so no locking.

**Resumption after a half-cleaned wave** is invariant 8 at work: green-ness is only granted
at the moment it is proven, so an aborted wave leaves a downstream-closed green set (its
proofs happened — still valid) plus the remaining dirty set; the next wave continues with no
extra bookkeeping. Violating invariant 8 (e.g. clearing dirty bits at scheduling time) is
the one mistake in this design that produces wrong results *without any hash collision*: a
stale fingerprint gets baked into a dependent's key, which then scores a genuine 128-bit
cache hit describing inputs that no longer exist.

## Lost Dirty Information

Dirty marking is event-driven, and events can be lost: a crash mid-marking, a file watcher
overflowing or missing events, edits made while the process is down. These are all the same
failure and share one cure: **events are hints; leaf digests are ground truth.** Correctness
never depends on having seen a notification — any trust boundary re-derives dirtiness by
comparing current leaf digests against recorded ones.

* While the memo is in-memory only, a crash during marking is a non-event: the memo (dirty
  bits included) dies with the process, and the next start pulls from the root, re-deriving
  every key from freshly digested leafs. Slower, never wrong. No cache poisoning and no
  persistent dirty-event queue is needed — such a queue cannot be complete anyway (no event
  exists for edits made while the process was down, and the watcher can die before
  enqueueing), so digest reconciliation is required regardless, and once it exists the queue
  adds nothing.
* If the memo is ever persisted to speed up cold starts, persist the recorded *leaf digests*
  with it — never dirty events. Startup then re-checks leafs (mtime first, hash on change)
  and runs the marking walk from every leaf whose digest differs from the recorded one. One
  mechanism subsumes crash-during-marking, watcher gaps, and offline edits. This is rustc's
  stance on incremental state: watchers only accelerate; freshness is always re-checked.
* Watcher overflow (e.g. inotify `IN_Q_OVERFLOW`) is detectable and triggers the same
  reconciliation immediately.

### Exact recovery mechanisms

**Interrupted marking** needs no detector; the mechanism is that the trigger outlives the
attempt. Events queue in memory, coalesced by path; the mutation phase pops an event only
*after* its (post-order, resumable) marking walk completes. Panic mid-walk ⇒ event still
queued ⇒ next mutation phase finishes the unmarked remainder. An event whose walk panics
repeatedly triggers the fallback: discard the session memo (one cold pull). Process death
kills memo and queue together — the cold start subsumes both.

**Missed events** are handled by one primitive, `reconcile()`: for each known leaf, `stat`;
mtime+size changed ⇒ re-digest; digest changed ⇒ synthesize a change event into the normal
queue (reconciliation is just a slower event source, reusing the mutation-phase machinery).
Mtime is itself only a hint (mtime-preserving tools, clock skew) — digest is truth; paranoid
mode skips the stat fast-path and hashes everything.

When `reconcile()` runs, by mode — the asymmetry is justified by consequences: IDE staleness
self-heals at the next keystroke; batch staleness ships a wrong artifact:

| Trigger | IDE / watch mode | Batch / CI |
|---|---|---|
| Session start | always (free: empty memo ⇒ cold pull) | always |
| Watcher overflow / watch error | immediately | n/a — batch mode has no watcher |
| Before each wave | no — trust the watcher, latency is king | always — never trust a watcher |
| Window focus regained | optional cheap re-stat | n/a |

Batch/CI mode does not run a watcher at all: it re-stats all leafs at wave start (what
make/Bazel do every build; milliseconds for thousands of files). A fresh CI process has an
empty memo and is maximally careful automatically; only a long-running build daemon needs the
explicit per-wave reconcile.

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
