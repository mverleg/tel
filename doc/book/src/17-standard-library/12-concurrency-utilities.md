# Concurrency Utilities

<!-- TODO: review -->

## What

This topic covers the `std` building blocks for concurrent work — **channels**
and the **logging** facilities — and how they sit on top of Tel's task model.

The task model itself (spawning, joining, racing, cancellation, error
propagation) is a language-level concern; its full design lives in the
concurrency chapter. This topic only covers the `std` *utilities* and notes
where they depend on open task-model questions.

## Channels

Tel needs a **channel** — a queue with a `close`
operation. The motivating observation is that a plain blocking queue has only
two states, *empty* and *non-empty*, while a channel has three: *non-empty*,
*empty*, and *closed*. The third state is what lets a producer signal "no more
values" cleanly, instead of callers guessing.

```tel
# A producer fills a channel, then closes it; the consumer drains
# until close. Syntax is loose pseudocode.
fn produce(out: Channel[Job], jobs: List[Job]) {
    for job in jobs { out.send(job) }
    out.close()
}

fn consume(inp: Channel[Job]) {
    for job in inp {        # iteration ends when the channel is closed
        handle(job)
    }
}
```

Two design rules:

- **Every channel must specify what happens when it is full** — block, drop,
  fail — and what happens with slow consumers or producers. There is no
  unspecified default; the policy is part of constructing the channel.
- **A `send` has three outcomes** — delivered, full, closed — and there may be
  a timeout. There is a choice between returning a `Result` versus a callback
  for this; Tel's error model points at a `Result`-shaped outcome, propagated
  explicitly. `TODO(open): the result-vs-callback choice for channel
  operations is genuinely open; settle it with the error-handling
  chapter.`

Channels are the recommended way for long-running tasks to communicate;
closing the channel is how such a task is told to wind down.

`TODO(open): channels cross task boundaries, so their thread-safety depends on
the unresolved mutability/data-race model
([`../02-philosophy/04-antifeatures.md`](../02-philosophy/04-antifeatures.md)).
A channel of immutable values is safe to share; a channel of mutable builders
is not. Pin this down once the mutability model is settled.`

## Logging

Logging is a first-class, ergonomic part of the library,
built around four features:

- **Lazy arguments.** A log call's message arguments are evaluated only if the
  log level is enabled. A disabled `debug(...)` costs almost nothing, and the
  `if log.isDebugEnabled()` guard disappears. This builds on lazy
  string-building — see
  [`06-strings-and-text.md`](06-strings-and-text.md).

- **Implicit caller file and line.** A log call automatically captures the
  source file and line it was written at, with no boilerplate — no
  hand-rolled `OriginNamedThreadFactory`-style hacks. The file/line is an
  implicit argument the compiler supplies.

- **An implicitly-wired logger.** A logger is not per-class, so it does not
  belong as a field on every type. It is delivered through the injected
  *context* (see [`08-io-and-filesystem.md`](08-io-and-filesystem.md)) and
  wired automatically down the call tree. A function that logs need not name
  the logger in its signature.

  This stays inside the capability model: a logger is still a host-granted
  capability (the host decides whether, and where, log output goes — there is
  no ambient `stdout`). It is just one that is threaded implicitly rather than
  passed by hand. `TODO(open): "implicit context" for the logger needs the
  context mechanism designed; until then, treat the logger as an injected
  capability that may also be passed explicitly.`

- **Structured logs.** Beyond flat text lines, the library supports structured
  logging — marking the start and end of events, counts, traces, and
  parent/child relationships between events. `TODO(open): is
  structured logging core `std` or a separate library? Lean core,
  since it composes with the implicit logger and task tree.`

```tel
# Lazy args: build_report(...) runs only if `debug` is enabled.
# File and line are captured implicitly.
log.debug("generated report", { build_report(orders) })
```

Logging is also a target of editor support — a log call with a known template
can be **folded** into an example rendered message, and log statements can be
visually de-emphasised so the happy path stands out. See
[`../18-tooling/09-editor-integration.md`](../18-tooling/09-editor-integration.md).

## Tasks and scheduling

`std` exposes *tasks*, not threads — the host decides whether a task is a
fiber, a worker thread, or a sequential continuation
([`../02-philosophy/03-features.md`](../02-philosophy/03-features.md)). The
utilities here are deliberately host-portable: a channel and a logger work
even on a host with no real concurrency.

