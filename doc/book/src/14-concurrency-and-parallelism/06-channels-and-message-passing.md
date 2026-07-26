# Channels and Message Passing

<!-- TODO: review -->

[Tasks](02-tasks.md) have [isolated heaps](07-memory-model-for-concurrency.md) —
they cannot share mutable memory. So tasks that need to communicate while
running do it by **passing messages**, and the primitive for that is the
**channel**.

## What

A channel is a **closeable queue**. One or more tasks push values in; one or
more tasks pull values out. Conceptually it is a queue with a `close`
operation — and that single extra operation is what makes it the right
communication primitive.

The key design point: a channel has **three
states**, not two.

| State | Meaning | A receiver gets |
|---|---|---|
| **Non-empty** | values are queued | the next value |
| **Empty** | no values *right now*, but the channel is still open | waits for more |
| **Closed** | no values, and no more will ever arrive | a definite "done" |

A plain blocking queue only distinguishes *non-empty* from *empty*, which
cannot answer "is more coming, or are we finished?" — receivers are left
polling or guessing. The closed state turns that question into a normal,
typed outcome. (This is the exact gap Java leaves: the lack of
a closeable queue forces awkward sentinel values; Go's channel, which *is* a
closeable queue, does not have the problem.)

## Why

- **It makes "we are done" expressible.** A loop draining a channel ends
  cleanly when the channel closes — no poison-pill sentinel, no separate
  "finished" flag racing the data. This is also the idiomatic way to stop a
  long-running worker task (see
  [structured concurrency](04-structured-concurrency.md)): close its input
  channel and its receive loop ends.
- **It is the only sound way to share data between live tasks.** Because heaps
  are isolated, there is no shared mutable structure to coordinate through.
  A channel transfers *values* — each send deep-copies the value across the
  heap boundary (see [the memory model](07-memory-model-for-concurrency.md)),
  so sender and receiver never alias the same mutable object.
- **It composes with structured concurrency.** A channel is an ordinary value;
  closing it is an ordinary, explicit operation, so a receive loop has a clear
  termination condition that the task tree can reason about.

## Portability — channels work on every host

