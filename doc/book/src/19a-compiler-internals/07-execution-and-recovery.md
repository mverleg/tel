# Execution and Recovery

<!-- TODO: review -->

The runtime half of the model: how concurrent compiles coordinate, how a
compile is interrupted, and how failures and lost events are recovered.
[Keys and fingerprints](03-keys-and-fingerprints.md) and
[Invalidation](04-invalidation.md) hold the data half.

## Concurrent compiles

Several end-targets can be pulled concurrently in one process; overlap is
coordinated by **per-query single-flight**. The first task to need a query
claims it — an atomic transition to `Pending` — and computes it; later arrivals
await a waker rather than duplicating the work.

- Cleaning a `Dirty` node happens under the same claim as computing an
  `Unknown` one, so overlapping targets coordinate identically whether a node
  is fresh or dirty.
- **Determinism makes lost races benign.** If two tasks ever do compute the same
  query, they produce byte-identical keys and byte-identical results, and
  content-addressed writes are idempotent. Locking discipline affects
  throughput, never correctness.
- Within a query phase, `Verified` nodes never regress and result storage is
  append-only, so answers can be borrowed by any number of tasks without read
  locks.
- Everything on the engine path must be async and take `&self`. A lock held
  across an await point, or a synchronous block on a result, can wedge the
  executor — starvation that presents as deadlock.

### Mutation vs query phases

File-change events are **never** applied while queries are in flight; they
queue. Applying them — updating leaf digests, running the marking walk, bumping
the generation — happens in a **mutation phase** that is mutually exclusive with
the query phase.

To interleave them (an editor sending keystrokes mid-compile), the in-flight
wave is cancelled cooperatively first. Nothing is wasted: everything that
completed before the cancel is already memoized and persisted, so the restarted
pull is mostly hits.

This keeps per-node transitions simple — the marker runs only in the mutation
phase, the cleaner only in the query phase.

> Rejected alternative: multi-version snapshots, letting queries and mutations
> overlap. Large complexity, small win for a compiler.

### Interruption mechanics

No safepoint machinery is needed: in async code every await point is a natural
safepoint. Cancellation is cooperative — either drop the wave's task group, so
futures are dropped at their next await and drop guards revert the `Pending`
claims, or signal a cancellation token that tasks check at query boundaries.

The cost during normal execution is zero for drop-based cancellation and one
atomic load per check for token-based. Granularity is one query step: a
long CPU-bound kernel between awaits cannot be interrupted until it finishes or
explicitly polls the token. Steps are small by design, so wave cancellation
lands within milliseconds.

**Scope and resumption.** A cancel takes down the *entire* query phase — every
in-flight target, including ones the pending change does not touch — because the
mutation phase requires exclusivity. This is cheap to undo, and there is no
suspend/resume machinery at all: "resume" is "re-request, and the memo makes it
cheap". Unfinished root requests are simply re-queued. Every query completed
before the cancel is `Verified` and persisted, so after the mutation phase,
untouched targets re-pull through green nodes at memo-hit speed and an affected
target restarts from exactly the queries that had not finished. Work is never
lost at a grain coarser than one query step.

## Failure classification

The question that classifies any outcome: **is it a function of the content key,
or an accident of this run?**

