# Execution, Concurrency, and Recovery

Design reference for the query compiler's *runtime* model: how concurrent builds coordinate,
how compiles are interrupted, and how failures and lost events are recovered. Companion to
[keys-and-invalidation.md](keys-and-invalidation.md), which holds the *data* model (the three
identifiers, the two cache layers, pull/push recomputation) and the authoritative invariants
list referenced below. Like that doc, this documents the model, not implementation status,
and stays valid after the implementation is done.

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

**Cancellation scope and resumption.** A cancel takes down the *entire query phase* — every
in-flight target, including ones the pending change does not touch — because the mutation
phase requires exclusivity. This is cheap to undo: unfinished root requests are simply
re-queued, and there is no suspend/resume machinery — "resume" is "re-request, and the memo
makes it cheap." Every query completed before the cancel is `Verified` in the memo (and
persisted), so after the mutation phase, targets whose cones were untouched re-pull through
green nodes at memo-hit speed, and an affected target restarts from exactly the queries that
had not finished. Resumption granularity is one query step; work is never lost at any coarser
grain.

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
  permanent, shared poison, since the append-only rule (invariant 5 in
  [keys-and-invalidation.md](keys-and-invalidation.md)) means it would never be evicted for
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
*after* its marking walk completes (the walk is post-order and therefore resumable — see the
marking invariant in [keys-and-invalidation.md](keys-and-invalidation.md)). Panic mid-walk ⇒
event still queued ⇒ next mutation phase finishes the unmarked remainder. An event whose walk
panics repeatedly triggers the fallback: discard the session memo (one cold pull). Process
death kills memo and queue together — the cold start subsumes both.

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
