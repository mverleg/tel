# Tasks

<!-- TODO: review -->

The **task** is the only concurrency primitive a Tel script sees. There are no
threads, no fibers, no futures-with-colour in user code — see
[the overview](01-overview.md) for why. This topic covers what a task is, how
one is spawned and joined, and how the runtime decides whether a task actually
runs in parallel.

## What

A task is a piece of work — usually a closure — handed to the host to run.
Spawning a task gives back a **task handle**: a value that represents the
work-in-progress and, eventually, its result.

```tel
let handle = tasks.spawn("fetch-prices", || fetch_prices(market))
# ... other work ...
let prices = handle.join()   # wait for the task to finish, yield its value
```

A task handle carries the *result type* of the work: `spawn` of a
`|| -> PriceTable` returns a handle whose `join` yields a `PriceTable`. If the
task body fails, that failure surfaces at `join` — see
[structured concurrency](04-structured-concurrency.md).

**The handle is the only awaitable.** Tel has no separate `Future` / `Promise`
type — the task handle *is* the future, and `join` is the await. The narrow
"resolve later" and "many readers of one result" cases are served by a settable
`Sync` cell ([`Once[T]`](10-locks-and-concurrency-primitives.md#once--lazyinit)),
not by a future value; see
[no `Future` or `Promise` type](03-async-and-function-colouring.md#no-future-or-promise-type--the-task-handle-is-the-only-awaitable).

### A task is a function call the host may run elsewhere

The simplest mental model: **a task is a function call.** It takes arguments,
captures values from the enclosing scope, runs a body, and produces a return
value. `spawn` differs from an ordinary call in only three ways — the host may
run the body *elsewhere*, *later*, or *inline*; the body runs against its own
[isolated heap](07-memory-model-for-concurrency.md); and the result comes back
through `join` rather than directly.

Because the body may run on another thread and against another heap, the values
that cross the boundary are constrained — the same constraints a
[channel](06-channels-and-message-passing.md) send obeys:

- **Arguments, captures, and the return value must be
  [`Send`](09-scoped-values.md).** They have to be movable to wherever the
  task runs. In practice that is almost everything — only **borrows**
  (`&T` / `&!T`; see
  [Why borrows are `not Send`](../12-memory-and-runtime/04-references-and-aliasing.md#why-borrows-are-not-send))
  and **thread-affine host resources** are `not Send`. A `Send` value is
  *copied* into the task's heap at spawn (semantically; an immutable value
  that is also `Sync` may be shared by reference instead — the script cannot
  tell); an affine mutable owned value is *moved* rather than copied. A
  `not Send` capture is rejected at compile time.
- **The handle is [relevant](../12-memory-and-runtime/08-substructural-types.md):
  it must be used.** A task's outcome — *including a failure* — cannot be
  silently dropped, because a task handle does not derive `Discard`. Either the
  handle is `join`ed, or the surrounding scope auto-joins it at close (the
  handle's `AutoUse`; see [structured concurrency](04-structured-concurrency.md)).
  This is what makes "failures are not forgettable" a type-level property rather
  than a convention.

### Tasks are the panic boundary

A panic — an unexpected, bug-signalling failure — is **confined to the task it
occurs in**; the rationale is in
[structured concurrency](04-structured-concurrency.md#fibers-fail-in-isolation).
The type-level consequence belongs here: **`join` reifies the panic as a
value.** Joining a task whose body might panic yields a `Result[R, PanicInfo]` —
`Ok(r)` if the body returned `r`, `Err(info)` if it panicked. The panic does not
unwind through the joiner; it arrives as data.

If the body is itself fallible, its own `Result[T, E]` *is* the `R` here, so the
two failure flavours stack: a fallible, panic-capable task joins to
`Result[Result[T, E], PanicInfo]` — an ordinary error is the body's value, a
panic is the outer layer. See
[error propagation](04-structured-concurrency.md#error-propagation).

Whether a task *can* panic is the
[`panics` effect](../05-types/05-function-types.md#effects-belong-on-the-function-type),
**not** a substructural type property: panic is an *effect*, delivered through a
default-on ambient [capability](../02-philosophy/03-features.md) — the same
machinery `alloc` uses — not an axis alongside `Send`/`Sync`. The task boundary
is where that effect is *handled*: `spawn`/`join` catches a panicking body and
reifies it as `Result[R, PanicInfo]`. A body that provably does not carry the
`panics` effect (a `pure`/`total` body, or one simply inferred non-panicking)
joins directly to `R`, with no `PanicInfo` wrapper — there is nothing to report.
The effect is inferred for concrete bodies and spelled only at a generic bound.

Tasks are *not* threads:

- They have **names** (a string passed at spawn time), the way Java threads do.
  A name shows up in logs, traces, and panic reports, so a failure points at a
  human-readable origin rather than an opaque worker id.
- They have an **isolated heap** — see
  [the memory model](07-memory-model-for-concurrency.md). Data captured by a
  task body or returned from it crosses a heap boundary and is deep-copied.
- They are **hierarchical** — a spawned task is a *child* of the task that
  spawned it. Cancelling or failing the parent cancels the children. See
  [structured concurrency](04-structured-concurrency.md).

## Join and detach

A task handle has two possible fates: **join** it, or **detach** it (spawn and
never join). They are not interchangeable — which fates are *available* depends
on what the task captured and what it returns.

Take the deliberately narrow model first: a task that **cannot be cancelled**
and has **no yield points** — it starts, runs its body to completion, and
returns. This strips the picture down to the part that is not about scheduling.
The fuller model layers cancellation and cooperative yielding on top (see
[cancellation and timeouts](08-cancellation-and-timeouts.md)), but the two
conditions below are unchanged by that; only the meaning of "join blocks" gets
richer.

**Join blocks.** With no cancellation and no cooperative yield, `join` genuinely
parks the caller until the body returns — there is no early-abort path. Join is:

- the **only** option when the task **borrows** parent data. Blocking-until-done
  is exactly what bounds the borrow: the borrowed value cannot be moved or
  dropped while the handle is live, and the scope cannot close until the join
  releases it (see
  [scoped borrowing](04-structured-concurrency.md#borrowing-in-a-scoped-task));
  and
- the way to recover the **result** — including a
  [reified panic](#tasks-are-the-panic-boundary).

**Detach** is admissible only when *both* of the things that make join
necessary are absent — and they are absent for different reasons, which is the
part worth being precise about:

- **No borrows — a *soundness* requirement.** A detached task can outlive the
  frame that spawned it, so everything it holds must be self-contained: `Send`
  and copied/moved in at spawn, never a borrow of the parent (`&T` / `&!T`,
  which are [`not Send`](07-memory-model-for-concurrency.md) anyway). If it
  borrowed, detaching would let the borrow dangle — nothing forces the parent
  to wait. This is the hard constraint; the type system rejects a violation.
- **No relevant result — a *usefulness* requirement, not a soundness one.** You
  *can* detach a task that returns a value; you just silently drop that value,
  *including a failure*. So this condition is not about safety — it is about
  whether detaching throws away something you needed. Detach is sensible exactly
  when the result type is
  [`Discard`](../12-memory-and-runtime/08-substructural-types.md) — unit, or a
  value whose loss is intended.

In Tel's substructural terms these collapse into one statement. A task handle is
[relevant — it must be used](#what) precisely because a result or failure should
not be dropped. **Detach is the case where the handle would derive `Discard`**:
no borrow to bound *and* no relevant outcome to lose, so "must be used" relaxes
to "may be forgotten." The borrow condition is what keeps this from being a mere
convenience — it is why detach cannot be offered unconditionally.

TODO(open): This carveout sits uneasily beside the structured-concurrency rule
that there are [no detached spawns](04-structured-concurrency.md#the-task-tree).
Reconcile the two. Lean: keep the tree — "detach" means *the enclosing scope
auto-joins the handle for you and drops its unit result*, not that the task
escapes its parent's lifetime. Under that reading a `Discard`-result, no-borrow
task still drains at scope close, so "no task outlives its parent" holds and the
only thing detach removes is the obligation to name the `join` and inspect the
result. The alternative — a truly free-floating task — reintroduces the leak and
lost-failure problems the tree exists to prevent, so it is the weaker option.
Decide together with the auto-join open question below and in
[structured concurrency](04-structured-concurrency.md).

## Why tasks, not threads

Three reasons, all rooted in [the priorities](../02-philosophy/01-priorities.md):

1. **Portability.** A thread is an OS concept. A browser host has none; a
   game-engine host has a tick loop, not a pool. A *task* is an abstraction the
   host can satisfy however it likes — including by running the body inline.
2. **Safety.** Threads invite shared mutable state and the data races that come
   with it. Tasks have isolated heaps, so a script *cannot* express a data
   race (see [the memory model](07-memory-model-for-concurrency.md)).
3. **Better failure defaults.** A thread that throws an uncaught exception
   tends to die silently. A task that fails delivers that failure to whoever
   joins it — failure is a value, not a lost signal.

## Tasks vs fibers, virtual threads, and goroutines

"Task" is not a synonym for goroutine, virtual thread, or fiber. Those are
*implementation mechanisms a host may map a task onto* (see
[the overview](01-overview.md)), not the user-visible unit — a Tel-on-JVM
backend may use virtual threads, a Tel-on-Lua backend coroutines, a Tel-to-Wasm
backend stackless state machines. The differences that matter to a script:

| | goroutine / virtual thread / fiber | Tel task |
|---|---|---|
| **Runs concurrently?** | yes — a real independent thread of execution | *maybe* — `spawn` is a request; a host may run it inline and sequentially |
| **Memory** | shares one heap / address space | [isolated heap](07-memory-model-for-concurrency.md); data copied (or shared, if immutable) at the boundary |
| **Lifetime** | detached (`go f()` returns nothing) | child of the spawner; [cannot outlive its parent](04-structured-concurrency.md) |
| **Failure** | dies on its own, easy to lose | delivered at `join` as `Result[R, PanicInfo]` |
| **Stack model** | stackful by definition | host's choice — stackful fiber, stackless state machine, or inline |

So a Tel task is closest to a goroutine or virtual thread that has been
deliberately stripped of its two strongest guarantees — *real concurrency* and a
*shared address space* — in exchange for host-portability and structural
race-freedom, then placed in a mandatory parent-child tree. A script that relies
on two tasks genuinely overlapping in time, or on tasks sharing mutable memory,
is relying on something a Tel task does not promise.

## How a task gets scheduled

Spawning a task is a *request*, not a command. The host runtime decides:

- run it on a worker thread (genuine parallelism),
- run it on a fiber multiplexed onto one thread (concurrency, no parallelism),
- queue it as an event-loop callback,
- or run the body **immediately and inline**, so `spawn` returns an
  already-completed handle.

A correct script does not care which happens. It must produce the same result
whether tasks overlap or run strictly one after another. If two tasks must not
interleave, that is expressed by *not making them separate tasks* — or by
ordering them with `join` — never by assuming the scheduler.

### The spawn strategy

Spawning has a cost: an isolated heap, a deep copy of captured data, scheduler
bookkeeping. Spawning one task per tiny work item can cost more than it saves.
Note also that simply combining lazy results does **not** make anything
parallel — work only runs concurrently once it is actually spawned as a task.

Tel's guidance — and the likely default for stdlib combinators that fan work
out (see [composing tasks](05-composing-tasks.md)) — is a **tapering** strategy:

> Spawn the first *N* work items as real tasks, then spawn progressively
> fewer (say every *k*-th item), and run the remaining items inline on the
> caller.

This gets parallelism onto the available workers quickly without flooding the
scheduler with millions of trivial tasks, and it degrades cleanly to "run
everything inline" on a host with no concurrency. A complementary heuristic:
prefer to spawn coarse work *high up* in a call tree rather than fine-grained
work deep inside it.

```tel
# A fan-out combinator decides per item whether to spawn or inline.
# The script just expresses "these are independent".
let scores = parallel_map(orders, |o| expensive_score(o))
```

The strategy is a *runtime quality-of-implementation* concern, not observable
behaviour: a host may pick any strategy, including spawning everything or
nothing.

### I/O-bound and CPU-bound tasks

One option is distinguishing **I/O-bound** from **CPU-bound** tasks
explicitly (a `spawn_io` / `spawn_cpu` split) so a host scheduler can keep
CPU-heavy work off latency-critical queues. Tel rejects making this a *typed*
split — it would be function colouring through the back door, and a single
script would have to pre-decide a property the host might know better.

Instead, Tel treats this as a **scheduler hint**, the same way
[`can-block`](03-async-and-function-colouring.md#can-block-markers) is a hint.
A spawned task body that is marked `can-block` (or annotated as expected to do
I/O) lets the scheduler bias it toward an I/O queue; an unmarked task is
assumed CPU-bound by default. A host without that distinction ignores the
hint. There is one `spawn`.

```tel
# Plain spawn — host decides where it runs.
let h = tasks.spawn("score", || score(order))

# Hint: this task will mostly wait on the host's I/O.
let h = tasks.spawn_blocking("fetch", || http.get(url))
```

A motivating case: a host language with **no non-blocking I/O
facility at all** (the host can only block a thread on a `read`). On such a
host the runtime's sensible strategy is to **reserve one or more dedicated I/O
threads** for blocking calls — even oversubscribe them, so dozens of blocking
reads can be parked simultaneously without choking the CPU workers. The
`spawn_blocking` hint tells the scheduler "this body is a candidate for the
blocking pool"; on a host with real async I/O the same hint is ignored and the
body runs on the normal queue. The script is identical either way.

TODO(open): Whether the distinction is two methods (`spawn` / `spawn_blocking`),
one method with a flag, or just a marker on the body. Lean: one `spawn` plus an
optional `blocking = true` hint, so the simple case stays simple. Also decide
whether the compiler should ever *infer* the hint from a function's `can-block`
metadata — convenient, but it ties the spawn API to a metadata feature whose
fate is still open.

### Heuristics, not promises

A few scheduling rules of thumb, none of them user-visible
guarantees, but all of them shaping the trade-offs:

- **Work stealing** is the default expectation when a host has worker threads.
  Idle workers steal *unscheduled* tasks from busy queues. The unit of
  migration is the task body and its captures, which must be `Send`; in
  ordinary code that is everything except borrows and host-affine resources
  (see [the memory model](07-memory-model-for-concurrency.md)).
- **I/O queues are preferred over CPU queues** when both are runnable. I/O
  tasks tend to be shorter and latency-sensitive; CPU tasks happily wait a few
  milliseconds.
- **One queue per worker, plus a small global I/O fallback** on hosts with
  enough cores to spare one. Details belong in implementation notes.
- **Thread-per-core, no migration of in-flight work.** A worker thread runs
  pinned to a core, taking from a local queue and (under work-stealing) from
  peers' queues. A task that has already started keeps running on the worker
  that picked it up — the runtime does not migrate it mid-flight. This is what
  lets a host treat a worker's local heap and caches as the task's natural
  home. The point is that the *unit of migration is the unscheduled task*, not
  the running coroutine state.
- **Eventual progress / fairness.** A task that is *not* making computational
  progress — parked on a channel, a lock, or a host wait — **should** yield so
  other runnable tasks proceed, and a host **may** preempt even a CPU-bound task
  at safepoints. No host can guarantee this universally: a purely sequential
  host that runs a non-terminating task inline starves everything else. So it is
  an *expectation*, not a promise — but cooperative concurrency is almost
  unusable without it, so a host *with* a scheduler is expected to provide at
  least the not-progressing-yields half; the may-preempt-anyway half is a
  quality-of-implementation bonus.
- **Cheap, plentiful tasks are an explicit goal.** A script should be able to
  spawn thousands of small tasks without measurable cost — Tel's tasks are not
  OS threads. The tapering spawn strategy above covers the case where even
  cheap tasks become too many.
- **Scheduling visibility.** A scheduler that knows *which tasks are blocked
  on what* — e.g. a worker waiting on a closed channel can be parked, a worker
  waiting on a `Mutex` can have its priority adjusted — schedules better than
  one that has to wake everyone and let them re-check. Tel's stdlib
  concurrency primitives (channels, locks; see
  [locks and concurrency primitives](10-locks-and-concurrency-primitives.md))
  are expected to participate in scheduler hints, so a single global
  scheduler can decide what is runnable without busy-waking idle tasks. None
  of this is observable in user code.

These are descriptions of what a reasonable host does, not commitments. A
sequential host ignores them all and runs every task body inline.

## Task-local origin information

Diagnosing concurrent code needs to know *where* a task came from. A task
should be able to report the source file and line of its `spawn` call, so a
failure or a slow-task warning can name its origin without manual plumbing
(the alternative is host-side hacks like a named-thread-factory).

TODO(open): The mechanism for capturing call-site file/line is unspecified.
Options: an implicit caller-location parameter (Rust's
`#[track_caller]` style) or a compiler-injected argument. This overlaps with
the same need in logging; decide once and apply consistently. Until then,
treat the task *name* as the primary origin marker.

## How it looks

```tel
fn process(batch: Batch, tasks: Tasks, log: Log) -> Summary {
    # Spawn a named child task.
    let totals = tasks.spawn("sum-batch", || sum_all(batch.rows))

    # Do other work concurrently with it.
    let header = build_header(batch.meta)

    # Join: wait for the child, get its result. A failure in the
    # child surfaces here, not silently.
    let row_totals = totals.join()

    log.info("processed", batch.id, row_totals.count)
    Summary.of(header, row_totals)
}
```

## The main task

The entry point of a Tel script — what the host invokes when it runs the
script — is itself a task: the **root task**. Every other task is a descendant
of it. Because tasks are always children of *some* task (no detached spawns,
see [structured concurrency](04-structured-concurrency.md)), the root task is
also the outermost cancellation and error boundary the script sees; the host
sees the result of joining it.

There is **no separate "async main" vs "sync main"** the script has to declare
— consistent with [no function colouring](03-async-and-function-colouring.md).
The root task body is just a function the host calls with the capabilities it
wants to expose; whether the runtime drives it on a fiber, a thread pool, or
inline is the host's call.

```tel
# Sketch of an entry point. `tasks`, `clock`, etc. are capabilities
# the host hands in — nothing is ambient.
fn main(input: Input, tasks: Tasks, clock: Clock) -> Output {
    # ... spawn children of the root task here ...
}
```

The same applies to **tests**: a `test` is a task the test runner spawns,
joins, and inspects. A test that wants to control timing supplies its own
[`TestClock`](08-cancellation-and-timeouts.md#clock-control-for-testing).

TODO(open): Confirm the root-task model — in particular, that a script cannot
return *before* its outstanding children are done. If the "process
ends if main reaches the end but stuff is in the queue?" question is answered
"no — the script waits for the task tree to drain," document it here. The
embedding philosophy points that way: the host expects a definite end, not a
script that keeps running invisible work.

## Open questions

- TODO(open): Exact spelling of `spawn`, `join`, and the task-handle type.
  Names here are illustrative.
- TODO(open): Whether `join` is always explicit or whether a handle can be
  *auto-joined* on first use of its value ("await at first use"). There is
  an implicit-tree idea — see
  [async and function colouring](03-async-and-function-colouring.md) and
  [composing tasks](05-composing-tasks.md).
- TODO(open): Whether a task handle is a linear/affine value that must be
  joined (so a spawned task cannot be silently forgotten). The principle that
  "failures are not forgettable" weighs strongly here; structured concurrency
  (auto-join children at scope exit) may make linearity unnecessary. See
  [structured concurrency](04-structured-concurrency.md).
- TODO(open): Reconciling [join and detach](#join-and-detach) with the
  no-detached-spawn rule — whether "detach" is a scope-auto-joins-for-you
  convenience (leaning yes) or a genuinely free-floating task (leaning no). Tied
  to the linearity/auto-join question above.
- TODO(open): The tapering spawn strategy is described as guidance. Whether the
  stdlib exposes a knob (target task count, min work-per-task) or keeps it
  fully implicit is undecided.
- TODO(open): Whether the host can read back "task did I/O for X, was active
  for Y" — a way to *time how long a task was active*
  (sum of running spans, not wall-clock from spawn to join, distinguishing
  *running* time from *suspended-waiting* time). Useful for diagnostics on a
  fiber-capable host where wall-clock time wildly overstates CPU use. Either a
  stdlib observability hook or a host-side capability; not a language feature.
- TODO(open): Whether spawning the **current** thread/worker is included in
  combinators like `combine` and `await_all` — for fan-outs with little
  orchestration overhead, running one branch inline on the caller can save a
  full hand-off. Lean yes for low-orchestration cases; this is
  a quality-of-implementation choice, not a guarantee.
- TODO(open): The I/O-hint / `spawn_blocking` shape (see above).
- TODO(open): The `PanicInfo` type and how a non-panicking body is spelled are
  unsettled. Panic is the `panics`
  [effect](../05-types/05-function-types.md#effects-belong-on-the-function-type),
  implemented as a default-on ambient capability (like `alloc`), so "this task
  cannot panic" is the *absence* of the `panics` effect (`pure`/`total`), not a
  separate `Send`/`Sync`-style marker. Decide with the
  [error-handling](../13-error-handling/) model and the effect system whether
  `join` ever yields a bare `R` (when the body is effect-inferred non-panicking)
  or always `Result[R, PanicInfo]` with the no-panic case an optimisation only.
- TODO(open): Whether Tel supports a **task that never returns** — a background
  loop typed as returning [`Never`](../05-types/14-never-type.md) (the
  never type, `!`). Such a task can only end by cancellation or panic, never by
  a normal `join`. It is hard to *rule out* — the halting problem means the
  compiler cannot reject all non-terminating bodies anyway — and making it
  first-class (spawning a `|| -> Never` body, joinable only for its `PanicInfo`)
  might be more ergonomic than today's "long-running task drained via channel
  close" pattern (see
  [structured concurrency](04-structured-concurrency.md#how-long-running-tasks-fit)).
  Gating it on the `Never` return type keeps "I meant this to run forever"
  explicit rather than accidental. Depends on the fairness expectation above: a
  never-returning task is only useful on a host that lets other tasks progress.
