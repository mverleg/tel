# Cancellation and Timeouts

<!-- TODO: review -->

Concurrent work needs a way to *stop*: a raced query whose siblings already
won, a request that overran its deadline, a subtree whose parent failed.
Tel handles all of these through one mechanism — **cancelling a task** — and
ties timeouts to it. Because tasks form a [tree](04-structured-concurrency.md),
cancellation is well-behaved: it always applies to a whole subtree.

## Cancelling a task

Cancelling a [task](02-tasks.md) requests that it stop. Two rules make it
predictable:

1. **Cancellation is hierarchical.** Cancelling a task cancels all of its
   descendants first. You never have to chase down stray children — the task
   tree *is* the unit of cancellation. This is the same invariant that powers
   [structured concurrency](04-structured-concurrency.md): a parent does not
   finish until its children have.
2. **Cancellation is cooperative, at well-defined points.** A task is not
   killed mid-instruction. It observes the cancellation request at a
   **yield point** — a place where it would already pause, such as a blocking
   call, a channel send/receive, or a `join`. At such a point a cancelled task
   stops instead of continuing.

So cancelling a parent interrupts its children *at their next yield point*,
unwinds the subtree, and the cancelled tasks' [isolated heaps](07-memory-model-for-concurrency.md)
are dropped wholesale — the same clean teardown as a panicking task.

<!-- TODO(open): This framing is being revised by
[TIP-0012](../tips/0012-task-cancellation-abort-and-shutdown.md) (Draft) and
must be reconciled on its acceptance. Two changes: (1) cancellation is a stdlib
cooperative *token* observed as a *value* (a token-aware blocking op returns
`Cancelled`, propagated by ordinary early return) — not a runtime force-stop, so
`h.cancel()` below is "trip the task's token," not "inject a stop at a yield
point"; (2) teardown *runs cleanup*, it is not a bare wholesale drop —
cancellation cleanup runs as ordinary code on the normal early-return path, and
even a *panic* runs the NoPanic cleanup unwind before the heap is reclaimed. So
"the same clean teardown as a panicking task" is right that both run cleanup, but
must not be read as "skips uses." Only true catastrophe (OOM, host teardown)
skips cleanup. -->

```tel
let h = tasks.spawn("slow-scan", || scan_everything(corpus))
# ... decide we no longer need it ...
h.cancel()        # h and any tasks it spawned stop at their next yield point
```

The composition operators in [composing tasks](05-composing-tasks.md) cancel
for you: `race_first` cancels the losers once a winner is in; `await_all`
cancels the siblings when one task fails.

## Why cooperative, not preemptive

- **Portability.** Preempting a task mid-execution requires OS-thread control
  the host may not have — a browser cannot preempt a running JS callback. A
  yield-point model works on every host, including one that runs tasks
  sequentially.
- **Predictability.** A task that stops only at yield points never stops with a
  half-mutated structure. Combined with per-task heaps, the cancelled task's
  state simply ceases to exist; nothing else has to be repaired.
- **No unwinding tax.** Because cancellation surfaces only at points where the
  task already yields, ordinary code does not have to be written
  "cancellation-safe" — consistent with Tel having
  [no exceptions and no surprise control flow](../02-philosophy/04-antifeatures.md).

### Cancellation safety, briefly

In async Rust, cancelling a future is a synchronous `drop` of the future
object. That makes "cancellation safety" a subtle property: a function that
holds an invariant across an `await` can be cancelled at that point and
leave its state half-updated, with no chance to run a cleanup. Tokio's
mutex, for instance, is *not* poisoned on cancellation — a cancelled task
inside the critical section silently releases the lock with the invariant
broken.

Tel's design dissolves most of this:

- **Heaps are isolated**, so a cancelled task's broken invariants vanish
  with its heap. There is no shared structure for the next caller to find
  in a bad state.
