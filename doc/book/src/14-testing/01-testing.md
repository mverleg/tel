# Testing

<!-- TODO: review -->

## What

Testing is a **first-class language feature** in Tel, not a bolt-on tool —
which is why it lives here among the language chapters rather than in
[Tooling](../18-tooling/01-compiler.md). Tests are ordinary Tel functions
marked with a `test` declaration; `tel test` (a built-in subcommand of the
compiler) discovers them, sets up the host capabilities they ask for, runs
them in isolation, and reports results. Benchmarks are the close sibling and
share this chapter (see [Benchmarks and profiling](#benchmarks-and-profiling)).

This chapter covers discovery, execution, isolation, reporting, modes, and the
test-only language affordances. The value-level surface — assertion helpers,
property generators, fixture builders, deterministic capability stubs — lives in
[`../17-standard-library/19-testing-utilities.md`](../17-standard-library/19-testing-utilities.md).

## Why

Embedded scripts have an unusually high cost of "untested" — the host
discovers the bug, the user is the one running the script, and the
language designer never sees the failure. So testing is part of the
language toolchain:

- **One test command across every host.** A script embedded in three
  hosts must give the same answer to "do my tests pass." That requires
  the runner — not just the assertions — to behave the same everywhere.
- **Capabilities make tests cheap.** Because all I/O, time, and
  randomness are injected, swapping them for deterministic stubs is
  the default state, not an afterthought.
- **Compile-time and runtime are the same checking story.** Tel
  treats invariants and asserts as a spectrum (compile-checked when
  provable, runtime-checked otherwise — see
  [`../02-philosophy/03-features.md`](../02-philosophy/03-features.md)).
  Tests sit on the same spectrum: some fire at compile time ("every
  implementer of `Serializable` round-trips"), others at run time.

## What `tel test` does

`tel test` is a single subcommand of the compiler, no separate build
step. A typical invocation:

1. Compile the project, including test-only code.
2. Discover all test declarations (and the compile-time checks they
   enable).
3. Build an isolated context per test (clock, RNG, filesystem,
   capabilities — see [Isolation](#isolation)).
4. Run tests in randomised order, in parallel where the host permits.
5. Report a rollup, with per-test detail on failure.

There is no split `tel test-build` and `tel test-run` lifecycle — a
test artefact that runs zero or one times does not earn a distinct
phase. `TODO(open): a host with a slow AOT compile may want a split;
revisit if it comes up.`

Two modes worth calling out:

- **Watch mode.** Re-runs affected tests on file change, driven by the
  same symbol-level incremental compile the LSP uses.
- **Quick mode** (sbt-style `testQuick`). Runs only tests that failed
  last time, were never run, or whose transitive dependencies
  recompiled. No manual rules — the dependency graph drives it.

## Run scope, platforms, and tags

**Scope.** `tel test` with no argument runs every test declared in the
workspace. Given a crate or binary, it runs that target's tests *and*
the tests of its in-workspace dependencies — so testing a binary also
exercises the workspace libraries it relies on.

**Platforms.** A script that embeds in several hosts must answer "do my
tests pass" the same way on each, so the runner makes it easy to run the
whole suite across multiple [compile targets](../18-tooling/02-compile-targets.md) in a
single invocation. A crate may *optionally* declare platforms it
explicitly supports or excludes; any platform it does not mention is
inferred from the [capabilities](../02-philosophy/03-features.md) it uses —
a crate that needs no host-specific capability is portable by default.

`TODO(open): how platforms are named. A "platform" like the JVM may have
several independent implementations, and external backends can add their
own, so a flat name is not enough. Decide whether there is a notion of
*official* platforms versus a capability-plus-backend descriptor, and how a
crate spells "supported on the JVM, any implementation" versus "supported
on this one backend".`

**Tags.** Tests carry tags, and a tag can be enabled or disabled by
default; a run can flip them. Tags group slow, integration, or
platform-specific tests so the common `tel test` stays fast while the full
matrix is one flag away.

**Incremental runs fall out of capabilities.** Because a unit test takes no
I/O, time, or randomness capability by default (see [Isolation](#isolation)),
the compiler knows which tests a given code change can affect. A test that
passed last time and whose transitive inputs did not change is skipped — the
same dependency graph that drives quick mode.

## Test discovery

A test is a `test` declaration. It does **not** need the `fn` keyword,
and its name is a **string**, not an identifier — a test name is prose
to read in a report, not something any code calls:

```tel
test "gold tier discount is ten percent" {
    expect(discount(Tier.Gold) == Percent(10))
}
```

The `test` marker lifts a few language restrictions inside the block:
access to private symbols of the enclosing module, the ability to
override otherwise-final classes for mocking (see [Mocks](#mocks)), and
access to test-only globals.

Tests live next to the code they exercise; there is no naming
convention to memorise and no required `tests/` folder.

`TODO(open): the rest of this chapter still shows the older `test fn
name()` form in examples — sweep them to the string-named `test "…" { }`
form.`

### Natural-language expectations

A bare `expect(cond)` reports the *expression* on failure. To make a run
read as prose, an `expect` block may carry a description string and wrap
the asserting code:

```tel
test "users can be found by name" {
    expect "Henk is found by his name" {
        assert(find_user("Henk").is_some())
    }
}
```

The description is optional and test-only; the runner prints it verbatim,
so a green run reads as a list of satisfied expectations rather than a
count. This is the positive-phrased sibling of the existing
[`fails "…" { }`](#snapshot-golden-and-compile-fail-tests) block, where the
wrapped code is expected to be *rejected* instead of to hold.

### Test-only visibility

Tel also exposes a `test` visibility modifier on non-test
declarations — a helper, constant, or fixture that is only callable
from test code (and from other `test`-visible items). This keeps
fixtures co-located with production code without leaking them into the
production API:

```tel
test fn fixture_default_order() -> Order { ... }
```

`test`-visible items are not themselves tests; they are *callable from
tests* and inherit the same extra privileges (private access,
override-finals).

### Skipping

A test may decide at runtime that it cannot meaningfully execute —
required executable missing, capability not granted, precondition
absent. Tel supports this through an explicit *skip*:

```tel
test fn round_trips_through_external_daemon() {
    let daemon = require_exe("teldaemon") or return skip("daemon not on PATH")
    ...
}
```

`skip(reason)` is reported in the run summary distinctly from pass and
fail. `TODO(open): exact spelling. Candidates: a return type
`TestResult = (Result | Skip)`, a magic keyword, or a top-level `skip`
function. Lean: explicit return, so the type system sees it.`

### Structural / compile-time checks

A test does not have to *call* anything. Tests may express
project-wide rules ("every implementer of `T` in `api` is
`Serializable`", "function `f` compiles without warnings"). These read
like tests in the output but are evaluated at compile time. They
overlap with linter rules; see [Linter](../18-tooling/07-linter.md) for where the
boundary sits.

### Examples as tests

Doc comments may include example blocks that the runner picks up as
tests (Rust/Python doctests). An example that stops compiling or whose
stated result no longer holds fails the run, so documentation stays
accurate. `TODO(open): syntax for example blocks. Coordinate with
[Documentation Generator](../18-tooling/10-documentation-generator.md).`

## Isolation

The default isolation model is aggressive:

- **Per-test memory isolation.** Each test runs as if no other test
  had ever executed — no shared globals, no carried-over caches, no
  leaked tasks. Inspired by Rust's `nextest`.
- **Scoped filesystem.** A test that gets a filesystem capability
  sees an overlay rooted at a per-test scratch directory. Outside
  reads fail; writes are discarded on exit.
- **Fixed clock and seeded RNG.** Time and randomness are
  capabilities; the test context wires them to deterministic stubs.
  `now()` returns a fixed instant, the PRNG starts from a seed printed
  in the failure output so a failure is reproducible.
- **Shared-state jitter.** Real bugs only surface when state is
  shared (memory ordering, missing barriers). The runner can opt
  individual tests into a *jitter* mode that makes every non-atomic
  variable thread-local, or applies it to a random subset across runs
  to surface accidental sharing. `TODO(open): an estimated
  ~80% of catches are false positives. Keep this off by default;
  expose as opt-in stress mode.`

Two escape hatches keep aggressive isolation from being a straitjacket:

- An attribute (working name `@share_immutable`) that lets an
  immutable fixture be shared between tests. Immutability is the
  gate; sync-by-type is not enough. `TODO(open): naming; whether
  immutability is type-inferred or asserted.`
- Mutable state is **never** shared between tests — the runner
  refuses to wire a mutable fixture into more than one test at a time.

**No capabilities by default.** A test takes **no** capabilities unless
it asks — by default it cannot touch the clock, the filesystem, the
network, or the RNG. So the bulk of a project's tests (the *unit* tier)
are statically incapable of depending on the environment: they run in any
order, fully parallelise, and can be skipped by the incremental runner
when nothing they touch changed. The mechanism falls out of the
capability system — a test declared with no capabilities in its signature
*cannot* call any I/O-shaped operation. A test that needs a real
capability opts in explicitly, which makes the environment dependency
visible right at the test signature, and pushes it into the *integration*
tier.

`TODO(open): the exact opt-in spelling — whether the capability-taking
form is marked `integration test "…"`, simply named by the capabilities
in its signature, or distinguished some other way.`

### Bugs this isolation design prevents

The aggressive per-test isolation is a direct response to recurring
real-world bug patterns:

- **"Test that used current time started failing on a specific date."** A
  unit test built dates from "today" and tripped over a DST gap. With
  injected `Clock` capabilities and the runner pinning a deterministic
  default, this can't drift.
- **"Concurrent test passed locally, failed under load."** A
  `synchronized` block was lost in a merge; the failure mode was a load-
  dependent `ConcurrentModificationException`. The
  *shared-state jitter* option (off by default) is the response — when
  intentionally enabled, it surfaces accidental sharing that real load
  would eventually expose anyway.
- **"Sim test failed because the broken data was *in* the persisted
  scenario."** A simulation reads its scenario; the scenario's broken
  half is exactly what would have repro'd the bug, but the bug prevents
  the scenario from loading. The runner's *fixture builders* and
  *snapshot tests* let the broken inputs be checked into source as
  literal data, not lifted from a fragile re-import path.
- **"Embedded-mode dev didn't see the serialization error."** A service
  pair ran in embedded mode with no serialization in dev; production
  serialized and broke. The runner's standard environment forces
  serialization at the boundary even in single-process tests, so
  encode/decode bugs surface where they belong.
- **"Tests passed for years because of a stub that always returned
  empty."** A mock had a default fallback that quietly returned defaults;
  the test passed for the wrong reason. The default `Fail` policy for
  unmocked methods on a mock is the response — silently returning
  defaults is exactly the failure mode this is meant to prevent.

## Order and dependencies

Tests run in **randomised order**, including sub-iterations within
parameterised tests, so an accidental order-dependency surfaces
immediately. Tests with legitimate dependencies declare them:

```tel
test fn save_then_load_returns_same_payload()
    depends_on serialiser_round_trip { ... }
```

The dependency is informational: the runner picks an order that
respects it for display, and when both fail it flags the dependency
root first so the investigator starts at the cause.

`TODO(open): automatic root-cause hinting — "if
test B's unit uses test A's unit, investigate A first." Lean: keep
`depends_on` as the declared form, add automatic hinting as a runner
heuristic.`

## Failure model

A test fails when an assertion fails, when a *loud abort* (the
`must` / `todo` / `unimplemented` family from
[`../17-standard-library/03-prelude.md`](../17-standard-library/03-prelude.md))
fires anywhere inside it, or when a propagated `Result::Err` reaches
the test boundary unhandled.

A test does **not** need to return an `Ok` value: using
`fancy-exit-?` in a test should fail the test, but not require
an okay value. A test is implicitly `Result[(), TestFailure]`, so
an early `?` propagation aborts the test with that error as the
failure.

### Soft assertions and multiple failures

A `soft { ... }` block lets multiple assertions all fail before the
test stops — useful when reporting *everything* wrong is more helpful
than stopping at the first problem. The shape and the dependent-check
pitfall are described in
[`../17-standard-library/19-testing-utilities.md`](../17-standard-library/19-testing-utilities.md).
Combined with parameterised tests, the runner produces a per-row
failure table.

## Parameterised tests

A test can declare a parameter set; the runner runs the body once per
row with reporting that separates them. The same machinery supports:

- **Data-driven tests.** A literal table of inputs and expected
  outputs — see [Case tables](#case-tables).
- **Shared setup across tests.** A group of related tests share a
  fixture (a parsed AST, a populated database capability). JUnit-5
  nested classes and Rust modules are the reference points; Tel uses
  **test groups** (working name).

```tel
test group orders {
    fixture let sample_orders = parse_fixture("orders.json")

    test fn pricing_runs_on_every_order(order in sample_orders) {
        expect(price(order).is_ok())
    }
}
```

`TODO(open): nested groups vs flat tests with shared setup. Lean:
groups — they give a place for fixture lifetime and shared parameter
sets.`

### Case tables

The simplest parameterised form is a list of literal rows. A test
declares typed parameters and precedes its body with one `case` line
per row; the runner parses each line into the parameters and runs the
body once per row, reporting rows separately.

The twist is *how a row's text becomes typed data*. Most of the work
in a data-driven test is exactly that — turning a string into a value
of the right type — and Tel already has a facility that does this: the
declarative [CLI argument parser](../17-standard-library/10-os-and-process.md#cli-argument-parsing)
that host entry points use. A `case` line reuses it, so a row reads
like a command line and the parameters are filled positionally or by
name:

```tel
case --today 2026-05-29 --born 2026-05-28 --age 0
case --today 2026-05-29 --born 2025-05-28 --age 1
case --today 2026-05-29 --born 2024-05-29 --age 2
test fn calculate_age(today: Date, born: Date, age: UInt32) {
    expect(age_of(born, today) == age)
}
```

Each `case` line is parsed against the test's parameter record with
the same machinery as `main`'s `argv`: `Date` parses from its literal,
`UInt32` from digits, an `Option[T]` may be omitted, a flag is bare. The
benefits are that the conversion is *typed and validated up front* (a
malformed row is a parse error against the test signature, not a
runtime surprise mid-body) and that the surface is one the author
already knows from writing entry points — `familiarity over a novel
surface` (see [philosophy](../02-philosophy/01-priorities.md)).

`TODO(open): does reusing CLI-arg syntax for case rows pay off, or is
it too clever? It is `writability`-leaning and a row mixes inputs and
expected outputs on one line with no visual divider. Alternatives: a
literal tuple/record table, or a `|`-delimited columnar table with a
header row. Lean: keep the CLI form for the value (string→typed reuse)
but revisit the inputs-vs-expected split — possibly a marker such as
`case … => age 2`, or a convention that trailing parameters are the
expectation.`

`TODO(open): positional vs named in `case` rows. One option is to
just use positional, since it's mostly about turning strings into data.
Named is self-documenting per row but verbose; positional is terse but
order-coupled. Lean: allow both, exactly as the CLI parser does.`

### Function cases (shorthand)

When a row only needs to *call one function and compare its return*,
writing a full `test fn` with a body is ceremony. Tel offers a
shorthand: attach `case` rows directly to the function under test,
with no separate test body. The runner synthesises the call-and-compare:

```tel
# Each row: arguments, then the expected return.
case 2026-05-28 2026-05-29 => 0
case 2025-05-28 2026-05-29 => 1
fn age_of(born: Date, today: Date) -> UInt32 { ... }
```

This is sugar over a parameterised test whose body is
`expect(f(args) == expected)`. It is intentionally limited to the
single-call/equality shape; anything richer (multiple assertions,
setup, soft blocks) is written as a normal `test fn` with a case table.

`TODO(open): several calls to resolve. (1) Where do the rows live —
adjacent to the function as sketched, in an attribute, or in a doc
comment that doubles as an [example test](#examples-as-tests)?
(2) The inputs-vs-expected divider (`=>` shown here) shares the open
question above. (3) Equality is the only predicate; decide whether to
allow a comparator or a small predicate, or to keep it strictly `==`
and push anything else to a full test. (4) Confirm that letting a
plain `fn` carry test rows does not blur the production/test boundary —
it should compile away entirely outside `tel test`, like any other
test declaration.`

### Property tests

Property-based testing is parameterised testing where the rows are
*generated* by a strategy and shrunk on failure. The runner uses
generators from
[`../17-standard-library/19-testing-utilities.md`](../17-standard-library/19-testing-utilities.md):

```tel
test fn parse_then_render_is_identity(s in any[ValidJson]()) {
    expect(render(parse(s)) == s)
}
```

The seed is captured in the failure output so a shrunk counter-example
is reproducible.

### Trait tests

A trait can declare "if you want to be a `Cache`, you must satisfy
these tests"; the runner runs them against every concrete implementer
in the project. Implementers supply example instances; an implementer
that does not is a compile-time error.

```tel
trait Cache[K, V] { fn get(k: K) -> Option[V]; fn put(k: K, v: V) }

test trait Cache {
    test fn get_after_put_returns_value(c: impl Cache[Int64, Str], k: Int64, v: Str) {
        c.put(k, v)
        expect(c.get(k) == Some(v))
    }
}
```

The example-instance side lives in
[`../17-standard-library/19-testing-utilities.md`](../17-standard-library/19-testing-utilities.md).

## Mocks

> **Status: the mock story is open.** The affordances below are a sketch, not a
> settled design — and the runtime-replacement parts (overriding finals,
> swapping a global function) lean on the same hot-swap machinery that pushed
> the [REPL](../20-appendix/06-deferred-features.md#repl) into the deferred pile.
> **Until the mock story settles, prefer dependency injection**: pass a
> collaborator in as an argument (or a [capability](../02-philosophy/03-features.md))
> and substitute a test double at the call site. DI needs no special language
> support, works today, and keeps the seam visible.

Whatever lands, mocks will be deliberately not magic — frameworks like mockito
are too magical, and classes should not become virtual just for mocking. The
sketched answer is a small set of **test-only** affordances:

- Test code can **override otherwise-final methods**. Inside `test`
  scope, the class hierarchy is treated as if non-final; production
  never sees this.
- Test code can **replace a global constant or function** for the
  duration of a test, restored on exit.
- Test code can **construct a class with selected methods overridden
  inline**, without a separate mock subclass:

  ```tel
  let claim = Claim::new() with {
      fn retrieve() = Err(NetworkError)
  }
  ```

For unmocked methods, the test picks a policy: **default** (return the
type's default), **real** (call the original), or **fail** (abort).
The default policy is `Fail`, because silent defaults are how mock
tests pass while testing the wrong thing.

`TODO(open): syntax bikeshed (`with { fn ... = ... }`, `override`,
`replace`). Overriding-only-in-tests assumes a notion of overridability
that is *off* in production — confirm this fits the no-inheritance
stance in
[`../02-philosophy/04-antifeatures.md`](../02-philosophy/04-antifeatures.md).
The antifeature was decided on production semantics; the test-only
relaxation is consistent with treating tests as a special scope.`

## Snapshot, golden, and compile-fail tests

- **Snapshots.** A snapshot test records the serialised form of a
  value and compares future runs against it. Intentional changes are
  promoted with a `--bless` flag.
- **Artefacts.** Integration tests can produce screenshots, exported
  files, derived JSON for a human to glance at even when the test
  passes. The runner collects them as build outputs. `TODO(open):
  which artefact types are first-class. Lean: any serialisable value
  or any byte stream from an injected capability.`
- **Compile-fail tests.** A test may assert that a given snippet
  *fails to compile*, optionally with a specific diagnostic. Reference:
  Rust's `trybuild`. The runner uses the same compiler process — no
  separate invocation. Working name: **`fails_to_compile`** (it reads as
  a sentence: *this snippet fails to compile*).

  ```tel
  fails_to_compile("EuroAmt is not numerically combinable with UsdAmt") {
      fn bad() -> EuroAmt { EuroAmt(5) + UsdAmt(3) }
  }
  ```

  **The snippet must parse; only a *later* phase may reject it.** A
  `fails_to_compile` block passes when the code is rejected by any
  compile phase *after parsing* — name resolution, type checking, a
  [refined-type](../05-types/12-refined-types.md) or
  [contract](../02-philosophy/03-features.md) violation, an effect-bound
  mismatch, a visibility error. A **parse error fails the test itself**;
  it does not count as a pass. The reasoning: a syntax error in the
  snippet is almost always a typo in the *test*, not the rejection the
  author meant to pin down. Letting a malformed snippet "pass" would turn
  the block into a test that passes for the wrong reason — exactly the
  failure mode the runner works to prevent elsewhere (cf. the mock
  default-`Fail` policy). So the block has two distinct failure modes:
  *did not parse* (broken test) and *parsed but compiled clean* (the
  asserted error did not fire) — both reported as test failures, with
  different messages.

  The optional string argument is matched against the emitted
  diagnostic (a substring, or a pattern — `TODO(open)`), so a snippet
  that fails for an *unrelated* reason does not falsely pass.

  **Strict mode: a warning counts as a failure too.** By default the
  block passes only on a hard error; a snippet that merely produces a
  *warning* (and otherwise compiles) does not satisfy it. A **strict
  variant treats warnings as failures**, so a warning satisfies the
  block — this is what makes it the natural place to assert that a
  [provably-violated contract or a guaranteed panic is caught](../13-error-handling/04-panics-and-aborts.md#provable-panics-warn-do-not-reject),
  since those findings are *warnings* by default (the stable-surface rule
  keeps them from being hard errors). It mirrors a `-Werror` build:

  ```tel
  # Default: passes only on a hard error.
  fails_to_compile("EuroAmt is not numerically combinable with UsdAmt") {
      fn bad() -> EuroAmt { EuroAmt(5) + UsdAmt(3) }
  }

  # Strict: a warning is enough — here, a provably-guaranteed panic.
  fails_to_compile(strict, "always aborts: todo() reached") {
      fn never() -> Int64 { todo() }
  }
  ```

  `TODO(open): naming bikeshed — `fails_to_compile`, `does_not_compile`,
  `rejects`, `compile_error`. Lean: `fails_to_compile`. Also decide the
  spelling of strict mode (a `strict` marker argument as sketched, a
  `fails_to_compile_strict` sibling, or inheriting the project's
  warnings-as-errors setting), and whether the diagnostic argument is a
  substring match, a glob, or a structured error-code.`

- **A unified `fails` block.** One form that passes if the snippet is
  rejected *either* way — a compile error (after parsing, as above)
  *or*, if it compiles, a runtime abort when run. It is the union of
  `fails_to_compile` and [`expect_aborts`](../17-standard-library/19-testing-utilities.md#assertion-helpers):
  "this must not succeed, by whatever route." An optional message
  argument matches against whichever diagnostic or panic message fired.

  ```tel
  fails("must be positive") {
      Ratio(-1)        # rejected at compile time if provable, else aborts at run time
  }
  ```

  This is genuinely useful for invariants that sit on the
  compile/runtime spectrum: a [refined type](../05-types/12-refined-types.md)
  or [contract](../02-philosophy/03-features.md) is checked at compile
  time *when the compiler can prove the violation* and at run time
  otherwise (see
  [features](../02-philosophy/03-features.md)). A test that just wants to
  say "this value is invalid" should not have to know *which* phase
  catches it — and should keep passing if a smarter compiler later moves
  the catch from run time to compile time.

  `fails` carries the **same strict mode** as `fails_to_compile`: by
  default a compile *warning* alone does not satisfy it (only a hard
  error or a runtime abort does), but `fails(strict) { … }` accepts a
  warning as well. This matters precisely for the spectrum case above —
  a provable-violation finding is a *warning* at compile time, so strict
  mode is what lets `fails` catch it whether the compiler proved it or
  the value aborted at run time.

  The trade-off, and why `fails` does **not** replace the two specific
  forms: it is *weaker*. A snippet you meant to test as a runtime panic
  could start failing to *compile* after an unrelated refactor (a new
  type error), and `fails` would still pass — masking the change.
  `fails` asserts "rejected somehow"; `fails_to_compile` / `expect_aborts`
  assert "rejected *this way*." Prefer the specific form when you know
  the phase; reach for `fails` only when the phase is genuinely an
  implementation detail (the refined-type/contract case above). The
  parse-must-succeed rule still holds: a syntax error is a broken test,
  not a pass.

  `TODO(open): `fails` is a runner/tooling construct, not a pure-stdlib
  helper, because the compile-time half needs the compiler to attempt
  compilation and catch a compile error as data rather than failing the
  whole test build — so it lives here alongside `fails_to_compile`, with
  the runtime half delegating to `expect_aborts`. Decide the name
  (`fails`, `rejected`, `invalid`) and confirm the union semantics are
  worth a third form rather than documentation pointing at the two
  specific ones.`

## Benchmarks and profiling

Benchmarks are a sibling of tests — close enough to be the same first-class
concern — run by the same tool (`tel test --bench`) and discovered the same
way. They live here, with tests, rather than in the
[observability](../17-standard-library/14-observability-and-logging.md) chapter:
benchmarking is a performance-*testing* concern, not a logging one.

### Execution surface

`std` exposes the measurement API:

- **`bench.run(name, || ...)`** — measure a block, returning a distribution
  (mean, p50, p95, p99, max).
- **Stopping criteria** — total time, number of iterations, or until variance
  falls below a threshold; the benchmark picks based on what the caller asks.
- **Warmup and teardown** — explicit phases so JIT / cache effects do not
  pollute the timing.
- **Optional `nice` mode** — a "grab extra resources" hint; whether the host
  honours it depends on the capability granted. `TODO(open): pre-pivot —
  re-justify against embedding. The host owns scheduling priority; the
  benchmark capability can request a hint, no more.`
- **Timing primitives that don't affect the measurement.** `bench.now()` is a
  monotonic, low-overhead timer separate from the wall-clock `Clock`, suited
  for hot loops; the default benchmark loop uses it.

```tel
let result = bench.run("parse", config = Bench(min_time = Duration.seconds(2))) {
    || parse(payload)
}
log.info("parse benchmark", result)
```

`TODO(open): whether `bench` is its own capability or piggybacks on
`MonotonicClock` from [`../17-standard-library/09-time.md`](../17-standard-library/09-time.md).
Lean: its own capability so a host can deny benchmarks in production.`

### What the runner contributes

- **A historical record.** Each run is written to a build artefact so
  a separate tool can plot results and surface regressions.
- **Code-change awareness.** The runner records a (best-effort) hash
  of the code under test so a result can be tied to the source. Dynamic
  dispatch makes the hash approximate. `TODO(open): mark the hash
  imprecise where dispatch is dynamic.`
- **Hardware normalisation.** A coarse hardware-performance number can
  be folded into reports for cross-machine comparison. It is a hint,
  not a calibrated score.

Two patterns matter: comparing two implementations
of the same interface under concurrent load (much easier to set up
than JMH); explicit warmup phases for CPU caches and any host-VM
warmup (e.g. Wasm).

`TODO(open): should every benchmark ship with a
"good-vs-bad" judgement? Probably not — benchmarks usually compare
alternatives. Provide both: assert-against-baseline and
comparison-of-alternatives.`

### Flame graphs and heap dumps

**Production-grade flame graphs** and **heap dumps** sit alongside benchmarks
as the deeper performance tools. Both are host-supplied: `std` exposes a thin
capability surface (`profiler.flame(duration)`, `profiler.heap_dump()`), and
the host's runtime decides whether the operation is supported, how much it
costs, and where the output goes. A host that does not support profiling simply
grants no profiler capability; the script must cope.

`TODO(open): a longer wishlist — allocation history, async-suspension trace,
lock-and-barrier traces, thread context switches. Several of these only make
sense in hosts that have threads, locks, or async, so they cannot be guaranteed
in `std`. Treat them as *hints* in the capability type, supported where the
host can deliver. Re-justify each against embedding before promising it.`

## Auxiliary capabilities

- **`require_exe(name)`** — asserts an executable is on the host's
  PATH and returns its path, otherwise the test skips. Same shape for
  required libraries.
- **Auto-breakpoint on panic** when a debugger is attached, so a
  failing test stops at the abort site instead of unwinding all the
  way out. See [Debugger](../18-tooling/08-debugger.md).
- **Test-scoped logging verbosity.** A test can raise its logger to
  `debug` for the duration without touching global state, because
  logging is a capability — see `test::capture_logs` in
  [`../17-standard-library/19-testing-utilities.md`](../17-standard-library/19-testing-utilities.md).

## Gherkin / BDD?

Should the runner accept a Gherkin (Given /
When / Then) syntax for tests? The lean: **no, not as a first-class
language form.** It is a fine way to organise prose inside a test
(comments, labelled sections inside `soft { ... }`), but baking a DSL
into the runner conflicts with *one good way over many clever ones*
and *familiarity over a "better" but novel surface*. A crate may
offer Gherkin bindings that compile to ordinary `test fn`
declarations. `TODO(open): confirm rejection.`

## See also

- [Compiler](../18-tooling/01-compiler.md) — `tel test` shares the compile pipeline
- [Linter](../18-tooling/07-linter.md) — project-wide structural rules
- [Debugger](../18-tooling/08-debugger.md) — auto-break on assertion / panic
- [Editor Integration](../18-tooling/09-editor-integration.md) — test results in
  the IDE, run-test-at-cursor, soft-failure highlighting
- [Documentation Generator](../18-tooling/10-documentation-generator.md) —
  examples-as-tests
- [Standard Library: Testing Utilities](../17-standard-library/19-testing-utilities.md) —
  assertions, generators, fixtures, mock and capability helpers
- [Standard Library: Observability](../17-standard-library/14-observability-and-logging.md) —
  logging, metrics, and traces (benchmarks live in this chapter, with tests)
- [Antifeatures](../02-philosophy/04-antifeatures.md) — why
  "test-only" relaxations stay narrow
