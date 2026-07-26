# Scheduling and Timed Operations

<!-- TODO: review -->

## What

`std` ships a small family of *time-aware* control-flow helpers — the
"do this in a moment", "try again with backoff", "no more than once
every 100 ms", "compute these in dependency order" patterns — built on
top of the injected `Clock` (see [`09-time.md`](09-time.md)) and the
task model (see [`12-concurrency-utilities.md`](12-concurrency-utilities.md)).

The chapter pulls together several related concerns:
scheduled execution, retries with backoff, debounce and
throttle, and a *computation dependency graph* — all driven by the
same `Clock` and worker-pool capabilities, all reproducible because
time is injected.

## Why: timing patterns repeat across every script

These are recurring needs:

- **Retry an operation N times** with fixed or exponential dropoff.
- **Compute at most once every 100 ms** — without dropping the last
  request.
- **Collect events for a window**, keeping only the first or last, or
  all.
- **A computation dependency graph** — a DAG of work items with
  declared dependencies, run in topological order, optionally on a
  task pool.

Every one of them can be hand-rolled in five lines. Every one of them
is also where five-line hand-rolled versions accumulate subtle bugs:
wrong rounding, ignored cancellation, dropping the last event,
spinning in the failure case. `std` makes the obvious form correct.

Because each of these depends on the injected `Clock`, a test can fast-
forward through hours of behaviour in milliseconds by advancing the
controllable clock manually.

## Scheduling

### Scheduled execution

A scheduled job runs a task at a future instant or on a repeating
cadence:

```tel
# Sketch — `sched` is a host-granted Scheduler capability.
sched.at(deadline = clock.now() + Duration.minutes(5), || cleanup())
sched.every(period = Duration.seconds(30), || heartbeat())
```

Every scheduled job is **cancellable** (`handle.cancel()` removes it
before it runs) and **observable** (the same metrics surface as a
worker pool — see [`14-observability-and-logging.md`](14-observability-and-logging.md)).
A job that *blocks* should be marked as such so the scheduler does not
starve the CPU pool; the open question on that marker lives in
[`12-concurrency-utilities.md`](12-concurrency-utilities.md).

### Cron expressions

The library exposes a *readable cron* sublanguage rather than the
classic five-field crontab string. This is a candidate
DSL showcase:

```tel
let job = Cron.every().weekday().at(Time.of(9, 30))
let nightly = Cron.daily().at(Time.of(2, 0)).in_timezone(TZ.utc())
sched.cron(job, || run_report())
```

The DSL produces a `Cron` value (an immutable description); the
scheduler interprets it. A misspelled cron expression is a *compile*
error, not a runtime "next fire is in the year 2099" bug.

`TODO(open): the readable-cron DSL is a candidate use-case showcase in
[`../19-use-cases/`](../19-use-cases/); decide whether the surface
lives in `std` or in a separate sub-crate. Lean: parser and types in
`std`, the syntax sugar in the language.`

## Retries and backoff

The retry adapter wraps any fallible operation and tries again on
failure, with a chosen backoff strategy and an upper limit:

```tel
let policy = Retry(
    max_attempts = 5,
    backoff      = Backoff.exponential(initial = Duration.millis(100),
                                       factor  = 2.0,
                                       jitter  = 0.2),
    retry_on     = |err| err is NetError.Transient,
)

let body = policy.run(clock, || http.get(url)) ?
```

Properties the library commits to:

- **Backoff strategies are first-class** — `fixed`, `linear`,
  `exponential`, with optional *jitter* to avoid thundering herds.
- **Retry predicates are explicit** — a script states *which* errors
  to retry, never a blanket "retry anything". Retrying on a 4xx is
  almost always a bug.
- **Deadlines** — a `Retry` may also accept an absolute deadline; if
  the deadline passes mid-retry, the policy returns the most recent
  error rather than starting another attempt.
- **Reproducibility** — backoff times are measured against the
  injected `Clock`; a test with a controllable clock walks through
  the retry schedule deterministically.

`TODO(open): the relationship between `Retry` and the worker pool's
"on_task_error" hook — both deal with failure, but at different
layers. Document so users don't reach for both.`

## Debounce, throttle, rate limit

The "compute at most once every 100 ms" pattern has several distinct
variants, each useful in different cases. The library names them
distinctly:

- **`throttle(period)`** — at most once per period; trailing edge by
  default (so the final event in a burst is not dropped). Edge mode
  (`leading`, `trailing`, `both`) is an explicit argument.
