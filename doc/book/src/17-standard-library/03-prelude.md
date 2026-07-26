# Prelude

<!-- TODO: review -->

## What

The *prelude* is the small set of `std` names available in every script
without an explicit import — the core types (`Option`, `Result`, the numeric
and text types), the workhorse collections, and a handful of free functions
that almost every script touches.

The prelude is deliberately small. It holds what a *30-line embedded hook*
needs, not everything `std` offers. Larger or more specialised areas
(networking, data formats) are imported explicitly.

**An embedding can extend the prelude for its domain.** A specific host (a game
engine, a config runtime, a data pipeline) may add its own names to the prelude
its scripts see. The `std` prelude described here is the *baseline* every Tel
program shares, not a ceiling.

**Operator-backed names are implicitly present and cannot be imported.** Every
built-in operator's function form — the comparison/arithmetic names like `gt`,
`lt`, `add`, and the
[fallback operator](../07-expressions/11-fallback-operator.md) — is part of the
prelude automatically. There is nothing to import (the operator *is* the name),
so these are not listed among the candidates below.

`TODO(open): the exact contents of the prelude are not pinned down. The
sections below describe the candidates; the final list is
a 1.0 decision.`

## Why

A 30-line modding hook should not open with a wall of imports. At the same
time, the prelude is a stability commitment: anything in it is a name every
script can assume forever, so it is curated tightly.

## Candidate contents

### A `todo` placeholder

A top-level placeholder — written without a hyphen, e.g.
`todo` — simply aborts when reached, so an unfinished branch type-checks
but fails loudly at runtime.

```tel
fn discount(tier: Tier) -> Percent {
    match tier {
        Tier.Gold   => Percent(10),
        Tier.Silver => Percent(5),
        Tier.Bronze => todo("bronze discount not decided yet"),
    }
}
```

There are also *conditional* variants on `Option` and `Result` — an "unwrap or
abort" — that abort only on the empty / error case. These are the explicit,
loud way to assert a value is present; they are not a quiet default.
`TODO(open): naming and exact spelling of the placeholder and the unwrap-style
helpers; coordinate with the error-handling chapter.`

### Lazy values

**Lazy values** are a first-class concept — either a
language feature or a prelude type `Lazy[T]`. The motivating use cases are
spread across the library: logging arguments, asserts, expensive default
values, and the body of a memoised computation. The wishlist:

- Evaluate the producing expression at most once, on first use.
- Make it visually obvious at the call site that the argument is deferred —
  side effects may not have happened yet. Swift's `@autoclosure` is rejected
  on this point: it hides the laziness from the reader.
- After the first evaluation the value reads like any other `T` — no
  `.get()` ceremony, no manual dereference.

A sketch:

```tel
# `lazy` marks the argument as deferred; the body runs on first use only.
let report = lazy { build_report(orders) }
log.debug("report ready", report)        # `report` is built here, once
return report                            # reads as if it were a plain Report
```

`TODO(open): whether `Lazy[T]` is a prelude type, a language form (e.g. a
`lazy` keyword or postfix marker), or both — this is undecided.
The common shape is: argument position uses light syntax, return-type position
uses an explicit type. Pure functions could potentially be lazy by default
since strictness is not observable, but capturing-by-value matters for
soundness. Coordinate with the language chapter on laziness and the logging
section in [`14-observability-and-logging.md`](14-observability-and-logging.md).`

### Combinators on `Option` and `Result`

The prelude includes the small set of combinators that prevent nested-shape
pitfalls — `map`, `and_then`, `or_else`, fallback — so that `Result[Option[T]]`
and `Option[Option[T]]` collapse the obvious way. The danger pattern
is the `Option[Result[Option[T]]]` mess that grows when each library
adds its own wrapper; the prelude's combinators are the answer. See also the
discussion of fallible iterators in
[`05-iteration-and-streams.md`](05-iteration-and-streams.md). `TODO(open):
final combinator set and naming.`

### A loud-abort family

In addition to `todo`, the prelude exposes a small family of loud, explicit
abort helpers. They are the *only* sanctioned way to turn an
`Option` or `Result` into a bare value:

- `must(opt)` / `must(result)` — extracts the value or aborts with a
  diagnostic. The name is deliberately short so the call site reads as a
  promise, not a polite request.
- `assert_unreachable("reason")` — abort marker for branches the programmer
  has proven impossible. In debug builds these always run; in optimised
  builds an implementation may treat them as a compiler hint.
- `unimplemented("optional note")` — semantic synonym of `todo` for entire
  function bodies, kept distinct so reviewers can grep for one or the other.

`TODO(open): the exact set, the precise spellings, and whether
"assert_unreachable" doubles as an optimisation hint (à la Rust's
`unreachable_unchecked`). Coordinate with the error-handling and contracts
chapters.`

### Small everyday helpers

A handful of micro-utilities are too dull to import but too useful to omit:

- `coalesce(a, b, c, ...)` — return the first non-`None` argument, an
  N-ary generalisation of the fallback operator.

(`clamp(x, lo, hi)` and `chance(p)` were considered and **left out** — useful,
but not common enough to earn a permanent, always-in-scope name. They live in
the relevant `std` modules — `clamp` with the numerics helpers, `chance` with
randomness — and are imported when needed.)

`TODO(open): the full list of "small helpers" admitted into the prelude. The
inputs gesture at many candidates; the prelude must stay small enough to
memorise.`

## See also

- [Standard Library Organisation](01-stdlib-organisation.md)
- [Core Collections](04-core-collections.md)
- [Observability and Logging](14-observability-and-logging.md) — uses lazy args
- [Randomness, Hashing and Crypto](15-randomness-hashing-and-crypto.md) — backs `chance`