One recurring open question: a task that does **blocking
I/O** should ideally be schedulable off the CPU pool. A possible mechanism is
marking a function as "can block" (conceptually what `async` signals
elsewhere) so the scheduler can sort tasks into I/O-bound and CPU-bound.
`TODO(open): whether Tel exposes a "can block" marker, and how it interacts
with the no-function-colouring rule in
[`../02-philosophy/04-antifeatures.md`](../02-philosophy/04-antifeatures.md) —
a marker that infects every caller is exactly the colouring Tel rejects. The
worked example is in
[`../19-use-cases/01-hello-world.md`](../19-use-cases/01-hello-world.md).`

## Worker pools

Worker / task pools follow firm rules:

- **Configuration is easy and explicit** — the Java `ExecutorService`
  surface, with its half-dozen knobs spread across half-a-dozen
  constructors, is the anti-pattern. A pool is constructed with a single
  options record, every field defaulted to a reasonable value but
  visible in the source.
- **Pools always have a failure handler.** There is no "uncaught task
  exception is silently logged to stderr" default. The failure handler
  is part of construction, not retro-fitted.
- **Pools always have an explicit task queue** — bounded by default,
  with a stated full-policy (drop, block, fail). This is the same rule
  as for channels, applied to the pool's input.
- **Pools support both Python-style `map` and Java-style submission.** A
  blocking `pool.map(items, |x| ...)` keeps workers alive for the duration
  and gathers results; an `enqueue(task)` flavour returns a future.
- **Map-style submission supports "fail fast" or "collect all"** — when
  one item's task fails, the pool can either cancel siblings and bubble
  the error, or run to completion and return a list of
  `Result[T, E]`s. The choice is at the call site, not a pool-wide
  setting.
- **Shutdown has a timeout.** The pool's shutdown takes a duration and a
  policy: wait for tasks to finish, interrupt them, or both.
- **Task timeouts.** Every submitted task can carry a timeout; the pool
  enforces it against the injected `Clock`.

```tel
# Sketch — syntax not pinned down.
let pool = WorkerPool(
    workers       = 8,
    queue         = Queue(capacity = 1024, on_full = OnFull.Block),
    on_task_error = |err, task| log.error("task failed", task.id, err),
    shutdown      = Shutdown(grace = Duration.seconds(5),
                             then  = ShutdownPolicy.Interrupt),
)

let results = pool.map(orders, |o| score(o), on_error = OnError.FailFast)
pool.shutdown()
```

**Scheduling at intervals** is also a pool feature — a
pool that runs a task every N units of `Clock` time. The library exposes
this directly rather than asking every caller to build a timer loop. The
broader timing surface (cron, retries, debounce, throttle) is in
[`17-scheduling-and-timed-ops.md`](17-scheduling-and-timed-ops.md).

### Parallelism level

Parallelisation should usually happen at the **highest** sensible code
block — jobs over subtasks, subtasks over operations, not the other way
round — because lower-level functions don't know what level above already
went parallel. The library makes this easy by letting a function take a
pool argument; calling code passes either a real pool or a "sequential
pool" that runs work inline. A nested function takes whichever it is
handed and does not over-parallelise. `TODO(open): the "pass a sequential
pool" idiom needs ergonomic syntax; coordinate with the capabilities /
context story.`

## `select` across awaitables

A `select` lets a task wait on multiple awaitables — channels, network
reads, timeouts, cancellation signals — and act on whichever resolves
first. Two specialised forms are also useful:

- **`race.first(...)`** — completes when the first awaitable resolves,
  regardless of success or failure.
- **`race.first_ok(...)`** — completes when the first awaitable resolves
  *successfully*; failures are collected and only surface if every input
  fails.
- **`all(...)`** — completes when every awaitable resolves; an
  `all_streaming` variant yields each result as it lands.

`TODO(open): whether `select` is a language form (like Go) or a stdlib
function over a list of awaitables. The latter composes better with the
"one good way" rule but is harder to type-check around heterogeneous
awaitable types.`

## Mutexes, atomics, and barriers (not exposed)

Mutexes, atomics, and memory barriers are **not** part of Tel's
user-visible surface — they are exactly the "low-level machine access"
the antifeatures forbid
([`../02-philosophy/04-antifeatures.md`](../02-philosophy/04-antifeatures.md)).
Tel explicitly does *not* expose them. A script
shares state across tasks through:

- **Immutable values** — freely shareable by construction.
- **Channels** — explicit, observable, closeable.
- **Higher-level synchronisation primitives** the library provides as
  needed (e.g. a `WaitGroup`-style task barrier; a `Once` for one-shot
  initialisation).

`TODO(open): pre-pivot — re-justify against embedding. There are
RwLock vs Mutex trade-offs, atomic references, false-sharing
mitigations, cache-line alignment hints. Most are correct for a systems
language and wrong for Tel. Keep the *capability surface* small; any
need for fine-grained locking is a sign the work belongs in the host.
Decide whether even a high-level `WaitGroup` and `Once` belong in
`std`.`

