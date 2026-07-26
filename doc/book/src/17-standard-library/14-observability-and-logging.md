# Observability and Logging

<!-- TODO: review -->

## What

`std` treats observability as a core concern, not an afterthought. The
library exposes one coherent surface for **logging**, **structured
events**, **traces**, and **metrics**, all reached
through capabilities the host grants — there is no ambient `stdout`,
no global statsd client, no implicit OpenTelemetry exporter.

(Benchmarks, flame graphs, and heap dumps are a *performance-testing*
concern, not a logging one, and live with the tests — see
[Testing: benchmarks and profiling](../14-testing/01-testing.md#benchmarks-and-profiling).)

The basic logging story lives in
[`12-concurrency-utilities.md`](12-concurrency-utilities.md) (lazy args,
implicit logger, captured file/line); this topic covers the wider
observability surface and the design decisions behind it.

## Why: large systems live and die by observability

*The largest problem with programming in large
systems is reproducing and debugging.* Tel itself targets small-to-medium
scripts, but those scripts run inside hosts that handle real traffic and
real failures. A script that cannot be observed in production is a
script that has to be observed by re-running it in a debugger — usually
on the wrong machine, usually too late.

The observability surface is therefore *built in*, not bolted on. The
two design rules:

- **Cheap when off, useful when on.** Lazy arguments and sampling mean a
  disabled `debug` or an unsampled trace costs almost nothing. A
  production system can leave observability on permanently.
- **Same shape across hosts.** Whether the host pipes events to
  OpenTelemetry, to a local file, to a syslog daemon, or to /dev/null,
  the script writes them the same way.

## Logging: features

The four-feature core (lazy args, implicit file/line, implicitly-wired
logger, structured logs) is described in
[`12-concurrency-utilities.md`](12-concurrency-utilities.md). This topic
fills in the *rendering* and *transport* side.

### Color and pretty printing

A log call against an injected logger may render with ANSI color codes
when the sink is a terminal, and without when it is a file or remote
collector. The script does not pick; the *logger capability* does. The
input also asks for a top-level `log_pretty(value)` (or similar) that
pretty-prints lists, maps, JSON, deeply nested values — the everyday
`println(JSON.toString(...))` pattern as a single named function.

### Severity and folding

Severities are a small fixed set: `trace`, `debug`, `info`, `warn`,
`error`. There are no custom severities — keeping the set fixed lets
sinks reason about them and lets the editor visually de-emphasise the
low-severity calls so the happy path stands out (see
[`../18-tooling/09-editor-integration.md`](../18-tooling/09-editor-integration.md)).

`TODO(open): whether `fatal` (or similar) is in the set, alongside the
abort helpers in [`03-prelude.md`](03-prelude.md). Most languages have
it; Tel's preference for aborting at the boundary suggests `error` is
enough.`

## Source context: `here`

`std` exposes a small `here` function that returns the **source context** of the
call site as a plain value:

- current [crate](../11-modules-and-packages/04-packages.md) and its version,
- current filename,
- current line number.

```tel
let ctx = here()
log.info("starting", ctx)     # -> starting  (pricing 1.4.2  rules.tel:88)
```

The crate/version half is the same data
[`crate.info()`](../11-modules-and-packages/04-packages.md#crate-metadata-from-code)
returns; `here` adds the *location* (file and line) of the call itself. It is
all **resolved at compile time** — there is no runtime stack-walking or
reflection (see [antifeatures](../02-philosophy/04-antifeatures.md)), so it
stays compatible with Tel's no-`eval` model.

`here` is the explicit form of the **implicit file/line capture** the logger
already does (see
[`12-concurrency-utilities.md`](12-concurrency-utilities.md)). The intent is
that a logging or assertion library can declare a parameter that **auto-injects
the caller's `here`**, so a log call records *where it was written* without the
caller passing anything — the everyday "log line knows its own location"
behaviour, made a first-class value rather than a logger-only trick.

`TODO(open): the mechanism for auto-injecting the caller's `here` into a callee
parameter is not designed. Candidates: a default-argument value that evaluates
at the *call* site (not the definition site), or a dedicated parameter marker.
This must not become a general macro/reflection hook — it is one fixed,
compile-time substitution. Decide its spelling alongside the logger's existing
file/line capture so the two are one feature, not two.`

`TODO(open): exact return shape (a `Context` record? its field names?) and
whether the build identifier from `crate.info()` is included. Keep the field
set small and fixed.`

## Structured / nested logs

Logs are **nested** by default, mirroring the
call-stack or supervision tree, so a reader can fold into the section
where a problem occurred. Because the logger is a capability, the nesting
should **mirror lexical scope**: opening a sub-logger takes a block, and
inside that block the sub-logger *shadows* the outer one (a block parameter is
an explicit binding site, so this
[shadow](../06-bindings-and-scope/04-shadowing.md) is allowed), so ordinary code
just writes `log` and gets the right level automatically.

```tel
# `log` is the implicitly-wired logger from the context.
let imports = log.sub("import") |log| {
    # `log` here is the "import" sub-logger, shadowing the outer one.
    log.info("everything is going well")
    log.sub("read file") |log| {
        read_file()
    }
    log.error("problem!")           # bubbles up severity to "import"
}
log.sub("save data") |log| {
    save_data()
}
```

This is preferable to the older thread-the-sub-logger-by-hand style
(`let import_log = log.sub("import"); start_import(import_log)`), which
forced every nested call to take and pass a `Logger` parameter. The
scope-mirroring form has one cost: a plain block would make it harder to
`return` / `break` out of the surrounding function, or to assign to an outer
binding. `log.sub` resolves this by being an
**[inline](../08-control-flow/06-early-return.md#lambdas-and-the-enclosing-function-question)**
function: the block is spliced into the caller, so a bare `return` stays local to
the block while an explicit **`outer return` / `outer break`** leaves the
surrounding function (see
[closures and lambdas](../09-functions/06-closures-and-lambdas.md#lambda-return)).

`log.sub` is where the two DSL features meet. It uses **`inline`** for the
control-flow half (`outer` jumps leave the enclosing function) and a
**[receiver](../09-functions/06-closures-and-lambdas.md#lambda-receivers)** for
the context half. The named `|log|` binding above is the *explicit* receiver
form, chosen deliberately so nested calls read `log.info(...)` and match the
shadowed outer `log`; the implicit-`this` form (bare `info(...)`) is available
but reads worse for logging. Because the block is `inline` it is non-escaping —
exactly the condition TIP-0010 requires for a receiver block to keep its `outer`
powers, so the two features compose cleanly here.

`TODO(open): confirm the final sub-logger surface — the named `|log|` binding
shown here versus an implicit-`this` receiver — and how aggregate-severity
bubbling attaches to the block. The control-flow semantics (`inline` + `outer`)
and the receiver rule are settled; only the surface spelling is open.`

The structure is a tree of named blocks containing message records and
nested blocks. The aggregate severity of a block is the worst severity
of any descendant — useful for collapsed views. A rendered output (text,
JSON, terminal with folding) preserves the structure:

```text
import [ERROR]
  everything is going well
  read file [...]
  problem!
save data [...]
```

Implementation note: the nesting is also a natural way to encode
**trace spans** — each `log.sub(name)` is a span, with the file/line
captured by the lazy-arg machinery filling in the source location. So
"nested logging" and "tracing" are two views of the same underlying
event tree, not parallel features.

`TODO(open): whether spans carry IDs explicitly (for correlation with
external trace systems like OpenTelemetry) or only implicitly through
nesting. The library should make the OpenTelemetry-compatible mapping
trivial without hard-coding the wire format.`

## Metrics

Metrics — counters, gauges, histograms — are first-class, not bolted on
as logging conventions. The library exposes:

- **`Counter`** — a monotonic count, incremented at named call sites.
- **`Gauge`** — a current value, set or adjusted.
- **`Histogram`** — a distribution, sampled per observation.
- **`Timer`** — a histogram specialised to durations, with a `time(||
  ...)` block that measures and records.

Every metric is named, optionally labelled by enum-typed keys (no free
string labels — those leak high-cardinality data into the sink), and
reported through a metrics capability. The same rule as logging
applies: a host that wires the capability to a real backend gets real
metrics; one that wires `/dev/null` gets quiet code.

```tel
let parse_errors = Counter("parse_errors", labels = [Source])
parse_errors.label(Source.Twitter).inc()

let request_time = Timer("request_time")
let result = request_time.time(|| http.get(url))
```

### API standard, minimal built-in impl

`std` cannot actually *ingest* metrics — that needs external infrastructure
(a Prometheus endpoint, an OTLP collector). What `std` owns is the **API
types** plus a **minimal default implementation** that an external crate can
swap out:

- **Compatible with common standards.** The metric and trace types are shaped
  to map cleanly onto [OpenTelemetry](https://opentelemetry.io/) and similar
  providers, so wiring a real exporter is a capability swap, not a rewrite.
- **A minimal built-in impl.** Out of the box `std` provides a small, low-cost
  implementation (e.g. in-process aggregation, periodic flush) so code is
  observable with no dependencies; the host replaces it with a real backend by
  granting a different metrics capability.
- **Built-in by default for channels and tasks.** Because these types are in
  `std`, channels and tasks can carry standard depth/throughput/latency metrics
  by default, rather than each program reinventing them.

Because these types must be **closed/flushed** correctly, [spans are a natural
fit for linear types](#traces-and-correlation) — the must-use obligation makes
"forgot to end the span" a compile error.

`TODO(open): a cluster of design calls to settle together for the default impl:`

- *Type relationships.* Could `Timer` be a special use of `Histogram` (rather
  than a separate type), and `Counter` a special use of `Gauge`? Lean toward
  fewer underlying types with convenience wrappers.
- *Histogram buckets.* What determines a histogram's buckets — fixed, caller-
  declared, or auto-adjusting?
- *Default resolution.* For the built-in impl, what is the default report
  cadence for gauges/histograms (every ~30s)?
- *Concurrency.* Do all metrics send to a channel by default, with one task
  draining and aggregating them? That keeps the hot path lock-free.
- *Wrong-scope guard.* Can anything stop a metric being created in the wrong
  scope — e.g. intended per-region but actually instantiated per-user? Tie the
  metric's identity to its declaration site so it cannot multiply per call.
- *Initialisation.* Wiring the metrics capability must fix enabledness, report
  frequency, and reporting destination up front.

`TODO(open): *Java Flight Recorder*-style custom events
would be nice. A `CustomEvent` type — schema-described, recorded, queryable —
overlaps with structured logging; decide whether they're one feature
or two. Lean: one feature, with metrics, events, and structured logs as
*projections* of the same underlying record stream.`

## Traces and correlation

A *trace* is the cross-task version of nested logs: a logical operation
that spans multiple tasks (a fanout-and-collate, an RPC) is one trace,
each task a span within it. The library exposes:

- **`Trace.start(name)`** — begin a new trace; returns a context the
  caller propagates.
- **`span(name)`** — open a sub-span within the current trace.
- **Automatic propagation** — when a task is spawned with a trace
  context in scope, the child inherits it. The injected context (see
  [`08-io-and-filesystem.md`](08-io-and-filesystem.md)) carries the
  current trace ID.

**Spans are linear.** A span *must* be closed exactly once, which is precisely
the [relevant/linear obligation](../12-memory-and-runtime/08-substructural-types.md):
making `Span` a linear type turns "forgot to end the span" and "ended it twice"
into compile errors, and gives the type system the close-on-scope-exit guarantee
without a runtime finaliser.

**Spans are structured** — a span opened inside another is its child, so a trace
is a tree, not a flat list. **Distributed** spans (across processes/services)
need a correlation ID carried on the wire so a child span in another system
joins the parent's tree; the API surface should make that propagation explicit
and OTel-compatible rather than hidden.

Trace sampling is the answer to "we cannot afford to record everything"
in high-traffic systems. Sampling is a
capability-level choice — the host decides — not a script-level one.

`TODO(open): the structured/distributed span model — how a correlation ID is
generated, carried across a task or process boundary, and reconciled with the
nested-logging event tree above. Pin down with the context-propagation story.`

The exhaustive, *unsampled* counterpart is a dev-time tool, not a library
surface: [`tel trace`](../18-tooling/08-debugger.md#tracing-a-run-to-a-log)
runs a single function and records *every* step to a log. These traces here
are for production at scale (sampled, cheap, cross-task); that one is for
understanding one run in full.

`TODO(open): the relationship between traces and structured logs is
real but unspecified. Pin it down once the context-propagation story is
designed.`

## Plottable / graphable data

`TODO(open): **Portable graph/plot output.** Scripts in the scientific and
data-analysis use cases routinely want to *show* a series or a table —
not just log it. Tel should not ship a graphing backend (a renderer is the
host's job and a poor fit for an embedded language), but a thin, portable
**graph-spec format** — a plain immutable value describing series, axes,
labels, a chart kind — would let scripts emit "here is a plot of X" in a
shape any host's renderer can consume. The host injects a
`Plotter` capability that takes the spec; a host without a renderer (or a
script run in CI) simply receives a no-op or a serializer capability that
writes the spec to disk for later viewing. Decide whether to align with an
existing portable format (Vega-Lite, Plotly JSON) or define one. Strong
preference: pick an existing open format rather than invent one.`

## Value provenance — *not* observability

`TODO(open): the use-case research repeatedly asks "where did this value come
from?" — for audit trails in finance, error messages in config, "which mixin
won" in inheritance-style configs. This is *not* a stdlib observability
concern: it is a business-logic concern that user code expresses with refined
types, tagged unions, and explicit derivation steps. Cross-referenced here
because the observability surface is the natural place users first look for
it; flag elsewhere (probably in the use-case showcases for finance and
configuration) that the answer is *typed data*, not a runtime trace.`

## Why every pool and queue has a failure handler and metrics

Several recurring catalogue patterns
shape why Tel insists thread pools and queues come with mandatory failure
handlers and on-by-default metrics:

- **"Thread silently died."** A builder threw inside a worker, the worker
  pool kept reporting "healthy", nothing alerted. The Tel stdlib makes the
  failure handler part of pool construction — there is no zero-argument
  constructor that omits it (see
  [`12-concurrency-utilities.md`](12-concurrency-utilities.md)).
- **"Exception logged but no alert."** An export threw; the exception was
  written to a log file nobody read. The lesson the catalogue draws is
  that log lines aren't an alerting mechanism — counters and structured
  events are. Tel's metrics surface (counters, histograms, timers above)
  exists so the alerting *and* the human-readable log can come from the
  same call without one being "the real story" and the other being
  cosmetic.
- **"Background task crashed, leaving zero in the grid."** A pricing
  thread died; downstream code couldn't tell stale 0.0 from live 0.0. The
  *staleness* maxim in
  [`../02-philosophy/02-maxims.md`](../02-philosophy/02-maxims.md)
  ("stale data is worse than no data — silence is a signal too") is why
  observability includes liveness signals, not just event counts.
- **"15-minute startup, quadratic schema registration."** Cross-system
  coordination ran in O(N²) without anyone noticing because the
  observability of *startup* time was poor. Boot-time spans
  (the nested logging story above) make this visible by default.
- **"No metrics on the queue that was the actual problem."** Two services
  shared a process; the OOM came from one that had no queue metrics. The
  queues-have-metrics-by-default rule (see
  [`../17-standard-library/04-core-collections.md`](../17-standard-library/04-core-collections.md))
  is a direct response.
- **"Counters that allocate a new metric per name."** A metric created on
  every call leaked memory until the writing buffer exceeded `2^31` and
  threw an `IllegalArgumentException` misdiagnosed as OOM. The Tel
  metric API binds a counter to a *named* call site once; per-label
  combinations are enumerated from the (enum-typed) labels, not
  invented from arbitrary strings.

## Editor support

Observability code is heavy on calls and easy to drown the happy path
in. The editor integration story (see
[`../18-tooling/09-editor-integration.md`](../18-tooling/09-editor-integration.md))
goes hand-in-hand with this chapter:

- Log/metric/trace calls can be visually folded or de-emphasised.
- A folded log can be expanded to show an *example* rendered message
  built from the template.
- A metric's labels can be autocompleted from the declared enum types.

## See also

- [Prelude](03-prelude.md) — `todo`, `must`, lazy values
- [Strings and Text](06-strings-and-text.md) — lazy string building
- [Concurrency Utilities](12-concurrency-utilities.md) — channels and the
  task tree that backs nested traces
- [Editor Integration](../18-tooling/09-editor-integration.md)
- [Antifeatures](../02-philosophy/04-antifeatures.md) — why everything
  goes through capabilities
