# Testing Utilities

<!-- TODO: review -->

## What

`std` ships the *value-level* side of the testing story: assertion
helpers, property generators, fixture builders, and the deterministic
stubs for the I/O, clock, and randomness capabilities that tests
inject. The *tooling-level* side — how tests are discovered, isolated,
parallelised, and reported — lives in
[`../14-testing/01-testing.md`](../14-testing/01-testing.md). This
topic covers what a test *writes*, not what the test runner *does*.

The split matters because the same assertion helpers are useful
outside the test runner too — `expect` shaped checks make good
contracts during development, and the deterministic clock makes
deterministic replays possible in any script.

## Why

A few priorities collide on the testing surface:

- **Capabilities make stubbing free.** Because time, randomness, and
  I/O are *injected* (see
  [`08-io-and-filesystem.md`](08-io-and-filesystem.md)), `std` only
  needs to provide the deterministic implementations and the test
  harness wires them in. There is no "monkey-patch global clock"
  pattern.
- **Readability over writability.** Assertions are the densest
  documentation a test carries; their shape decides whether a
  failing test reads as a clear story or a stack trace. Tel favours
  named helpers (`expect_eq`, `expect_contains`) over magic operators
  so the failure message can be precise.
- **One good way over many clever ones.** One assertion family, one
  property generator surface, one fixture form — not three styles per
  topic.

## Assertion helpers

The core is a small family of `expect_*` functions that record a
failure on the current test (or panic when used outside a test). Each
helper produces a structured failure message — actual vs expected,
location, fold-out context — that the runner formats.

```tel
expect(condition)                          # bare boolean
expect_eq(actual, expected)                # equality
expect_neq(actual, expected)
expect_lt(a, b); expect_le(a, b)          # ordering
expect_contains(collection, value)
expect_matches(value, pattern)             # pattern match
expect_close(actual, expected, eps)        # float comparison
```

Two specifically loud forms:

- `expect_err(result)` / `expect_err_kind(result, ErrKind.SomeKind)` —
  the result must be `Err`, optionally of a specific kind.
- `expect_aborts(|| ...)` — the closure must trigger a loud abort (a
  `must` or `todo` from
  [`03-prelude.md`](03-prelude.md), or an assertion). Useful for
  guarding panics in test-mocked code. `TODO(open): how this interacts
  with the runner's auto-breakpoint-on-panic mode; presumably the
  runner suppresses the break for `expect_aborts` blocks.`
