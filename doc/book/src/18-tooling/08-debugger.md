# Debugger

<!-- TODO: review -->

## What

The debugger is the surface for **pausing**, **inspecting**, and **stepping
through** a running Tel program — typically while testing locally, debugging
a failing test, or diagnosing an issue inside a host. It is exposed through
the host (most often, via the host's existing debugger UI: IDE, browser
devtools, terminal TUI) rather than as a stand-alone Tel binary.

Tel ships:

- A **debug information format** baked into xolir
  ([`02-compile-targets.md`](02-compile-targets.md)) so a backend can produce
  the right host-level debug symbols (DWARF for native, source maps for
  JS, `.class` line tables for the JVM, …).
- A **runtime protocol** the interpreter and AOT runtimes implement so any
  debugger client can pause, inspect, and step.
- A small set of **language-level hooks** — an explicit `breakpoint`
  expression, conditional breakpoints driven by [Result](../13-error-handling/03-error-propagation.md)
  values, stack-trace capture from any value.

It is not a separate language tool with its own UI; it is what lets the
host's existing debugger speak Tel.

## Why hand the debugger to the host

The maxim **embedding is the point** ([`../02-philosophy/02-maxims.md`](../02-philosophy/02-maxims.md))
applies as forcefully here as anywhere. A Tel script running inside a
Java host is being debugged by someone running that host in IntelliJ; one
running in a Unity game is being debugged in Unity's tooling; one in a web
backend is debugged through that runtime's tracer. A Tel-owned debugger UI
would be a second, worse window the user has to keep open. Tel's job is to
make sure its own program shape is *visible* to the host's existing tooling,
not to ship a new front-end.

This is the same reasoning that puts the
[build system](03-build-system.md) and
[concurrency model](../14-concurrency-and-parallelism/04-structured-concurrency.md)
behind the host: Tel owns the language, the host owns the surrounding
machinery.

## What the language ships

### A `breakpoint` expression

A first-class expression that pauses the program at that point when a
debugger is attached and is a no-op otherwise — a breakpoint command in
the spirit of JS's `debugger;`:

```tel
fn process(order: Order) -> Result[Receipt, Err] {
    let validated = validate(order)?
    breakpoint                            # pause here if debugger is attached
    let priced = price(validated)?
    Ok(receipt_for(priced))
}
```

`breakpoint` is the only language form needed; the editor-set breakpoints
(click-the-gutter) ride on top of the same runtime hook.

`TODO(open): exact spelling. JS's debugger; is the precedent; lean: a
plain identifier breakpoint, not a sigil.`

### Conditional and error-shaped breakpoints

Two sharp cases are "a breakpoint on a new `Err` being
returned" and "execution leaves this function anywhere before X." The
debugger protocol supports these as **conditional breakpoints over the
runtime's value stream**, not as new language syntax:

- *Break on any `Result::Err` constructed in this function.* Pure
  conditional breakpoint, set in the IDE.
- *Break on a `Result::Err` that is *propagated past* `?`.* The runtime
  exposes the propagation as a debug event the client subscribes to.
- *Break on any unobserved task failure.* Plumbed through the structured-
  concurrency machinery in
  [`../14-concurrency-and-parallelism/04-structured-concurrency.md`](../14-concurrency-and-parallelism/04-structured-concurrency.md).

Because errors are values ([`../13-error-handling/01-philosophy.md`](../13-error-handling/01-philosophy.md)),
these are all "break when this value flows past this point" — there is no
exception machinery to special-case.

### Inspection at the pause

When paused, the debugger client can:

- Read every binding in scope, formatted via the value's display trait.
- Walk into nested values without forcing evaluation of lazy expressions —
  inspection has no observable side effect.
- Read the **stack trace** in source-level terms (function names and
  line numbers from the original Tel source, not xolir node IDs). Async
  task spawning preserves the parent-task chain in the trace.
- Read the **capability set** the current task holds, so "this code can't
  reach the filesystem because no capability was passed in" is debuggable
  without guesswork.

It cannot:

- Mutate bindings unless the value's type is mutable and the binding is in
  scope — the same rules that apply to user code apply to the debugger.
- Bypass capability gates. A debugger session that wants to read a file the
  script wasn't granted access to has to get that capability the same way
  the script would.

These constraints keep the debugger from being a back-door capability
escalation.

## Auto-break on assertion or panic

When the runtime detects an assertion failure or aborts a task with `must`
/ `unimplemented` / `todo` (see
[`../17-standard-library/03-prelude.md`](../17-standard-library/03-prelude.md)),
and a debugger is attached, the runtime pauses at the abort site **before**
unwinding the task. This is also called out in
[`11-testing.md`](../14-testing/01-testing.md) — *auto-breakpoint on panic so a failing
test stops at the abort site instead of unwinding all the way out*. Without
this, the most useful state — the bindings at the moment of the panic — is
gone by the time the test reporter sees the failure.

## Stack traces as values

A stack trace is a regular Tel value: any code can capture the current
trace, attach it to a logged event, or include it in an error message. The
goal is that *all log messages have stack traces*, with a trace treated
as a serialisable artefact (Smalltalk stores the whole stack trace when
panicking as binary, which can be reloaded on another machine). Tel's
position:

- Capturing a trace is an ordinary function call exposed through the
  observability capability ([`../17-standard-library/14-observability-and-logging.md`](../17-standard-library/14-observability-and-logging.md)).
