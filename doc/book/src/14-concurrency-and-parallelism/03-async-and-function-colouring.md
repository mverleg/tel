# Async and Function Colouring

<!-- TODO: review -->

Tel has **no first-class async/await** and **no function colouring**. This is a
deliberate antifeature — see
[antifeatures](../02-philosophy/04-antifeatures.md). This topic explains what
that means, why Tel made the call, and what replaces async in practice.

## What "no function colouring" means

In languages with `async`/`await`, every function is one of two *colours*:

- a **sync** function — callable normally, and
- an **async** function — callable only from another async function, its
  result a `Future`/`Promise` you must `await`.

The colour leaks into every signature. Turning a sync function async forces
every caller to become async too, and a higher-order function that takes a
callback must pick a colour for it (`Fn` vs `async Fn`). The same split recurs
for other function attributes — `const fn`, `FnMut`, throwing vs non-throwing —
and each one multiplies how many ways a generic function must be written.

Tel rejects this. A Tel function has **one colour**. There is no `async`
keyword, no `await` operator forced into signatures, and no `Future[T]` type
the caller has to thread through.

## Why

- **Embedding.** Async/await is only meaningful with a runtime that drives
  futures. Tel's host might be a browser event loop, a thread pool, a
  game tick, or nothing. Baking in one async model would break embedding in
  the others. Concurrency is expressed as [tasks](02-tasks.md); the host
  decides what running a task means.
- **Stability and simplicity.** Function colouring is a viral signature
  concern. A frozen language ([Tel1](../02-philosophy/01-priorities.md)) cannot
  afford a feature that doubles the surface of every generic abstraction.
- **It is not needed.** Async/await exists mostly to make high-performance,
  non-blocking I/O ergonomic. That is neither a Tel priority nor easy to
  support across every target host — see
  [goals and non-goals](../01-overview/03-goals-and-non-goals.md). Tel scripts
  are small-to-medium guest code, not I/O-bound servers.
- **Forgotten futures are a real bug.** In colour-split languages it is easy to
  build a future and never await it — especially when a function silently
  becomes async and callers are not updated. Removing the explicit future
  removes the bug class.

## What replaces it

Two things together cover what async/await is normally used for:

1. **Tasks** for *doing things concurrently*. `spawn` + `join` (see
   [tasks](02-tasks.md)) — the host decides whether that is a fiber, a thread,
   or inline execution. A blocking call inside a task is fine: the host's
   scheduler, if it has one, parks the task; if it has none, the call simply
   blocks, which is correct behaviour on a sequential host.
2. **Suspension is the runtime's job, not the signature's.** Where a colour-split
   language writes `await`, Tel writes an ordinary call. If the host runs the
   task on a fiber, suspension at a blocking point is invisible to the script.
   This is the *fiber* model: the suspend points are real but not spelled in
   the source.

The result: code that *reads* like straight-line synchronous code, but that a
fiber-capable host can run concurrently, and a sequential host can run as-is.

```tel
# No `async`, no `await`. Just calls. The host decides whether
# `fetch` parks a fiber or blocks a thread.
fn enrich(id: Id[Order], store: Store) -> Order {
    let order = store.fetch(id)        # may block; that is fine
    let cust  = store.fetch(order.customer)
    order.with(customer_name = cust.name)
}
```

## No `Future` or `Promise` type — the task handle is the only awaitable

Tel **commits to having no general `Future[T]` / `Promise[T]` value type.** A
[task handle](02-tasks.md) *is* the future: spawning returns it, and awaiting it
(`join`) is an ordinary call that yields the value or its failure. There is no
constructible future you build, pass around, map over, and await separately from
a task.

This is a decision distinct from "no colouring," though it reinforces it. Even
decoupled from `async`/`await`, a free-standing future type would add a *second*
awaitable concept beside the task handle — against
[one good way](../02-philosophy/02-maxims.md) — and a constructible,
pass-around future tends to drag function colouring back in through the door
(`async fn`, viral `.await`). So Tel keeps exactly one awaitable: the handle.

### How a handle differs from a `Future`

A fair objection: *isn't a join handle just a future?* In the loose sense —
an opaque token for a not-yet-ready result you await — **yes**. What Tel drops is
the *general, constructible* future value; the handle is the deliberately narrow
subset, with four concrete differences:

- **You *get* a handle, you don't *build* one.** A `Future`/`Promise` is a
  first-class value with constructors and an algebra — `Promise.resolve(x)`, an
  `async {}` block, `f.map(...)`, a function *typed* to return one, a future of a
  future. A handle only ever comes out of `spawn`: no value constructor, no
  combinator that returns a new handle, no future algebra. It is a receipt, not a
  monad.
