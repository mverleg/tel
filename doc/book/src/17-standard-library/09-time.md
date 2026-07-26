# Time

<!-- TODO: review -->

## What

`std` describes time types — instants, durations, dates — and the `Clock`
capability that produces "now". As with all I/O, **there is no ambient
clock**. A script that needs the current time receives a `Clock` from the
host; one that was not given a `Clock` cannot read wall-clock time.

## Why: time is a capability

Making time an injected capability rather than an ambient global buys two
things ([`../02-philosophy/01-priorities.md`](../02-philosophy/01-priorities.md)):

- **Reproducible runs.** A test, or a deterministic re-run, hands the script
  a fake clock and gets identical behaviour every time.
- **One script, many hosts.** A host with no real-time clock simply grants a
  different `Clock`, and the same source runs.

## How it looks

```tel
# The host injects `clock`; the script reads time only through it.
fn age_days(order: Order, clock: Clock) -> Int64 {
    clock.now().days_since(order.placed_at)
}
```

## Controllable clocks

Because the clock is a value, a host (or a test harness) can supply one whose
time it controls — advancing it manually rather than waiting for real
seconds. The payoff: a simulated clock can be *advanced
to the next event's arrival time*, firing any pending timeouts instantly
instead of sleeping. A controllable clock should therefore also drive
**timeouts** — a timeout is measured against the injected `Clock`, not against
an ambient timer — so the whole timing behaviour of a script is reproducible.

`TODO(open): should time be modelled as an explicit
*effect* (a `Time` effect) rather than just an injected value? Tasks measuring
timeouts would then declare the effect. Decide once the effects/capabilities
design firms up; for now treat the `Clock` as an ordinary injected capability
value.`

## Date and duration types

`std` provides several distinct time-shaped types so the programmer says
exactly which one they mean. They are all ordinary immutable values; only
*reading the current time* needs a capability — arithmetic on durations
and dates does not.

- **`Instant`** — a point on the wall-clock timeline, monotonic with
  respect to UTC. Suitable for "when did this happen", logs, scheduling
  by absolute time. **APIs that hand back a timestamp return an `Instant`
  by default, never a bare `Int64`** — a raw epoch number invites
  unit-confusion (seconds vs millis vs nanos) and lets a timestamp be added
  to an unrelated integer. The raw number is still reachable for the cases
  that need it (serialisation, FFI, bucketing): an `Instant` exposes its
  epoch value explicitly, so the conversion is a deliberate call rather than
  the default shape.
- **`MonotonicInstant`** — a point on a clock that never jumps backwards,
  suitable for measuring elapsed time without being fooled by NTP
  corrections or daylight-savings transitions. Read from a separate
  `MonotonicClock` capability; subtracting two yields a `Duration`.
- **`Duration`** — a length of time, with arithmetic.
- **`Date`**, **`Time`**, **`DateTime[TZ]`** — calendar values for human
  reasoning, parameterised by a timezone where ambiguity matters. The
  *timezone database* itself is host-supplied (it changes more often than
  the language can).
- **`TimeOfDay`** — wall time without a date, for things like "every day
  at 09:00".

The two clocks (`Clock` for wall, `MonotonicClock` for elapsed) are
deliberately separate capabilities so a host can grant one without the
other — a deterministic simulation environment may want monotonic only,
or wall only. `TODO(open): exact type names; whether `Instant` defaults
to wall or monotonic; UTC handling for `DateTime`.`

## Duration helpers

`std` offers a small set of conveniences around durations:

- **`time_since(t)` / `time_until(t)`** — `Duration` between *now* and a
  given `Instant`, with the right sign. Equivalent to `clock.now() - t`,
  but the named form reads better at call sites.
- **Multi-format parsing** — accept several formats (ISO 8601, RFC 2822,
  a permissive "natural" form) without silently misinterpreting one as
  another. The library exposes *named format groups*, not a single
  permissive parser, so a caller commits to which set of formats they
  accept.
- **Months between dates** — non-trivial because months are not a fixed
  length; the library exposes `months_between(a, b)` with a documented
  rounding policy.
- **Days since epoch** — primitive for serialisation and bucketing.

`TODO(open): parsing date/time is the classic source of subtle bugs (the
input cites the "falsehoods programmers believe about dates" lists). The
exact API needs care — pick a small, opinionated set of named formats and
refuse to autodetect.`

## Bugs the time-types story prevents

The catalogue records a thick band of
time-related production bugs. A few that drive the design:

- **"Unit tests failed at 9:00 in Amsterdam on January 9."** Tests used
  *today + 2 months* in Chicago; the hour didn't exist due to DST. Fix in
  Tel: a `Clock` capability the test controls — the test runs in a fixed
  timezone with a fixed instant, not in whichever timezone the developer
  ran the test in.
- **"Builds suddenly failed because a test used the current time as an
  underlying expiry."** A real-clock dependency in a test passed for years
  until a rolled date pushed an option past expiry. Same fix: the test
  injects a `TestClock`.
- **"Calculation got more and more wrong, because it relied on the current
  date."** Same root cause; same fix: the calculation takes the date it
  needs, the test pins it.
- **"Date jumps every day at fixed times because forwards roll."** A
  feature thought to be inactive was producing values that disappeared at
  forward-roll boundaries. The boundary itself isn't a bug — but the
  injected clock and the
  [observability story](14-observability-and-logging.md)
  make "this fired because a roll boundary passed" something a test can
  surface deterministically.
- **"Time-to-expiry cutoff at startup, GUI started hours earlier than the
  fitter."** A startup-time cutoff baked the launch hour into the
  filter's behaviour. With an injected `Clock`, the cutoff is computed
  against the clock the rest of the system shares, not against the
  process start time.
- **"Long-running report stopped rendering past a certain point each
  day."** A per-expiry report was filtering at a late stage; the expiry
  rolled mid-run; the report appeared empty rather than reporting the
  roll. An injected `Clock`, plus the observability story, makes "data
  filtered out because of a clock-driven boundary" visible rather than
  invisible.
- **"Kafka delay of 8 seconds shifted persisted-vs-live ordering."** Live
  data and persisted data diverged because consumer-receive time and
  publish time differ. Tel's distinction between `Instant` (wall time)
  and `MonotonicInstant` (elapsed), and the separation between *the
  capability that gives me now* and *the timestamp on a message*,
  is what makes this debuggable.
- **"Pillar dates rounded back and forth, picking up holidays
  inconsistently."** A round-trip conversion (settlement → expiry →
  settlement) didn't agree with itself because intermediate conversions
  applied holiday rules differently. The general rule the catalogue
  draws: prefer to keep a date in its native representation and pass it
  through, rather than convert back and forth. Refined types
  (`SettlementDate` vs `ExpiryDate`) make accidental round-trips a
  compile-time error.

## See also

- [I/O and Filesystem](08-io-and-filesystem.md)
- [Concurrency Utilities](12-concurrency-utilities.md) — timeouts on tasks
- [Scheduling and Timed Operations](17-scheduling-and-timed-ops.md)
- [Internationalisation and Formatting](16-internationalisation.md) —
  locale-aware date rendering
