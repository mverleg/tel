# Recovery

An [abort](04-panics-and-aborts.md) ends a task — but it should not always end
the *program*. A web server should survive one bad request; a test runner should
survive one failing test; a pipeline should survive one bad message. Tel allows
exactly one form of recovery from an abort: **at a task (fiber) boundary**.

## Recovery only at a boundary

Ordinary Tel code cannot catch an abort — there is no `try`/`catch` (see
[panics and aborts](04-panics-and-aborts.md)). The single exception is a
**task**: a unit of work spawned to run with its own isolated state.

If a task aborts, the abort does **not** propagate as unwinding through its
internals. Instead the *whole task* fails as a unit. The code that spawned it —
its parent — observes that failure when it **joins** the task: the join yields
a failure outcome instead of a value. The parent may then log it, retry, call a
handler, or fail in turn. The program need not come down.

```tel
# Each request runs in its own task. One request aborting does not
# take down the server loop.
for request in incoming {
    let task = spawn { handle(request) }
    match task.join() {
        Ok(response) -> send(response),
        Err(failure) -> log.error("request aborted", request.id, failure),
    }
}
```

This is the maxim *crash by default; recover at the boundary, not in the middle
of the work*. Inside the work there is no recovery; at the boundary there is.

## Why the Erlang-style model

The design is Erlang-flavoured: **isolated failure**. A task has its own heap;
when it aborts, that heap is discarded as a whole and the task object is
dropped. The reasoning:

- **Crash-by-default is the safe default.** An unexpected failure stops the
  faulty work immediately rather than limping on over invalid state.
- **But total recovery is possible at a defined seam.** A failed request, a
  failed fit, a failed test — contained, without bringing down the host or the
  other tasks.
- **No fancy cleanup.** Because each task owns its heap, an aborted task is
  cleaned up by throwing the whole heap away — no unwinding, no per-value
  destructors, no half-initialised state to repair. This is the *same* property
  that lets [`panic = abort`](04-panics-and-aborts.md) keep
  [must-consume values](03-error-propagation.md) tractable.
- **Monitors are natural.** Because a parent observes a child's failure at
  join, supervisor/monitor patterns — restart on failure, escalate after N
  failures — fall out of ordinary code, the way they do in Erlang.

## Recovery is not error handling

Catching a task abort is a **last-resort backstop**, not the way to handle
expected failures. Expected failures are [`Result` values](02-result-types.md)
and are handled inline; the task boundary is for the *unexpected* — a bug, a
violated invariant, a `todo` reached. If you find yourself spawning a task just
to "catch" an error you could have returned as an `Err`, model it as an `Err`
instead.

## Tasks still propagate failure by default

A spawned task is not a fire-and-forget daemon. Its failure is **not** silently
swallowed: by default it surfaces at join, exactly as an
[error propagates](03-error-propagation.md) up a call chain. There is no
default daemon mode: a task whose failure nobody observes is a bug, not a
quietly-ignored event. The old "uncaught exception silently
kills the thread" behaviour is precisely what Tel avoids.

## Open questions

The recovery model leans on the task/concurrency design, which is not yet
settled. Several points are owned by the concurrency chapter and only sketched
here:

- TODO(open): the exact API — `spawn`, `join`, what a join of a failed task
  yields (an `Err` carrying a failure descriptor? how much context?), and
  whether a monitor/handler can be registered separately from `join`.
- TODO(open): whether aborting a parent task aborts its children (hierarchical
  tasks — killing a parent kills children), and at which point.
- TODO(open): whether a task abort can be distinguished, at the join site, from
  an ordinary `Err` the task chose to return — they should *not* be conflated;
  an abort means a bug, an `Err` means a handled outcome.
- TODO(open): what context a join-side failure carries. This is the same
  stack-trace/origin question raised in [philosophy](01-philosophy.md): a
  contained abort is the natural place for a richer diagnostic, since it marks a
  real bug.

TODO: review — new section; recovery depends on the unfinished concurrency
design and the points above must be reconciled with chapter 14.