- **No colouring.** A `Future`-returning function is async-coloured and the
  colour is viral up the call graph. A function that spawns and joins internally
  is *not* coloured — `join` is an ordinary call, and concurrent work inside a
  function never changes its signature or forces anything on callers (the whole
  point of this chapter).
- **The work is already running; it is not lazy.** `spawn` schedules the work (or
  runs it inline) immediately — the handle is a receipt for work in flight. A
  Rust `Future` is *inert* until an executor polls it, so the future often *is*
  the computation. The handle is therefore close to Rust's **`JoinHandle`** (a
  future for an *already-spawned* task), **not** Rust's general `Future` trait —
  Tel keeps the former and drops the latter.
- **Structured and linear, not free-floating.** A `Future`/`Promise` is a loose
  value: drop it (silent fire-and-forget or silent cancel), clone it, share it to
  many awaiters, keep it alive indefinitely. A handle is a child in the
  [task tree](04-structured-concurrency.md) — it must be joined or auto-joined,
  cannot be detached, cannot outlive its parent, and is single-owner (which is
  why [fan-out](#what-about-the-cases-a-future-usually-covers) needs the `Sync`
  cell instead).

So the precise claim is not "Tel has nothing future-like": it is that the only
future-like thing is an opaque, single-shot, structured, uncoloured receipt for
already-spawned work — there is no general future *value* you construct, compose,
and thread through signatures.

### What about the cases a `Future` usually covers?

Walking the situations other languages reach for a future, almost all reduce to
"spawn a task and await its handle":

- **a timer or delay** — a task (or a capability call), composed with the task
  [combinators](05-composing-tasks.md) (`race`, `await_first`);
- **I/O completion** — the capability call returns the value, or you spawn a
  task that performs it;
- **select / race across several pending operations** — task handles already
  compose;
- **deferred *pure* computation** — that is [`Lazy[T]`](../05-types/05-function-types.md),
  laziness, not concurrency — a different axis entirely.

Only **two** needs are genuinely distinct from "await a handle", and both are
*producer / sharing* shapes served by existing `Sync` primitives, **not** by a
future value:

- **Resolve-later / first-writer-wins** — a cell someone *else* fills (bridging
  a host callback, a deferred hand-off). This is a settable one-shot:
  [`Once[T]` / `LazyInit[T]`](10-locks-and-concurrency-primitives.md#once--lazyinit)
  or a one-shot [channel](06-channels-and-message-passing.md). It is a write
  surface on a `Sync` cell, not an awaitable type of its own.
- **Fan-out await** — *many* consumers of *one* in-flight result. A task handle
  is [linear/single-owner](02-tasks.md), so it does not by itself serve many
  readers; the answer is the same settable `Sync` cell (or handing the in-flight
  task's result through one — see
  [the cache story](10-locks-and-concurrency-primitives.md#cache-and-lock-interaction-the-forkjoin--recursiveload-story)),
  read by all consumers.

### If uniform awaiting is ever wanted

If awaiting the task handle and the settable one-shot through *one* surface
proves worthwhile, the most Tel would add is a minimal internal **`Awaitable`
bound** — a trait the handle and the cell both satisfy, so `join` and the
combinators work over either. A bound is not a colour and not a pass-around
value, so it stays inside this antifeature. A user-constructible `Future[T]`
value is **rejected**.

TODO(open): whether the internal `Awaitable` bound is worth surfacing at all, or
whether the task handle alone is sufficient and the settable cell is awaited by
reading it. Lean: keep the handle as the sole awaitable; add the bound only if a
real combinator needs to range over both. See
[`inputs/futures-and-promises.md`](../../../inputs/futures-and-promises.md).

## `can-block` markers

Although functions are uncoloured, it is useful for a host scheduler — and for
humans — to know that a function *may block* (do I/O, wait on a lock, wait on a
long computation). This lets a scheduler keep blocking work off latency-critical
workers, or sort tasks into I/O-bound and CPU-bound pools.

Tel may therefore allow a function to be **marked `can-block`** (exact spelling
TBD). This is informational metadata, not a colour:

- It does **not** change the function's type and does **not** force callers to
  do anything — a `can-block` function is called exactly like any other.
- It is one-directional: a function *not* marked `can-block` is asserting it
  will not block; a function *marked* `can-block` *might*. There is no
  corresponding "this is heavy CPU work" guarantee in the other direction.
- A host scheduler may use it as a scheduling hint; a host without a scheduler
  ignores it.

A related idea is a `can-lock` marker, to help reason about deadlock — but lock
acquisition in Tel is confined to a few stdlib types
(see [the memory model](07-memory-model-for-concurrency.md)), so its value is
unclear.

TODO(open): Decide whether `can-block` ships at all, its exact spelling, and
whether it is checked (a non-`can-block` function calling a `can-block` one is
an error) or purely advisory. A *checked* marker risks becoming function
colouring through the back door — the very thing this topic rejects. Lean:
advisory only, or omit. Also decide `can-lock`'s fate (lean: omit).

## Stackful vs stackless, and what Tel says about it

Two implementation models for "suspend a task without blocking the thread"
recur:

- **Stackful coroutines / fibers.** Each task gets its own (small) stack. A
  blocking call inside the task parks the whole stack and lets the scheduler
  run another task. Recursion works as in ordinary code, no function colour is
  needed, and the suspension point is a runtime detail. Cost: real stacks
  to allocate and switch.
- **Stackless coroutines.** The compiler transforms an async function into a
  state machine, allocates the captured state on the heap, and resumes by
  re-entering it. No second stack, but every `await` point is part of the
  function's *type* — function colouring is the price of admission, and
  recursive async functions need explicit boxing.

Tel takes **no position** on which model a host uses, because the same script
must run under both. What it *does* say is that the user-visible language
cannot leak the choice:

- It cannot require an `async` keyword on every suspending function (that is
  a stackless-only concern).
- It cannot require recursive async functions to be boxed (a stackless-only
  workaround that a stackful host should not even see).
- It cannot rely on stackful-only abilities either — most relevantly,
  scripts must not rely on **deeply nested recursion** to express what
  iteration could. A host that compiles Tel to JavaScript or Wasm has no
  ergonomic fiber facility; a script must work there too.

The familiar conflict — "stackful coroutines are nice but can't be supported in
many languages, so use async" — is precisely what tasks resolve:
the *user* sees tasks, the *host* picks the implementation. A Tel-to-Wasm
backend may compile tasks to state machines; a Tel-on-JVM backend may use
virtual threads; a Tel-on-Lua backend may use coroutines.

TODO(open): Whether the docs make any *recommendation* to host implementers
(e.g. "prefer fiber-style if your runtime supports it; fall back to
stackless"). Lean: leave that to implementation notes.

## Effects, handlers, and "implicit arguments"

A richer story is possible: model "suspending", "logging", "failing",
and similar cross-cutting concerns as **algebraic effects** with handlers, in
the style of Koka. The appeal: a function that does logging or I/O lists those
effects in its signature, and a caller installs handlers without threading
state by hand.

This is **out of scope for Tel** as a language feature. The reasoning:

- Tel's [capabilities](../02-philosophy/03-features.md) already cover the
  motivating cases — logging, time, I/O are values handed in by the host, not
  ambient powers. A capability is just an argument; effect rows would only
  pay off if the language hid the threading, which conflicts with
  *readability over writability* and with the "no hidden suspend points"
  concern above.
- Effect systems are leading-edge language research; Tel is committed to
  conservatism (see [the priorities](../02-philosophy/01-priorities.md)).
- The trickiest effect — async — is already dissolved by the task /
  uncoloured-function design above; the rest do not need their own machinery.

The observation that **a closure must not capture the effect handler**
(it must resolve handlers on invocation) is real, but in Tel it reduces to
"a closure captures the capability *value* the same way it captures any other
argument" — there is no separate handler dimension to manage.

Koka's `ctl`/`resume` first-class continuations are another tempting option, where
an effect handler can store its `resume` continuation, call it later, or call
it more than once (turning a sampler into an enumerator, implementing
backtracking, building schedulers from primitives). This is firmly
**rejected** for Tel: re-invocable continuations are powerful but are also
arguably the deepest end of control-flow research, they interact badly with
linear/affine resources, they make stack semantics observable, and they
require either a tracing GC or sophisticated stack copying to implement
across all hosts Tel targets. Tel's only "suspend and resume" mechanism is a
task on a fiber, controlled by the host scheduler, and is invoked once per
suspension.

TODO(open): Some effect-style sub-features (a `yield`-based generator, an
external iterator built from internal iteration) might still be useful. Park
that with the [iteration / generators](../08-control-flow/04-for-loops-and-iteration.md) discussion rather
than here. Worth noting: external iterators built from internal iteration
(Ruby's lift via coroutines, Python's generators which made `await` a special
case of `yield from`) need either stackful coroutines or a state-machine
transform — same trade-off as async. If Tel grows generators, the host-side
implementation choices mirror the task ones.

## Async closures and async drop

Two ergonomic concerns arise about colour-split async, which Tel's
uncoloured-task model sidesteps but worth recording:

- **Async closures.** In colour-split languages, closures need their own
  async/non-async variants, and the trait hierarchy multiplies (in Rust:
  `Fn`/`FnMut`/`FnOnce` × `async`/`!async`, with each `async` variant
  effectively returning an opaque `Future`). Closures also can't capture
  values when modelled as plain function pointers, so a separate
  closure-with-async-body abstraction is needed at all. In Tel, a closure is
  just a closure; if its body calls a function that may park the task on a
  fiber, that is invisible at the closure's type.
- **Async drop / async cleanup.** Some resources need to do I/O on cleanup
  (flush a buffered writer, commit/rollback a database transaction, release a
  network lease). The canonical hard case is a database transaction: at scope
  exit it needs to either `commit` or `rollback`, both of which talk to the
  server, and `commit` can fail in ways the caller may want to handle. In
  colour-split languages this is a known hard problem because destructors are
  sync, can't `await`, and can't return errors. In Tel, cleanup runs at a
  scope boundary in ordinary code that can call anything the host allows, can
  block (the host parks the task), and can return a `Result` — so the
  transaction's cleanup is just code in a `with`-shaped block, not a special
  destructor. See [cancellation and timeouts](08-cancellation-and-timeouts.md)
  for the cleanup-on-cancel angle and a checklist of the questions every
  resource type must answer.

There is also the idea of **on-suspend / on-wake hooks** — a way for a value to
react to its task being parked and resumed (flushing a buffer when suspended,
re-checking an invariant on wake). These are interesting in a colour-split
world where suspension points are visible at the type level; in Tel they
would have to fire from inside the runtime around every fiber park/unpark,
and it is unclear what a script could usefully do at those moments without
also exposing the fiber model. Park as TODO(open): probably omitted.

## Rejected: automatic `await`

A middle path is conceivable — keep futures, but insert `await`
*automatically*, with a postfix marker (`@`, `&`, …) for the rare case where
you want to *defer* and build a lazy future instead. The argument: less
verbosity, fewer forgotten-future bugs, code that reads synchronous.

This is **not adopted**. It still requires a `Future[T]` type and a runtime to
drive it, so it does not solve the embedding problem — it only hides the
colour, it does not remove it. The fiber model achieves the same "reads
synchronous" outcome without a future type at all. Auto-`await` is recorded
here as a considered-and-rejected alternative.

TODO(open): A weaker version of the idea — a postfix operator that makes
*any* expression lazy (`Lazy[T]`), unifying lazy evaluation with deferred
tasks — might still have merit independent of async. That belongs with lazy
evaluation / closures, not here. Flagged so it is not lost.

## Async iterators and streams

A recurring shape in async-heavy languages is the **async iterator** or
**stream** — a sequence whose `next` may suspend. In colour-split languages
this is yet another shadow hierarchy (`Iterator` vs `AsyncIterator`,
`merge`/`zip` doubling, no async iteration syntax for a long time in Rust,
etc.).

Tel does not need a separate type. An iterator is a value whose `next`
returns the next item (or done); if that `next` may park the task on a
fiber, the caller does not have to care. Receiving from a
[channel](06-channels-and-message-passing.md) is the canonical "stream of
values that arrives over time" — its `for` loop yields each value and ends
when closed, with no `AsyncIterator` distinction. A pull-based iterator that
talks to a remote source uses the same shape.

TODO(open): Whether stdlib iterator combinators (`map`, `filter`, `take`)
work seamlessly over a channel as well as over a finite collection — i.e.
one iterator trait covers both. Lean: yes. The "stream" question dissolves
if the answer is yes.

## Generics over "might suspend"

Even without colouring, a generic higher-order function
may want to express "this combinator is concurrent iff its argument is". The
suggested default: *"`F` is concurrent if any generic argument is"*, inferred
rather than spelled. Because Tel has no `Future` type and tasks are uncoloured,
this mostly dissolves — a generic function just calls its argument. Recorded as
a TODO so the concern is not lost.

TODO(open): Confirm that uncoloured tasks fully remove the need for
"concurrency-polymorphic" generics. If a residue remains (e.g. a combinator
that must know whether to spawn), document it here.