A channel is a **core primitive every host must implement** (see
[Platform Layer](../17-standard-library/02-platform-layer.md#core-primitives)),
not a feature that may be missing on some platforms. The minimum obligation is
an **SPSC closeable queue**; the MPMC and cloneable-`Sender` shapes are built
on top, backed by the platform's concurrent primitives where available (see
[Shared mutable types](../17-standard-library/12-concurrency-utilities.md#shared-mutable-types-platform-conditional)).

The implementation strategy varies; the user-visible semantics do not:

- **Multi-threaded native / JVM / .NET** — the runtime provides a
  lock-free or locked queue; sender and receiver may run on different
  threads.
- **Erlang / BEAM** — each process has a mailbox, owned by the runtime,
  reached through the platform's send primitive (`!`). Tel channels map
  onto mailboxes or onto stdlib-level queues built from them.
- **Browser JS Workers / Dart isolates** — the host's `postMessage`
  between isolates is the primitive; Tel layers the channel API on top,
  with the send-time copy being either the host's structured-clone or
  Tel's own deep copy.
- **Single-threaded Wasm or any single-threaded host** — no concurrency,
  but channels are still valid: sending appends to a queue and receiving
  pulls from it, deterministically within the one thread. A receive that
  would block on an empty open channel surfaces as a runtime deadlock that
  the structured-concurrency layer can report — it is not silently
  forgotten.
- **Wasm with `SharedArrayBuffer` + atomics** — channels can be backed by
  shared memory; on Wasm hosts without that, the same script falls back to
  the isolate model via instance-to-instance message passing.

In every case, sending a value across a channel is the **same memory-model
operation as crossing a task boundary** at spawn or join (see
[the memory model](07-memory-model-for-concurrency.md)): immutable values
share or copy indistinguishably, affine mutable owned values move, mutable
values the sender keeps using are deep-copied, borrows are refused at compile
time. **No part of the channel API requires a shared heap** — the
shared-heap rule in the memory model
([Shared heap is never required](07-memory-model-for-concurrency.md#shared-heap-is-never-required))
applies here too.

## Receiving: a typed three-way outcome

Because the empty-vs-closed distinction is real, a *receive* is not a value or
a null — it is a three-way result the caller must handle. A `for` loop over a
channel is the common sugar: it yields each value and ends when the channel
closes.

```tel
# Drain a channel until it closes.
for msg in inbox {
    handle(msg)
}
# falls through here once `inbox` is closed and drained

# The explicit form, when you need to tell empty from closed.
match inbox.receive() {
    Got(msg)  => handle(msg),
    Empty     => wait_or_do_other_work(),
    Closed    => stop(),
}
```

## Sending: bounded, and a send can fail

A channel needs an explicit policy for what happens when it is **full** and for
**slow consumers or producers** — this is not left implicit. Sending therefore
has more than one outcome, modelled explicitly as a result:
a send can **succeed**, find the channel **full**, or find it **closed**. There
may also be a **timeout**.

```tel
match outbox.send(value, timeout = 50.millis) {
    Sent      => continue,
    Full      => apply_backpressure(),
    Closed    => stop_producing(),
    TimedOut  => apply_backpressure(),
}
```

Modelling the send outcome as a value (rather than a thrown exception or a
silently dropped message) keeps it in line with Tel's
[error handling](../13-error-handling/): the unhappy paths are visible in the
type and cannot be forgotten.

TODO(open): result-style vs callback-style for exactly this
"send can succeed / be full / be closed / time out" case is unsettled.
Tel's [no-exceptions, errors-are-values](../02-philosophy/04-antifeatures.md)
stance points firmly at the result form shown above; this documentation adopts
it. Confirm, and decide whether `send` on a closed channel is an ordinary
`Closed` result or a panic (lean: ordinary result — closing is normal).

## Capacity and overflow policy

Every channel **must specify what it does when full** and how it treats slow
peers — there is no silent default. Likely options:

- **Bounded, blocking** — a full send waits (the common backpressure choice).
- **Bounded, rejecting** — a full send returns `Full` immediately.
- **Rendezvous** (capacity zero) — a send completes only when a receiver takes
  the value.

TODO(open): The exact menu of capacity/overflow policies, their names, and
whether *unbounded* channels are allowed at all. An unbounded channel hides
backpressure problems and can exhaust memory; the insistence that
overflow behaviour be explicit argues against an unbounded default. Decide.

## Channel dimensions

The design space has several orthogonal axes. A given
channel type picks a point in each:

| Dimension | Choices |
|---|---|
| **Readers** | 1 (single-consumer) or N (multi-consumer) |
| **Writers** | 1 (single-producer) or N (multi-producer) |
| **Capacity** | 0 (rendezvous), bounded, unbounded — see above |
| **Blocking on send** | yes (waits) or no (returns `Full`) |
| **Blocking on receive** | yes (waits) or no (returns `Empty`) |
| **Who closes** | one designated side, any sender, the last sender, never |
| **Order** | ordered (FIFO) by default |

Ordering is **FIFO** by default. There is no known
use case where dropping ordering is a meaningful win, so unordered channels
are not part of the core. A workload that genuinely does not care about
order (a worker pool draining a queue) gets the same throughput from an
ordered channel — the receivers consume in whatever order the scheduler
gives them.

Channels are not one type with N flags. The likely shape is **a small set of
concrete types** (single-producer-single-consumer, multi-producer-multi-
consumer, rendezvous) — each picking a coherent set of choices, so a script
chooses a channel by its intended use, not by ticking flags.

TODO(open): The exact set of channel types, their names, and how many of the
axes above become separate types vs constructor parameters. *Every* channel
must answer all of these questions; the surface remains unsettled. Lean: a small number of named types covers the common
cases, with constructor parameters for capacity.

TODO(open): Multi-producer close rule. Either *any* producer can close (and
subsequent sends from other producers fail) or *the last live producer*
closes implicitly when it goes out of scope. The latter composes better with
structured concurrency — when a fan-out subtree finishes, its downstream
channel closes on its own — but it ties channel lifetime to handle counting.
This is unsettled.

## Long-running workers

The channel is the backbone of the long-running-task pattern that
[structured concurrency](04-structured-concurrency.md) leaves open. A worker
task loops receiving from its input channel; the task ends when that channel
closes. Closing the channel is thus both the "no more work" signal and the
clean shutdown mechanism — no separate stop flag, no cancellation needed for
the normal case.

```tel
fn worker(jobs: Channel[Job], results: Channel[Done]) {
    for job in jobs {            # ends when `jobs` is closed
        results.send(run(job))
    }
    results.close()              # propagate "done" downstream
}
```

## Decoupling producers and consumers

A subtle point: a producer-consumer pair where **both
sides do I/O** can end up running *sequentially* if naively connected. The
producer reads, sends one value, blocks on the channel; the consumer
receives, processes (does its own I/O), then signals readiness; only then
does the producer get to read again. The wall-clock looks like
`io_p → send → io_c → io_p → send → io_c → …` — no overlap.

Buffering helps a little (the producer can stage a few values ahead), but
the real fix is to make sure each side gets its own task so the scheduler
can interleave their I/O. Closing a producer and consumer "in the same task
with a queue between them" is a common mistake; the fan-out structures in
[composing tasks](05-composing-tasks.md) avoid it because each branch is
spawned independently.

## Channels as resource gates

A bounded channel doubles as a **resource pool**: pre-populate it with a
fixed number of *handles* (database connections, GPU contexts, file
descriptors), receive to acquire, send back to release. A consumer that
needs a handle waits on `receive`; when N consumers are already using all N
handles, the (N+1)th waits — same as a semaphore, expressed as ordinary
channel ops.

```tel
# Hand out database connections. The pool capacity is the channel's
# initial count, not a separate limit.
fn with_conn[R](pool: Channel[Conn], body: |Conn| -> R) -> R {
    let conn = pool.receive()
    let result = body(conn)
    pool.send(conn)
    result
}
```

Consider the **connection-pool starvation** failure mode: a host
program with 64 DB connections that uses them from an async backend can,
without care, end up with more than 64 in-flight "acquire" calls and
deadlock. The fix is exactly the gate above — `receive` parks the caller
until a handle is free. Because the gate uses a channel and not a hidden
semaphore, the wait point is visible in the source.

TODO(open): Whether the standard library offers a dedicated `Pool[T]` /
`Semaphore` type or leaves the pattern to channel ops. A named type would
clarify intent at the cost of one more primitive; the operations are the
same either way.

## `select` and multi-channel waiting

Languages with channels almost always need a way to wait on **more than one
channel at once** — pick whichever has a value, or pair a receive with a
timeout. Go and Kotlin both spell this `select`, apparently to good effect.

In Tel the same job is partially covered by `await_first` on
[composing tasks](05-composing-tasks.md): wrap each channel's `receive` in
its own task and race them. That works for an *ad-hoc* multi-wait, but is
heavier than necessary for the common "loop forever, handle whichever
channel is ready" worker shape, and it duplicates close-detection logic.

TODO(open): Decide whether channels get a dedicated `select` construct, or
whether `await_first` on per-channel receive tasks is the official answer.
A `select`-style construct would have to match exhaustively over the
possible receivers (each branch types its message), which fits the rest of
the language; the cost is one more primitive that overlaps with
`await_first`. Lean: provide one, because the pattern is too common to make
users build it from scratch.

## What about the Disruptor / lock-free ring buffers?

The LMAX Disruptor and similar lock-free ring buffers can outperform a
generic queue by an order of magnitude in extreme low-latency settings,
because they exploit cache layout and avoid kernel arbitration. This is
relevant for "pipelines with multiple stages."

This is **out of scope** for the Tel surface. The Disruptor wins by trading
generality for hardware sympathy (fixed-size ring, mechanical-sympathy
padding, single-writer principle on each cursor) — exactly the kind of
low-level control Tel deliberately keeps out of user code (see
[antifeatures](../02-philosophy/04-antifeatures.md)). A host that needs
microsecond-latency message passing exposes its own pipeline as a
capability; ordinary Tel scripts use channels and accept the queue cost.

## Bugs this prevents

A few catalogue cases that drive the design:

- **"OOM in the wrong service."** A heavy service appeared to be running out
  of memory; the actual culprit was a different service in the same process
  whose queue was overfull and had no metrics. Tel pushes back two ways: a
  channel is bounded by default with a stated full-policy (so the queue
  cannot grow without limit), and the stdlib expects channels to report size
  and drops as standard metrics (see
  [`../17-standard-library/04-core-collections.md`](../17-standard-library/04-core-collections.md)).
- **"Connection-pool starvation."** A program with 64 database connections
  ran out because more than 64 async callers were waiting to acquire one;
  the pool deadlocked. Using a channel as the resource gate (see
  *Channels as resource gates* above) makes the wait point visible and the
  bound enforced.
- **"Replay test ran for >5 minutes because there's no way to close the
  queue."** Without a closeable queue, readers had to poll past a long
  timeout to know they were done. With a three-state channel, the producer
  closes when done and the receiver loop ends immediately. The variant the
  catalogue records — *closing too early* because multiple producers raced
  with the close — is exactly why the chapter flags multi-producer close
  semantics as an open call rather than papering over it.
- **"Producer and consumer ran sequentially because they shared a thread."**
  See *Decoupling producers and consumers* above — the same catalogue case
  is what motivates that section.
- **"GUI overloaded by an event that fires every run."** A subscriber to a
  GUI update channel started receiving updates every run instead of on
  certain events, swamping the rendering layer. The fix in the catalogue
  was throttling per event type; in Tel this is the
  [throttle/debounce/rate-limit](../17-standard-library/17-scheduling-and-timed-ops.md)
  family applied to a channel adapter.

## Open questions

- TODO(open): Channel type spelling, and whether send/receive are methods or
  free functions. Names here are illustrative.
- TODO(open): Multi-producer / multi-consumer support — allowed for all
  channels, or a distinct channel kind? Closing semantics with multiple
  producers (does any producer close it, or the last?) — see the *Channel
  dimensions* table above.
- TODO(open): Whether `select`-style waiting on several channels at once is in
  scope — see above. Lean: yes.
- TODO(open): What "return value" channels look like, if anything — letting
  a task `return` a value cleanly may need a dedicated
  result channel, which can hurt ergonomics. For now, a task's result is
  delivered via its [handle's `join`](02-tasks.md), and channels carry
  *streamed* messages only. Confirm this split.
- TODO(open): Whether a dedicated `Pool[T]` / `Semaphore` ships in stdlib —
  see *Channels as resource gates* above.