- `expect_aborts_with(msg, || ...)` — the same, but the panic message
  must also match `msg`. This is the *"does panic with `…msg…`"* form:
  the closure must abort, and the abort's message must match the given
  pattern (a substring, or a richer matcher — `TODO(open)`). An abort
  with a *different* message fails the assertion, so a test cannot pass
  on the wrong panic. This is a pure-stdlib helper, **not** a compiler
  builtin: it runs the closure, catches the abort at the closure's task
  boundary (the same reification described in
  [tasks](../14-concurrency-and-parallelism/02-tasks.md#tasks-are-the-panic-boundary)),
  and inspects the resulting `PanicInfo`.

  ```tel
  expect_aborts_with("index out of range", || data[99])
  ```

  `TODO(open): the `_with` suffix mirrors `expect_err` / `expect_err_kind`.
  Alternatively `expect_aborts` could take an optional message argument
  (`expect_aborts(|| ..., msg = "...")`). Lean: the separate `_with`
  name, so the common no-message case stays a bare call. Also decide the
  matcher kind (substring / glob / regex / structured) and keep it the
  *same* matcher `fails_to_compile` uses for its diagnostic, so there is
  one matching vocabulary across the test surface.`

`TODO(open): consider AssertJ-style fluent assertions
(`assertThat(x).isGreaterThan(...).isLessThan(...)`). Tel favours
named functions over fluent chains, but a small builder shape might be
worth it for collection assertions. Decide after the prelude is final.`

### Soft assertions

A `soft { ... }` block (described in
[`../14-testing/01-testing.md`](../14-testing/01-testing.md)) lets
multiple `expect_*` calls all fail without stopping the test. The
stdlib side is a thin context that the helpers consult to decide
whether to throw or record. Outside a `soft` block they throw; inside
one they accumulate.

A dependent-check pattern that is awkward:

```tel
soft {
    let user = expect_ok(load_user(id))
    # below this point, user is None if load_user failed —
    # the soft block must short-circuit dependent checks, not NPE
    expect_eq(user.name, "Ada")
    expect_eq(user.email, "ada@example.com")
}
```

`TODO(open): how the soft block expresses "if this fails, skip the
checks that depend on it." Candidates: a `then` chain inside the
block; automatic dependency tracking through bound names; an explicit
`given { ... }` sub-block whose failure skips the rest. This is a
real pain point with AssertJ; pick a shape that makes
the dependency visible without being verbose.`

### Soft and parameterised

A soft block inside a parameterised test produces a per-row collection
of failures. The runner renders this as a table (rows down,
expectations across). No special API on the stdlib side; the soft
context simply tags each failure with the current parameter row, which
the runner already tracks.

## Property generators

A *strategy* describes how to generate values of a type and how to
shrink a failing value toward a minimal counter-example. The stdlib
exposes:

- **Built-in strategies** for primitive and stdlib types: `any[Int64]()`,
  `any[Str]()`, `any[List[T]]()` (where `T` is itself generatable),
  `range(lo, hi)`, `one_of(strategies)`, `weighted(...)`.
- **Derived strategies** for user records and unions, generated by a
  `derive(Arbitrary)` attribute. The derivation handles the common
  case; refining types (`EuroAmt`, bounded numerics, `NonEmpty[List]`)
  bring their refinements with them, so generated values respect the
  constraints by construction.
- **Filter and map** combinators (`s.filter(p)`, `s.map(f)`,
  `s.flat_map(f)`).
- **Shrinking** is automatic for built-in and derived strategies;
  hand-written ones declare a shrink function.

```tel
test fn round_trips_identity(s in any[ValidJson]()) {
    expect_eq(render(parse(s)), s)
}

test fn discount_within_bounds(t in any[Tier](), n in range(0, 100)) {
    let p = discount(t, Quantity(n))
    expect(p >= Percent(0) and p <= Percent(100))
}
```

The seed used for a property test is captured in the runner's failure
report, so a shrunk counter-example reproduces deterministically.

`TODO(open): property testing is under-specified. Pin down the
strategy combinator set and whether shrinking is on by default for
hand-written strategies (it should be, but is harder to derive).`

## Declared example values and counter-examples

A type can carry a small set of **canonical example values** — and, just
as usefully, **counter-examples**: values that look plausible but are
deliberately *not* valid for the type. The declaration sits next to the
type definition, so the examples travel with it:

```tel
# Mass is a non-negative weight, in kilograms.
unit Mass = Real64 where self >= 0.0

examples        Mass = [Mass(0.0), Mass(1.1), Mass(5_000.0)]
counterexamples Mass = [Mass(-1.0)]      # negative mass must be rejected
```

The values serve three roles at once:

- **A property-test corpus.** The property runner exercises every
  declared example directly before random generation starts, and reuses
  the examples as shrink targets — so a shrunk counter-example lands on a
  value a human already recognises. Hand-picked examples supply the base
  cases pure random generation is slow to reach: the zero, the empty
  list, the value sitting exactly on a boundary.
- **An executable boundary specification.** A counter-example asserts
  that *construction fails*: `Mass(-1.0)` must be refused by the
  refined-type constraint
  ([`../05-types/12-refined-types.md`](../05-types/12-refined-types.md))
  or the record invariant
  ([`../10-data-modelling/01-records.md`](../10-data-modelling/01-records.md)).
  The runner checks that every counter-example is rejected, so a type's
  valid range cannot silently widen without a test going red.
- **Documentation.** `tel doc` renders the examples as concrete
  instances of the type (see
  [`../18-tooling/10-documentation-generator.md`](../18-tooling/10-documentation-generator.md)),
  so the reference shows *what a `Mass` looks like*, not just its
  constraint.

Examples compose with derived strategies: when `any[T]()` builds a
record whose fields have their own declared examples, it can draw field
values from those corpora as well as from random generation, biasing
toward values a domain expert flagged as interesting.

`TODO(open): spelling — `examples` / `counterexamples` declarations, an
`@example` attribute, or a `test examples` block like the trait form
below. Keep one vocabulary with the per-impl trait examples; a per-type
example list and a per-impl example list are the same idea at different
scopes.`

`TODO(open): whether examples are optional, encouraged-by-lint, or
required. Lean: optional in general, but the linter can require them for
public refined types and records that carry a non-trivial constraint or
invariant — the constraint is exactly where a hand-picked boundary
example earns its keep. Counter-examples have no trait-test analogue
yet; decide whether a trait wants "instances that must *not* satisfy the
contract" too.`

## Example instances and trait tests

The trait-test form in
[`../14-testing/01-testing.md`](../14-testing/01-testing.md) requires
each trait implementer to supply *example instances*. The stdlib
provides the form a trait uses to *demand* them and the form an
implementer uses to *supply* them:

```tel
trait Cache[K, V] {
    fn get(k: K) -> Option[V]
    fn put(k: K, v: V)

    test examples: Iter[impl Cache[K, V]]
}

impl Cache[Int64, Str] for HashCache {
    ...

    test examples = [
        HashCache::empty(),
        HashCache::with_capacity(16),
    ]
}
```

The `test examples` declaration is a *test-only* property on the impl
— production code cannot read it, and the runner uses it to feed
trait tests. An impl that does not supply examples is a compile-time
error when the trait declares them as required.

`TODO(open): exact spelling — `test examples`, `for_tests`, an
attribute, etc. The point is that the obligation is visible at the
trait declaration so an impl knows what it owes.`

### "Find every instance of this type"

A test sometimes wants every concrete implementer of a trait or every
instance of a type. The stdlib exposes a *test-only* reflection
helper (`test::all_impls_of[T]()`) that returns a list of types or
instances at compile time. This is **not** general reflection: it is
restricted to test scope, restricted to compile-time evaluation, and
returns types rather than runtime metadata. It cannot defeat
capabilities or AOT compilation.

`TODO(open): this is the only place in std that touches reflection,
and it sits uncomfortably with the *no reflection* antifeature in
[`../02-philosophy/04-antifeatures.md`](../02-philosophy/04-antifeatures.md).
The antifeature is about *runtime* reflection; this is *compile-time*
enumeration in test scope only. Document the distinction explicitly in
the antifeatures chapter once accepted.`

## Fixtures

A *fixture* is a reusable test value or a reusable setup-and-teardown
pair. Tel's fixture surface is small:

- **Pure value fixtures** are ordinary `test fn` declarations that
  return a value:

  ```tel
  test fn fixture_default_order() -> Order { ... }
  ```

  Tests call them like any other helper. The runner does not cache
  them between tests — see the isolation discussion in
  [`../14-testing/01-testing.md`](../14-testing/01-testing.md).

- **Scoped fixtures** carry a setup-and-teardown lifecycle and are
  declared inside a test group (working name):

  ```tel
  test group server {
      fixture let server = HttpServer::start(...)
      teardown { server.stop() }

      test fn ... { ... }
  }
  ```

  Scope determines lifetime: a per-group fixture is built once per
  group, a per-test fixture is rebuilt for each test.

- **Shared immutable fixtures.** A fixture marked
  `@share_immutable` (working name) is shared across tests without
  rebuilding, as a performance optimisation. The compiler checks the
  fixture's type is immutable; mutable fixtures cannot use the
  attribute. `TODO(open): naming; whether the immutability check is
  inferred from the type system or asserted.`

## Mock helpers

The language-side mock affordances live in
[`../14-testing/01-testing.md`](../14-testing/01-testing.md); `std`
provides the value-level helpers:

- **Spy capabilities.** `test::spy[Cap](cap)` wraps a capability and
  records every call made through it. After the test, `spy.calls()`
  returns the recorded calls for assertion.

  ```tel
  let log_spy = test::spy(log)
  do_work(log_spy)
  expect(log_spy.calls().contains(LogCall("work done")))
  ```

- **Stub builders.** `test::stub[T](default = ..., on = [...])` builds
  a value implementing `T` whose unmocked methods follow the chosen
  default policy (return default / call real / fail).
- **Frozen-state recorders.** `test::record(value)` captures a value
  to a per-test scratch file; a subsequent `test::recorded(name)`
  returns it for replay-style tests.

## Deterministic capability stubs

Several capabilities have stdlib-provided deterministic stubs that the
test runner wires in by default:

- **`FixedClock(at: Instant, tick: Duration)`** — `now()` returns
  `at`, and each call advances by `tick` (which may be zero). Doubles
  as a stub for time-driven tests outside the test runner (e.g.
  reproducing a production trace).
- **`SeededRandom(seed: Int64)`** — a PRNG capability with a captured
  seed. The seed is also reported in the runner's failure output for
  reproducibility.
- **`ScopedFilesystem(root: Path)`** — wraps the host filesystem
  capability so that reads outside `root` fail and writes are
  redirected into a per-test scratch area.
- **`InMemoryNetwork()`** — a network capability whose responses are
  scripted; useful for unit-testing code that holds a real network
  capability in production.

These are ordinary types in `std`, not magic. A script can also
construct them outside a test, which is exactly the *reproducible by
default* maxim:
[`../02-philosophy/02-maxims.md`](../02-philosophy/02-maxims.md).

`TODO(open): "deterministic clock that auto-ticks on idle" — there are
JMH-style warmup and clock-related delicacies. A more elaborate
fake clock that integrates with task scheduling is probably needed for
testing async code. Defer until the concurrency chapter pins down the
task surface.`

## Compile-time `assert` and `require`

Tel's assertion story is a spectrum: the compiler checks what it can
prove, runtime checks fill the rest. The stdlib exposes two surfaces:

- **`assert(cond)`** — runtime check, with compile-time elimination
  when the compiler can prove `cond`. The diagnostic message is
  derived from the source expression.
- **`require(cond)`** — *compile-time* check. The compile fails if
  the compiler cannot prove `cond`. Used to give the prover hints and
  to assert invariants the programmer is sure of. The boundary
  between `assert` and `require` is fuzzy and may need a refined
  story; it may be a foundation for full program
  proving.

```tel
fn first(li: List[T]) -> T {
    require(li.len() > 0)        # compile error if prover cannot show this
    li[0]                         # bound check elided, prover knows len > 0
}
```

`TODO(open): pin the assertion family down. Candidates:
*always-on assert*, *debug-only assert*, *debug-only assert that
informs the optimiser*, *debug-only assert with no-side-effect
constraint*, *check-result-only assert*. Each has real use cases; the
right answer is probably two or three, not all five. Coordinate with
the error-handling chapter (`13-error-handling/04-panics-and-aborts.md`)
and the contracts story in
[`../02-philosophy/03-features.md`](../02-philosophy/03-features.md).`

`TODO(open): may code *rely* on a debug-only
assert holding even when the assert is disabled (Rust-style
`unreachable_unchecked`). The safer answer is "no" — undefined
behaviour conflicts with Tel's *never panic on the host* rule. A
separate `promise(cond)` form, which the compiler may use as a
proof hint but which is checked when in doubt, is probably the right
shape. Decide.`

### Debug-only vs always-on

Tel's preference is **assert-always**, because *the chance of failure
differs by site*: pre-conditions fail more often (called by different
code paths, possibly by different programmers), post-conditions
mostly hold and are documentation. Only ~10% of
asserts are hot enough to warrant skipping; the rest pay for
themselves in caught bugs. Hot-path asserts can be tagged for
elimination, but the default is on.

`TODO(open): tagging mechanism — `@hot` on the function, a
per-assert flag, a profile feeding back into the build. There is a
worry about asserts becoming unreliable (`@Nonnull`-style) if
everyone runs release builds; the default-on policy is the answer.`

## Integration with logging

A test inherits a logger capability from the runner. The stdlib
provides a `test::capture_logs(level)` helper that returns a logger
whose output is captured for assertion:

```tel
let (log, captured) = test::capture_logs(Level.Debug)
do_work(log)
expect(captured.entries().any(|e| e.message == "started"))
```

This is also how a test raises its own log verbosity for the duration
of the test without touching global state.

## See also

- [Prelude](03-prelude.md) — `must`, `todo`, `assert_unreachable`
- [I/O and Filesystem](08-io-and-filesystem.md) — capabilities the
  test stubs wrap
- [Time](09-time.md) — `Clock` and the deterministic stub
- [Randomness, Hashing and Crypto](15-randomness-hashing-and-crypto.md) —
  `Random` and the seeded stub
- [Observability and Logging](14-observability-and-logging.md) —
  `capture_logs` and benchmark helpers
- [Concurrency Utilities](12-concurrency-utilities.md) — task
  isolation that the runner builds on
- [Testing tool](../14-testing/01-testing.md) — what `tel test` does
  with all of this
- [Antifeatures](../02-philosophy/04-antifeatures.md) — why
  test-only reflection is the only reflection