**Deterministic compile errors are answers.** A type error *is* the answer to
"type of `X`" for those inputs. Error answers are fingerprinted, cached, and
persisted exactly like successes — reopening a broken project shows its errors
instantly from cache. Dependents must recover deterministically (a poisoned
type, rustc's error-type pattern) so one error does not cascade into thousands.
The error type must itself satisfy
[deterministic hashing](06-deterministic-hashing.md).

**Panics, cancellation, out-of-memory, and transient IO failures are not
answers.** The `Pending` claim reverts to not-started, waiters are woken with
"aborted" rather than handed a fabricated answer, and abort-ness is *sticky*
within the run: no step that observed an aborted dependency may produce an
answer. Nothing derived from an aborted subtree is written to the persistent
cache — a transient failure laundered into a deterministic-looking error would
become permanent, shared poison, since the append-only rule means it would never
be evicted for correctness reasons.

### Interrupting a compile is safe by construction

A persistent entry is a self-contained fact — "these inputs produce this
output" — whose validity does not depend on the enclosing compile finishing.
This is the same reason a git object store tolerates a killed push. An
interrupted compile leaves the store valid and strictly *richer*: partial
compiles are free checkpointing, and no resume protocol exists.

The required discipline is per-entry only:

- atomic visibility — write to a temporary name and rename, or use a
  transactional store;
- a checksum over the serialized bytes, verified on read; a torn write is
  treated as a miss and deleted;
- no in-progress state ever persisted.

Two runs racing on the same key write identical bytes, so no locking is needed.

### Resuming a half-cleaned wave

Green-ness is only granted at the moment it is proven, so an aborted wave leaves
a downstream-closed green set — whose proofs happened and are still valid — plus
the remaining dirty set. The next wave continues with no extra bookkeeping.

Violating that rule ([invariant 8](09-invariants.md) — for instance by clearing
dirty bits at scheduling time rather than at commit time) is the one mistake in
this design that produces wrong results *without any hash collision*: a stale
fingerprint gets baked into a dependent's key, which then scores a genuine
128-bit cache hit describing inputs that no longer exist.

## Lost dirty information

Dirty marking is event-driven, and events get lost: a crash mid-marking, a file
watcher overflowing or missing events, edits made while the process was down.
These are all the same failure and share one cure:

> **Events are hints; leaf digests are ground truth.**

Correctness never depends on having seen a notification. Any trust boundary
re-derives dirtiness by comparing current leaf digests against recorded ones.

- While the memo is in memory only, a crash during marking is a non-event: the
  memo dies with the process and the next start pulls from the root, re-deriving
  every key from freshly digested leafs. Slower, never wrong. No persistent
  dirty-event queue is needed — such a queue could not be complete anyway (no
  event exists for edits made while the process was down, and the watcher can
  die before enqueueing), so digest reconciliation is required regardless, and
  once it exists the queue adds nothing.
- If the memo is ever persisted to speed up cold starts, persist the recorded
  **leaf digests** with it — never dirty events. Startup then re-checks leafs
  (cheap metadata first, hash on change) and runs the marking walk from every
  leaf whose digest differs. One mechanism subsumes crash-during-marking,
  watcher gaps, and offline edits.
- Watcher overflow is detectable and triggers the same reconciliation
  immediately.

### The recovery mechanisms

**Interrupted marking** needs no detector; the mechanism is that the trigger
outlives the attempt. Events queue in memory, coalesced by path, and the
mutation phase pops an event only *after* its marking walk completes — and the
walk is post-order, therefore resumable. A panic mid-walk leaves the event
queued, so the next mutation phase finishes the unmarked remainder. An event
whose walk panics repeatedly triggers the fallback: discard the session memo,
one cold pull. Process death kills memo and queue together, and the cold start
subsumes both.

**Missed events** are handled by one primitive, `reconcile()`: for each known
leaf, check metadata; if size or mtime changed, re-digest; if the digest
changed, synthesize a change event into the normal queue. Reconciliation is just
a slower event source, reusing the mutation-phase machinery. Mtime is itself
only a hint — mtime-preserving tools and clock skew exist — so the digest is
truth; a paranoid mode skips the metadata fast path and hashes everything.

When `reconcile()` runs depends on the mode, and the asymmetry is justified by
consequences: editor staleness self-heals at the next keystroke, batch staleness
ships a wrong artifact.

| Trigger | Editor / watch mode | Batch / CI |
|---|---|---|
| Session start | always (free: empty memo means a cold pull anyway) | always |
| Watcher overflow or error | immediately | n/a — no watcher |
| Before each wave | no — trust the watcher, latency is king | always — never trust a watcher |
| Window focus regained | optional cheap re-check | n/a |

Batch mode does not run a watcher at all: it re-checks all leafs at wave start,
which is what make and Bazel do every build — milliseconds for thousands of
files. A fresh CI process has an empty memo and is therefore maximally careful
automatically; only a long-running build daemon needs the explicit per-wave
reconcile.