- **A scope can run cleanup** before its frame is torn down (the
  `with`-shaped / try-with-resources story under TODO below). Cleanup
  itself can yield and block, because there are no destructor-vs-async
  problems to dodge.
- **Locks live in the stdlib**, not in user code; their cancellation
  contract is part of the type, not a thing each script reinvents.

The residual concern is **host-side state** a cancelled task was
manipulating (an in-flight FFI call, a half-written file). The host owns
that resource; the language can only promise to call the resource's
declared cleanup. See [the FFI story](../16-ffi-and-interop/).

## Timeouts

A timeout is just a deadline-driven cancellation: run a task, and if it has not
finished when the deadline passes, cancel it (and its subtree).

```tel
match run_with_timeout(2.seconds, || fetch(url)) {
    Completed(doc) => use(doc),
    TimedOut       => fall_back(),
}
```

Timeouts depend on a clock, and in Tel **the clock is a
[capability](../02-philosophy/03-features.md)**, not ambient — there is no
global `now()`. A timeout reads the time from the same injected `Clock` the
rest of the script uses. This is what makes the next section possible.

## Clock control for testing

Because the clock is injected, a test can supply a **fake clock it controls**.
This is decisive for testing timeout-sensitive code:

- A real-time test of a 30-second timeout takes 30 seconds. A controlled-clock
  test **advances the clock instantly** to the deadline, fires the timeout, and
  asserts the result — in microseconds.
- Tests become deterministic: the timeout fires *exactly* when the test moves
  the clock past the deadline, never a flaky millisecond early or late.

The technique mirrors how deterministic simulators (the
FoundationDB / "messaging in-memory, advance the clock to arrival time" style)
test timeouts without waiting for wall-clock time.

```tel
# A test drives the clock directly.
test "request times out after its deadline" {
    let clock = TestClock.at(epoch)
    let h = tasks.spawn("req", || serve(req, clock))

    clock.advance(31.seconds)        # jump past the 30s deadline
    assert h.join() == Err(TimedOut)
}
```

This is the concurrency-facing payoff of Tel's general rule that *time and
randomness are injected, not ambient* (see
[the maxims](../02-philosophy/02-maxims.md)) — see also the `Clock` capability
in the [standard library](../17-standard-library/) and
[goals](../01-overview/03-goals-and-non-goals.md).

### Concurrency simulation for testing

The same principle — make timing injectable, drive it from the test —
generalises beyond timeouts. There is also the idea of a **concurrency
simulator** the host can offer: a deterministic scheduler that exposes
exactly the pathologies a script must survive.

- **Reorderings under weak memory models.** A test running on x86 sees the
  strong ordering x86 happens to provide; the same script may fail on an
  ARM host where stores can be reordered. A simulator can permute the order
  of channel operations and parked-task wake-ups so the failure surfaces in
  CI on any machine.
- **Worst-case scheduling.** Queues that almost-fill, workers that wake
  slowly, fan-outs where one branch finishes far ahead of another. A
  simulator can dial these knobs.
- **Failure injection.** A task panics at a specific point; a channel
  closes mid-stream. These are normal failure paths the script should
  already handle, but exhaustively triggering them is hard without a
  controlled scheduler.

Tel exposes none of this as language surface — it is **library territory**
and depends on the host runtime. The two language-level enablers are
already in place: the [clock is injected](08-cancellation-and-timeouts.md)
and tasks/channels go through a runtime that *could* be the simulator.
FoundationDB's deterministic-simulation testing is the inspiration the
inputs cite.

### A cleanup checklist for resources

A checklist that every resource type should answer (a
file handle, a channel, a database transaction, an FFI handle) — its
answers shape both the type's API and its cancellation behaviour:

1. **Who is responsible for cleanup — the type or the user?** A type that
   knows how to clean itself up (close, flush, rollback) is the
   `with`-shaped case. A type that needs caller-supplied policy
   (commit vs rollback?) needs explicit user code.
