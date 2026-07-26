# Error-Handling Philosophy

Tel's error handling rests on one decision: **errors are values**. A failure is
an ordinary value of a `Result`-shaped type, returned from the function that
detected it and handled like any other value. There are no exceptions, no
`throw`, no `catch`, no stack unwinding, no surprise control flow.

This page explains the model and the reasoning. The mechanics live in the rest
of the chapter:

- [Result types](02-result-types.md) — the shape of a fallible return value.
- [Error propagation](03-error-propagation.md) — passing failures up, terse but
  unforgettable.
- [Panics and aborts](04-panics-and-aborts.md) — the *abort* path for truly
  unexpected conditions.
- [Recovery](05-recovery.md) — the one place a failure can be contained: a
  task/fiber boundary.
- [The fallback operator](06-fallback-operator.md) — supplying a default for a
  `none` or missing value.

## Two kinds of failure

Tel separates failures into two categories, and treats them differently.

- **Expected failures** — an order is rejected, a lookup misses, input is
  malformed, a channel is full. These are part of the function's contract.
  They are returned as `Err` values through a `Result`-shaped type and handled
  explicitly. The caller *must* deal with them.
- **Unexpected failures** — a broken invariant, an unreachable branch reached,
  an `assert` violated, arithmetic that overflowed. These mean the program's
  assumptions are already wrong. They **abort** the current task. There is no
  catching them mid-work.

The maxim *crash by default; recover at the boundary, not in the middle of the
work* captures the split. Day-to-day error handling is `Result` values; the
abort path is the seatbelt for "this should never happen."

## Why errors-as-values, not exceptions

Returning errors as values, rather than throwing them, serves several
priorities at once:

- **Readability.** Every place a function can fail, and every place a failure
  leaves a function, is written in the source. There is no invisible control
  flow — *if it looks correct, it probably is correct*.
- **Safety.** The type system carries the failure: a `Result`-returning
  function cannot have its error case quietly ignored (see
  [error propagation](03-error-propagation.md)). The compiler tells you about
  the unhandled path before a user does.
- **Stability and portability.** Exceptions and unwinding are exactly the kind
  of machinery that behaves differently across host runtimes. A value returned
  up the call stack behaves identically whether Tel is interpreted or compiled,
  in a JVM host or a browser. *Same code, same results.*
- **One way.** Two parallel error channels — exceptions *and* return values —
  is two ways to do one thing. Tel keeps one.

The cost is some familiarity: a Java or Python reader expects `try`/`catch`.
The trade is made deliberately — see
[antifeatures](../02-philosophy/04-antifeatures.md).

## Results vs. callbacks

When a function has several possible outcomes — say, pushing to a channel that
can *succeed*, be *full*, be *closed*, or *time out* — there are two ways to let
the caller react: return a result the caller inspects, or take callbacks the
function invokes. Tel chooses **results**. The tradeoff, drawn from the design
notes:

**Results** (Tel's choice)

- An outcome is conceptually a value, so modelling it as one is honest.
- The caller keeps full control in its own scope: it can `return`, `break`, read
  and write its own local bindings, and decide everything itself.
- The risk is *forgetting to handle* the result when the happy path produces
  "nothing" — Tel addresses this by making a `Result` resist being silently
  dropped (see [error propagation](03-error-propagation.md)).
- A result can only be delivered once, at the end — it cannot model "called
  repeatedly" or "called before the function returns."

**Callbacks** (rejected as the primary mechanism)

- Hard to forget; easy to pass several handlers; can be invoked early or
  repeatedly.
- But a callback cannot `return` from, or `break` in, or touch the locals of
  the calling function — not without the
  [inline-lambda machinery](../08-control-flow/06-early-return.md#lambdas-and-the-enclosing-function-question)
  (an `outer return` / `outer break` from a block passed to an `inline` helper),
  which brings its own restriction: the block cannot escape.
- Control flow becomes hard to follow across several callback layers — directly
  against *readability over writability* and *surprise is a cost*.
- Every function that invokes callbacks must itself be careful about aborts.

Callbacks remain available as ordinary lambda arguments where a function
genuinely needs "do this for each item" or "do this when ready." They are just
not how Tel models *failure*. The few cases that need a repeated or early signal
(streaming, channels) are served by [generators](../08-control-flow/04-for-loops-and-iteration.md)
and channels, not by turning every fallible call into a callback.

## The bugs this is meant to prevent

The "errors are values, and they cannot be silently dropped" rule is a direct
response to a recurring family of production bugs the design wants to make
structurally impossible. A small selection from the catalogue:

- **Empty output persisted as if it were real state.** A fitting process
  failed and returned an empty value; downstream code had a path for *missing*
  state but not for *empty* state; the empty value got persisted and fed into
  the next run. With `Result`-shaped failures the next run sees `Err(...)` and
  has to deal with it, not a "successful empty" that looks just like normal
  data.
- **Background task crashed silently, leaving stale 0.0 in a sparse grid.** A
  pricing thread died unobserved; the grid kept reading `0.0` for any missing
  cell; downstream code treated that as a real price. Tel's tasks deliver a
  failure outcome to whoever joins them (see [recovery](05-recovery.md) and
  the [task model](../14-concurrency-and-parallelism/04-structured-concurrency.md)),
  and Tel pools always ship with a failure handler (see
  [`../17-standard-library/12-concurrency-utilities.md`](../17-standard-library/12-concurrency-utilities.md)).
- **Exception thrown by a builder vanished into the void.** A builder threw,
  the calling thread died, no monitoring picked it up. Same story: in Tel
  there is no thrown exception to lose, no "uncaught handler" to forget to
  install — a fallible step returns `Result` and the caller's signature
  reflects it.
- **Publisher silently dropped every message for hours.** A Kafka publisher
  could not register its schema at startup; it logged at startup, then never
  again, and silently dropped every subsequent publish. Tel pushes against
  this from two directions: a publish operation that *can* fail is a
  `Result`-returning call its caller must handle, and a long-running task
  whose failure surfaces only at the seam (rather than as ambient log noise)
  cannot be ignored downstream.

These are not separate features — they are the same rule (an error is never
dropped silently) appearing in different costumes.

## No stack traces by default

One thing a returned `Result` does *not* carry is a stack trace back to the
origin of the problem — and after a few layers of aggregation the original
context can be lost. Exceptions get this for free; values do not.

TODO(open): decide whether `Result` errors carry any origin/context
information — a captured source location, a cheap cause chain — and whether
that is debug-only. *Crash by default* means the abort path
([panics and aborts](04-panics-and-aborts.md)) is where rich diagnostics
belong; expected-error values should probably stay cheap. Confirm.

TODO: review — new section; the stack-trace/context question is the main open
point.