## Shared mutable types (platform-conditional)

Tel's user code obeys "mutable ⇒ affine": you cannot define a type that is
both mutable and freely shareable across tasks. But some real workloads want
exactly that shape — a map that many workers read from and write to without
forwarding every operation through one owner. `std` provides a **small,
named set** of high-level shared-mutable primitives for those cases.

### What `std` provides

- `ConcurrentMap[K, V]` — a hash map safe for concurrent reads and writes.
- `ConcurrentSet[T]` — the set sibling.
- The cloneable `Sender[T]` from [Channels](#channels) — same shape: an
  Arc-like handle to a synchronised internal queue.
- `WaitGroup`, `Once`, `Barrier` — coarse coordination primitives.

These types are **`Send + Sync` stdlib primitives** with their own internal
synchronisation. The user does not write the synchronisation; it is part of
the type. The "mutable ⇒ affine" rule is lifted *only* for this named set —
user types remain bound by it (see
[the memory model](../14-concurrency-and-parallelism/07-memory-model-for-concurrency.md)).

TODO(open): final list and exact spellings.

### Why "platform-conditional"

Implementing these types needs either a genuinely shared region of memory
across worker threads or a global concurrent runtime. Not every host has
that:

- **JVM, .NET, native multi-threaded runtimes** — direct implementation.
- **Browser JS Workers, Dart isolates, per-instance Wasm linear memory** —
  heaps are isolated by design; a real shared concurrent map needs
  platform-level shared-memory features (e.g. `SharedArrayBuffer` +
  `Atomics`) or falls back to the actor-based alternative below.
- **Erlang/BEAM** — no shared mutable state in the platform. The actor-based
  alternative is the only option.

A host that cannot provide a primitive at all may **omit it from `std`** on
that target. The
[shared-heap rule](../14-concurrency-and-parallelism/07-memory-model-for-concurrency.md#shared-heap-is-never-required)
in the memory model is the reason every *other* part of the library stays
portable; this section is the named exception.

### The actor-based portable alternative

For cases where "concurrent map" really means *"many tasks coordinating on
keyed state"*, the **portable** answer is to spawn one task that owns the
data and to talk to it through a channel:

```tel
# Sketch — one owner task, queried by message.
let store = tasks.spawn("kv-store", || {
    let uniq my_map = Map[Text, Value]()
    for an_op in incoming {
        match an_op {
            Get k r => r.send(my_map.get(k))
            Put k v => my_map.insert(k, v)
        }
    }
})
```

This is the Erlang answer, and it works on *every* host — including the
heap-isolated ones — because it only uses channels and per-task heaps. The
trade-off is one serialisation point per coordination domain, which is the
right shape for most workloads and the wrong shape for a few (a hot path
with thousands of concurrent writers wants a real concurrent map).

`std` may expose a polymorphic interface so call sites stay identical:

```tel
# Same surface; the stdlib picks ConcurrentMap on capable hosts and an
# actor-backed implementation elsewhere.
let scores: SharedMap[PlayerId, Score] = std.sharedMap()
```

TODO(open): whether `std` ships such a polymorphic interface, or whether the
choice is left to library authors. The polymorphic interface is the
"portable by default" answer; the static choice is the "pay-for-what-you-pick"
answer. Lean: ship the polymorphic interface so the common case stays
portable, with concrete `ConcurrentMap` available where a host guarantees
it.

## Cancellation and supervision

Tel follows Erlang's "let it crash" model:

- A task that fails aborts its own subtree of work, not the whole
  program.
- A *supervisor* task chooses what to do with the failure — restart,
  skip, give up — based on a policy chosen at supervisor construction.
- Subtask trees form a hierarchy ("supervisor trees"); the parent knows
  about the children, the children don't have to know about the parent.

Closing a channel is the recommended way to ask a downstream task to
wind down; cancellation propagates through the awaitable graph, not
through stack unwinding (Tel has none — see
[antifeatures](../02-philosophy/04-antifeatures.md)).

`TODO(open): the supervisor-tree pattern is a *language*-level concern
that deserves its own topic in the concurrency chapter, not just an
`std` utility. Lift the design once the task model is firmed up.`

## See also

- [Strings and Text](06-strings-and-text.md) — lazy string building
- [I/O and Filesystem](08-io-and-filesystem.md) — capabilities and context
- [Scheduling and Timed Operations](17-scheduling-and-timed-ops.md) —
  cron, retry, debounce
- [Observability and Logging](14-observability-and-logging.md)
- [Disruptor Ring Buffer](../19-use-cases/06-disruptor-ring-buffer.md)
- [Editor Integration](../18-tooling/09-editor-integration.md)