2. **Can the user forget?** A type that does its own cleanup at scope exit
   cannot be forgotten. A type that requires an explicit `close` must be
   used through a scope wrapper or the compiler must enforce the call.
3. **Is cleanup allowed to suspend?** A buffered writer flushing to disk, a
   transaction issuing `COMMIT`, a network release. Tel's "cleanup can
   block" answer (see [function colouring](03-async-and-function-colouring.md))
   makes this trivially yes.
4. **Can cleanup return an error?** A transaction commit can fail. Cleanup
   that may fail cannot be implicit — the caller has to handle the
   `Result`. This is where pure `Drop`-style destructors run out.
5. **What happens to cleanup under cancellation?** If the task is being
   cancelled, does cleanup still run? Does cancellation cleanup happen at
   yield points too?

TODO(open): Tel needs a `with`-shaped construct that answers all five
questions consistently. The likely shape is a block that pairs an
acquisition with an exit action; the exit action runs on normal exit, on
panic, and on cancellation. Decide spelling alongside
[scope and bindings](../06-bindings-and-scope/).

## How it looks

```tel
# Race two mirrors with an overall deadline. Whichever loses, or
# overruns the deadline, is cancelled — and so is its subtree.
fn fetch_quote(mkts: List[Market], tasks: Tasks, clock: Clock) -> Quote {
    match run_with_timeout(clock, 500.millis, || race_first(mkts, |m| quote(m))) {
        Completed(q) => q,
        TimedOut     => Quote.stale(),
    }
}
```

## Bugs the cancellation model prevents

A few catalogue cases that shape the
design:

- **"Replay test stuck for >5 min because readers had no close signal."**
  A test waited out a long timeout because the underlying queue had no
  close. The follow-up bug: a sentinel was added too early, but other
  writers came in after the sentinel was put. Tel's channel close
  (see [channels](06-channels-and-message-passing.md)) and the
  structured shutdown rule (parent cancels children, then waits for
  cleanup) is the structural response.
- **"Force-restart Hazelcast node took down a neighbour."** A hard kill
  mid-operation cascaded. Graceful shutdown — reject new operations,
  finish in-flight ones, then exit — is what the structured-concurrency
  shutdown does for every task tree, with cancellation that propagates
  cleanly to children rather than yanking individual workers.
- **"Process load check skewed by debugger breakpoints."** A "should we
  pick up work" check used measured system load; in debug mode load was
  artificially low, so behaviour diverged. The injected `Clock` /
  `Load` capability story keeps the measurement under test control,
  rather than letting "real" system measurements leak.

## Open questions

- TODO(open): Names and signatures of `cancel`, `run_with_timeout`, and the
  timeout result type are illustrative.
- TODO(open): Whether a task can run **cleanup** as it is cancelled (a
  `defer`-style block, or scope-based resource release à la `with` /
  try-with-resources) and whether such cleanup may itself yield. This arises
  for host resources: can cleanup "safely unwind"?
  This needs a decision together with the resource-management story in
  [memory and runtime](../12-memory-and-runtime/) and
  [FFI](../16-ffi-and-interop/).
- TODO(open): Whether a task can *temporarily mask* cancellation to finish a
  critical step (an uninterruptible region). Useful but adds surface; the
  inputs do not raise it. Lean: omit unless a concrete need appears.
- TODO(open): What `join` on an already-cancelled task yields — a distinct
  `Cancelled` outcome, or an ordinary error. Lean: a distinct `Cancelled`
  variant, so callers can tell "stopped on request" from "failed".
- TODO(open): Whether timeouts can also be expressed directly on a blocking
  operation (e.g. a channel `send`/`receive` already takes a `timeout`
  argument — see [channels](06-channels-and-message-passing.md)). Reconcile
  per-operation timeouts with task-level `run_with_timeout` so there is
  [one obvious way](../02-philosophy/01-priorities.md).