- **`debounce(period)`** — wait until `period` of quiet, then fire
  once. Used for "fire when typing stops".
- **`rate_limit(n, per)`** — at most `n` events per `period`; excess
  events are *queued* by default (configurable: `drop`, `fail`, or
  `queue`).
- **`sample(period)`** — fire whatever the *current* value is once
  per period; intermediate values are dropped.

Each takes a `Clock` and (where applicable) a queue from
[`04-core-collections.md`](04-core-collections.md), so the
fullness-policy story is uniform across the library.

## Window aggregation

The companion to throttle / debounce is *window aggregation* — collect
events over a window, hand the batch to a callback:

- **`tumbling(period)`** — non-overlapping fixed windows.
- **`sliding(window, step)`** — overlapping windows.
- **`session(gap)`** — windows defined by gaps in the event stream.

Each is exposed as a stream adapter (see
[`05-iteration-and-streams.md`](05-iteration-and-streams.md)) so they
compose with the rest of the pipeline.

`TODO(open): the window-aggregation set overlaps with stream adapters
and with the throttle / debounce family. Decide one canonical naming
so the reader sees one consistent family.`

## Timeouts

A timeout wraps any awaitable; if it does not resolve within the
duration, the wrapper fails (and signals the underlying task to cancel,
if it can):

```tel
let body = timeout(Duration.seconds(2), || http.get(url)) ?
```

The timeout is measured against the injected `Clock`. The library also
exposes a `with_deadline(t)` variant taking an absolute instant
instead of a relative duration — useful in retry loops where the *aggregate*
deadline is what matters.

## Computation dependency graph

A *computation graph* — a DAG of operations with declared dependencies, run in
topological order on a task pool, with optional result caching and a
visualisation hook — is **deferred**. The surface is substantial and sits at the
library/workflow-engine line, so a small script never pays for it and a crate
can ship it before `std` blesses one. The sketch and properties live in
[Deferred Features → Computation dependency graph](../20-appendix/06-deferred-features.md#computation-dependency-graph-stdlib).

## Cache and persistence

`std` exposes a small *cache* interface (set, get with default,
invalidate, scan by prefix) backed by host-granted storage:

- **In-memory** — default, no capability needed.
- **Disk-backed** — needs a filesystem capability.
- **Host-managed** (browser local storage, remote KV store) — needs
  a host-granted cache capability.

The retry, debounce, and computation-graph utilities all *may* use the
cache; none of them require it.

`TODO(open): how the cache interacts with the persistent storage in
[`13-data-formats.md`](13-data-formats.md) — the two are very close;
decide whether they are one feature or two.`

## Bugs these utilities prevent

A few concrete catalogue cases:

- **"GUI overloaded by an event that fires every run."** A subscriber to a
  GUI update channel was extended to fire on every run instead of on
  specific events. The fix was throttling per event type — exactly the
  per-stream `throttle` / `debounce` family above.
- **"Updates didn't fire for half an hour, GUI got stale."** A fitter
  reported only on significant updates; a stuck pipeline produced none.
  Tel's answer is a *liveness* signal alongside the value stream — a
  scheduled heartbeat tied to the same `Clock` the producer reads, so a
  consumer can distinguish "no news because nothing changed" from "no
  news because the producer is stuck".
- **"Retry policy retried on a 4xx response."** A blanket retry retried
  errors that were not transient, amplifying the load. The explicit
  retry predicate (`retry_on = |err| err is NetError.Transient`) is the
  structural fix.
- **"Connection pool exhausted under async load."** More than `N` async
  callers were waiting to acquire one of `N` database connections; the
  rest deadlocked. Use a [bounded channel as the gate](../14-concurrency-and-parallelism/06-channels-and-message-passing.md#channels-as-resource-gates)
  — the wait point is visible and the bound enforced.
- **"Avro buffer underflow because messages were tiny and packed."**
  Stream-style parsers that expose mid-buffer state get out of sync.
  Use the *window-aggregation* adapters above (or proper streaming
  parsers in [`13-data-formats.md`](13-data-formats.md)) rather than
  rolling buffer arithmetic by hand.

## See also

- [Time](09-time.md) — `Clock`, `MonotonicClock`, `Duration`
- [Concurrency Utilities](12-concurrency-utilities.md) — worker pools,
  `select`, supervision
- [Iteration and Streams](05-iteration-and-streams.md) — window
  adapters
- [Observability and Logging](14-observability-and-logging.md) —
  scheduled-task metrics
- [Core Collections](04-core-collections.md) — queue fullness policies