- The trace is structured (frames, source positions, capability set), not
  a pre-formatted string. Formatting is done by the consumer.
- A trace can be serialised, sent over a wire, and reloaded for inspection,
  subject to the host's privacy policy (a trace may carry source
  fragments).

## Debug vs release

There is **one runtime semantics** in Tel — see
[`01-compiler.md`](01-compiler.md). The debug story does not introduce a
second semantics:

- The debug *information* (line tables, scope metadata, capability metadata)
  is always available; a build can strip it for size, but the language
  spec is the same with or without.
- Assertions and contracts are checked according to their declared mode
  (see [philosophy on design-by-contract](../02-philosophy/03-features.md)),
  not according to a "debug build" flag. A *debug-only
  asserts* mode is rejected — what runs in dev must also run in prod, or
  it isn't really tested.
- A debugger attached or not changes whether `breakpoint` pauses, and nothing
  else. The program's observable behaviour is identical either way.

This is the same call as **no `#cfg`-style boolean flags** in
[`03-build-system.md`](03-build-system.md).

## Time-travel / step-back

Stepping a debugger *backwards* — record-and-replay so a task can be driven to
an earlier step — is **deferred**. Per-task heap isolation makes it tractable,
but journaling every capability call is expensive, so the language reserves the
runtime hook without committing to ship a recorder for every backend. The design
lives in
[Deferred Features → Record-and-replay debugging](../20-appendix/06-deferred-features.md#record-and-replay-time-travel-debugging).

## Tracing a run to a log

Separate from the *interactive* debugger is a **non-interactive trace**:
run one function with tracing turned on and get a log of *all* the steps it
took — bindings as they are set, branches taken, calls entered and returned,
capability calls made — written to a file instead of a paused UI. Where the
debugger answers *"let me stop and look,"* the trace answers *"show me
everything that happened, after the fact."*

```bash
tel trace --out run.log -- my_module.process '<order json>'
```

- The trace is the same value-stream the debugger subscribes to, drained to
  a file rather than to a client. No code change and no `breakpoint` is
  needed — tracing is a runtime mode, not an instrumentation the author adds.
- Output is **structured** (one record per step: source position, the
  bindings touched, the branch outcome), so a viewer can fold it, diff two
  runs, or render it as a tree. A plain-text rendering is the default; the
  structured form is what tools read.
- Because a single task's run is reconstructable from its inputs (seeded RNG,
  fixed clock, recorded capability results — the same determinism that makes
  [step-back](#time-travel-step-back) tractable), a trace **replays
  deterministically**: the log is a faithful record of one run, not a
  best-effort sampling like production [traces](../17-standard-library/14-observability-and-logging.md#traces-and-correlation).

This is the dev-time, exhaustive sibling of the sampled, always-on trace
spans in [observability](../17-standard-library/14-observability-and-logging.md):
that surface is for *production at scale* (sampled, cheap, cross-task); this
one is for *understanding a single run in full*. It is also the form most
useful to a reviewer or an AI assistant asking *"what did this function
actually do on this input"* — a complete, ordered log reads better than a
reconstructed guess.

`TODO(open): how much is "all" the steps — every binding and branch is a lot
of volume for a hot loop. Likely a depth/scope filter (trace this function
and its direct callees, or trace until a capability boundary). Decide the
default granularity and whether it shares the
[adjustable-detail](09-editor-integration.md#adjustable-detail--show-less-or-show-more)
levels. Also confirm `tel trace` is a real subcommand vs a flag on `tel run`.`

## What the debugger does *not* do

- **No live code reload.** Changing a function while the script is paused
  and resuming with the new version is rejected — see the
  [stability priority](../02-philosophy/01-priorities.md) and the
  no-reflection antifeature ([`../02-philosophy/04-antifeatures.md`](../02-philosophy/04-antifeatures.md)).
  Edit, rebuild, rerun. `TODO(open): re-examine. The "no live reload" stance was
  taken from the debugger angle; the use-case research (modding, model
  configuration, fast-iteration scientific workflows) flags hot reload as a
  recurring ask that should not be under-estimated. The question is whether Tel
  can offer a *constrained* reload story — replace pure functions, refuse to
  reload code holding live capability handles, preserve task heaps — without
  reopening the dynamic-dispatch / reflection door. Coordinate with the
  language-side decision on whether modules can be reloaded at all.`
- **No eval-in-debugger.** The incremental compiler can be attached
  to a paused session for *typed* expression evaluation, but there is no
  way to run a string as code from the debugger any more than there is
  anywhere else in Tel.
- **No silent capability escalation.** A debugger cannot grant the script
  capabilities the host did not grant it. (It can, of course, *show* the
  script being denied a capability, which is itself useful.)

## See also

- [Compiler](01-compiler.md) — the resource-bound mode and the never-panic
  rule that bound what a debugger can observe.
- [Compile Targets](02-compile-targets.md) — debug info rides on xolir.
- [Testing](../14-testing/01-testing.md) — auto-break on assertion / panic,
  fixed clock and seeded RNG for reproducible repro.
- [Editor Integration](09-editor-integration.md) — the IDE's debugger UI
  is the primary surface.
- [Standard Library: Observability](../17-standard-library/14-observability-and-logging.md)
  — stack traces and structured events as values.
- [Embedding Tel in a Host Application](../16-ffi-and-interop/04-embedding-tel-in-a-host.md)
  — the host wires the debugger protocol into its own debugger.
