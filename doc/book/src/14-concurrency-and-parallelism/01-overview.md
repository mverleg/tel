# Concurrency and Parallelism Overview

<!-- TODO: review -->

Concurrency in Tel is built around one user-visible abstraction: the **task**.
A script asks the host to *spawn a task*; it never asks for a thread, a fiber,
or an async runtime. What "running a task" actually means is the host's
decision, not the script's.

## What

A **task** is a unit of work that can run alongside other work. Tel scripts
create tasks, combine them, race them, await their results, and cancel them.
That is the whole surface. Below it, the host runtime is free to map a task
onto:

- a fiber / green thread,
- an OS thread or a worker thread pool,
- a JavaScript microtask or event-loop callback,
- or **nothing at all** — running the task body inline, sequentially, on the
  caller's stack.

The last point is load-bearing: **a Tel script must behave correctly on a host
with no concurrency whatsoever.** Tasks express *that work may proceed
independently*, never *that work will run in parallel*. A script that depends
on two tasks truly overlapping in time is a buggy script.

## Why

Tel is a guest language. It runs inside game engines, browsers, backends, and
data pipelines, each with a different concurrency story — a browser bundle has
an event loop and no threads, a game engine has a fixed-tick scheduler, a
backend has a thread pool. Baking one concurrency model into the language
would make the same script behave differently, or fail to embed, depending on
the host.

So Tel exposes only the abstraction (`task`) and leaves the mechanism to the
host. This is the [host-portable concurrency](../02-philosophy/03-features.md)
feature, and it drives several deliberate omissions:

- **No first-class async, no function colouring.** There is no `async`/`await`
  split that infects function signatures. See
  [async and function colouring](03-async-and-function-colouring.md).
- **No threads, locks, atomics, or memory barriers in user code.** Those are
  machine-level concepts the host owns — see
  [antifeatures](../02-philosophy/04-antifeatures.md). User code reasons about
  tasks and values.
- **Per-task isolated heaps.** Tasks do not share mutable memory; data that
  crosses a task boundary is deep-copied. This makes data races structurally
  impossible — see [the memory model](07-memory-model-for-concurrency.md).

## The pieces

| Topic | Covers |
|---|---|
| [Tasks](02-tasks.md) | Spawning, joining, task trees, names, the spawn strategy, main-thread+workers shape |
| [Async and function colouring](03-async-and-function-colouring.md) | Why there is no `async`/`await`; `can-block` markers; stackful vs stackless |
| [Structured concurrency](04-structured-concurrency.md) | Task hierarchy, error propagation, fiber isolation, monitors, supervision |
| [Composing tasks](05-composing-tasks.md) | Combine, race, await-all, await-first, building task trees, dependency graphs |
| [Channels and message passing](06-channels-and-message-passing.md) | Closeable queues, the three-state model, channel dimensions, `select` |
| [Memory model for concurrency](07-memory-model-for-concurrency.md) | Isolated heaps, deep copy on transfer, mutable/immutable separation |
| [Cancellation and timeouts](08-cancellation-and-timeouts.md) | Cancelling tasks, deadlines, clock control for testing, cleanup on cancel |
| [Send and Sync](09-scoped-values.md) | When values can move between or be shared across tasks |
| [Locks and concurrency primitives](10-locks-and-concurrency-primitives.md) | `Mutex`, atomics, `RwLock`, `Once`, the rules they all share |

## How it looks

```tel
# `tasks` is a capability the host injects — no ambient runtime.
fn render_page(req: Request, tasks: Tasks) -> Page {
    # Spawn independent work. The host decides if these overlap.
    let ads = tasks.spawn("build-ads", || build_ads(req))
    let hits = tasks.spawn("search", || search(req))

    # Await both. Errors in either propagate here automatically.
    let (ad_links, results) = await_all(ads, hits)
    Page.of(ad_links, results)
}
```

Even on a host that runs the two task bodies one after another on the same
stack, this code produces the same `Page`.

## What concurrency is *for* in Tel

Tasks are used for three things, and Tel keeps them under one
abstraction rather than three separate features:

- **Overlapping I/O waits** — the "non-blocking IO" case. While one task waits
  for a network reply, another can run. On a fiber-capable host this is what
  the scheduler does; on a sequential host the wait simply blocks, which is
  still correct.
- **Spreading CPU work over cores** — the "parallel compute" case. A
  fan-out/fan-in over a list, a dependency graph of computations. On a
  worker-pool host this becomes real parallelism; elsewhere it runs inline.
- **Isolating failure** — the Erlang reason. A task that panics dies on its
  own, without taking the rest of the script with it. See
  [structured concurrency](04-structured-concurrency.md).

A Tel script does not pick between these — it just spawns tasks. The host
runtime decides whether a given task ends up on a fiber, a worker thread, or
inline.

## A note on the model the host is likely to run

Tel is deliberately host-agnostic, so any of the schedulers below is allowed.
But the *shape* the docs assume when describing
trade-offs is:

- One **main task** (often the script's entry task) that runs on the host's
  primary thread and does the orchestration. On a host with a single thread
  (a browser, a game tick) this is the only thread there is.
- A **worker pool** for CPU-heavy work, sized to roughly the host's core
  count, so the main thread is never blocked by long computation.
- For hosts with an event loop, blocking I/O on a worker thread; for hosts
  with a real async runtime, fibers multiplexed on the workers.

Scripts should not assume any of this and must run on a no-concurrency host
too. The shape is documented because some trade-offs (spawn cost, channel
capacity, copy cost) only make sense in light of it. The concrete scheduler
choices belong in the implementation notes, not the language docs.

## What is deliberately out of scope

Several adjacent topics are tempting but Tel rejects them from the
language surface; collected here so the reasoning is in one place:

- **SIMD, GPU compute, and similar accelerator sub-languages.** Layout
  constraints (contiguous, aligned, no pointer indirections) and
  hardware-specific operations belong in a host capability, not in Tel
  source. See [antifeatures](../02-philosophy/04-antifeatures.md).
- **Quantum-style execution.** Raised as a "does it fit?"
  question; the honest answer is no — entanglement, non-cloning,
  destructive measurement, and stochastic results all violate the
  assumptions that ordinary Tel code (value semantics, immutability,
  reproducibility) is built on. A quantum DSL is host territory.
- **Low-level lock-free constructions (the LMAX Disruptor, mechanical-
  sympathy ring buffers, cache-line padding).** Tel's channels are the
  general primitive; a host that needs microsecond message passing
  exposes its own.
- **Acquire/release, fences, memory ordering.** No barriers or per-op
  ordering knobs in user code; stdlib Sync types are sequentially
  consistent from the script's view.
- **Process-style isolation as a language feature.** "Tasks as
  processes" was explored (with the appeal of clean panic recovery), but
  the model Tel actually settles on is *task with isolated heap*, which
  gets the failure-isolation benefit without forcing process-level
  copying and IPC.

## Open questions

- TODO(open): The `Tasks` capability is shown here as a host-injected handle,
  consistent with Tel's capability model. Tasks are assumed to be
  spawnable, but whether spawning needs a capability at all is unpinned, or
  whether it is ambient. Capability-gating it fits the philosophy (the host
  controls what a script can do) and lets a no-concurrency host hand out a
  sequential implementation — but this is an inference, not a stated decision.
- TODO(open): Naming of the abstraction. Inputs use "task" and "fiber"
  loosely; "fiber" sometimes means the isolation unit (own heap, panics
  locally) and sometimes the scheduling unit. This documentation uses **task**
  throughout for the user-visible unit and reserves *fiber* for the
  host-side implementation choice. Confirm.
