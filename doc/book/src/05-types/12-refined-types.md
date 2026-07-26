# Refined and Newtype Types

A **refined type** is an existing type plus a name and, optionally, a value
constraint. `EuroAmt`, `Id[Person]`, `NonEmpty[List[T]]`, `Real64 > 0` — these are
the workhorses of Tel's "make invalid states unrepresentable" promise. They are
cheap to define and the language pushes you to use them freely.

## What — three things in one feature

"Refined type" covers a spectrum, treated here together:

1. **Newtype wrappers** — a new, distinct type with the same representation as an
   existing one. `type EuroAmt = newtype Decimal` is a `Decimal` underneath but
   is *not* interchangeable with `Decimal` or with another currency. The purpose
   is nominal distinctness: the compiler stops you adding a `EuroAmt` to a
   `UsdAmt` or passing a `Id[Order]` where a `Id[User]` is wanted.

2. **Constrained types** — a type narrowed by a predicate on its value:

   ```tel
   type Ratio       = Real64 > 0.0
   type Probability = Real64 where 0.0 <= self and self <= 1.0
   type NonEmptyText = Text where len(self) > 0
   ```

   A value can only have the type if it satisfies the predicate; the constraint
   is checked when the value is constructed.

3. **Invariant-carrying records** — a record whose constructor enforces a
   relationship between fields (covered with records in
   [`../10-data-modelling/01-records.md`](../10-data-modelling/01-records.md)).

## Recommended use — newtypes are the domain ↔ hardware boundary

The single most important habit Tel pushes: **don't pass domain values around as
bare primitives — give each one a newtype.** A newtype is where domain knowledge
gets translated into a hardware decision, and that translation should happen in
exactly one place.

```tel
type Age      = newtype Int16     // a human age: small, non-negative-ish
type UserId   = newtype Int64     // an identity, never arithmetic
type EuroAmt  = newtype Decimal   // money, so exact base-10
type Celsius  = newtype Real32    // a sensor reading; single precision is plenty
```

Each line answers two questions at once:

- **What is this, in the domain?** `Age`, `UserId`, `Celsius` — names the
  concept, and the compiler then stops an `Age` being added to a `UserId` or
  passed where a `Celsius` is wanted.
- **What width does the machine use for it?** Picking `Int16` for `Age` is a
  judgement — "no age needs more than 16 bits" — and writing it as a newtype
  *records that judgement* in one spot. If it ever needs to change (say `Age`
  must cover geological time), one definition changes and every use follows.

This is why Tel commits to [explicit widths](02-primitive-types.md): the width
is not noise to be hidden behind an abstract `Int64`, it is a real engineering
choice that belongs in the domain type's definition. Reach for a bare `Int64` /
`Real64` only for throwaway local arithmetic; the moment a value *means*
something, wrap it.

## Trait inheritance for newtypes

A newtype is nominally distinct from the type it wraps, so the question is
*which* of the wrapped type's capabilities come along. Tel makes this
**opt-in**: a newtype lists the traits it inherits, written as a
[trait list](../10-data-modelling/03-traits-or-interfaces.md#trait-lists-and-bound-aliases)
after `:`.

```tel
type EuroAmt = newtype Decimal : Eq + Ord + Add + Sub + Neg + Show
type UserId  = newtype Int64   : Eq + Ord + Hash        # NO arithmetic
```

`EuroAmt` forwards arithmetic from `Decimal`, so two amounts add; `UserId`
deliberately omits `Add`, so `id1 + id2` is a compile error — usually the whole
reason the newtype exists.

**Why opt-in, not inherit-everything.** A newtype's frequent job is to *remove*
an operation that is meaningless on the domain value (adding two user ids,
multiplying two dates). If inheritance were automatic, that safety would be
opt-*out*, and forgetting to opt out silently re-introduces the very bug the
newtype was meant to prevent. Opt-in keeps the default safe — a fresh newtype
carries nothing but its own identity until told otherwise — at the cost of naming
the capabilities you *do* want. *Serves: safety over flexibility.*

That naming cost is paid down by
[bound aliases](../10-data-modelling/03-traits-or-interfaces.md#bound-aliases):
bundle the common tiers once and inherit one in a single word.

```tel
type UserId  = newtype Int64   : Ordinal          # Ordinal = Eq + Ord + Hash
type EuroAmt = newtype Decimal : Numeric + Show    # Numeric = Ordinal + Add + Sub + ...
```

Inheriting forwards the wrapped type's *existing* impl. When the underlying type
does not implement a trait you want, reach for `derive` (synthesise a fresh impl)
or a hand-written `impl` on the newtype instead — the same trait-list grammar,
a different source for the implementation.

**Nothing is inherited automatically — not even `Eq`.** The most opaque newtype
carries only its own identity; comparability is listed like any other capability
(directly or via an alias bundle), so there is exactly one rule and a newtype
never silently keeps an operation it was never asked to keep.

A newtype is also the **escape hatch for the orphan rule**: when you need to
implement a foreign trait on a foreign type — which neither crate may do
directly — wrap the type in a newtype and write the impl on the wrapper, which is
unambiguously yours. See
[Traits — coherence](../10-data-modelling/03-traits-or-interfaces.md#coherence-the-orphan-rule-and-specialisation).

## Why — validation is a type-system job

The safety priority puts *validation* on the same footing as typing: the
compiler should catch a bad value, and where it cannot prove safety statically,
a runtime check fires at the boundary. Refined types are how that happens for
data:

- A bare `Real64` says "a number". `Ratio = Real64 > 0` says "a number we have
  *established* is positive" — every later use can rely on it without
  re-checking.
- Newtypes stop whole categories of mix-up (currencies, ID kinds, units) at
  compile time, for zero runtime cost.
- A core promise: **anything a built-in type can do, a wrapper *can* do too** —
  operators, ordering, hashing, formatting are all available to carry over. A
  newtype inherits them from the wrapped type *by listing them*, not
  automatically (see [trait inheritance](#trait-inheritance-for-newtypes) below);
  the point is that nothing is structurally lost, so a wrapper is never the
  annoying choice. See
  [`../10-data-modelling/07-equality-and-hashing.md`](../10-data-modelling/07-equality-and-hashing.md).

## How constraints flow — the dependent-ish question

There is a wanted behaviour that is the hard, open part of this
feature. The compiler should ideally *track* constraints through operations:

```text
x: Real64  / y: Real64        -> (Real64 | inf)    // y might be zero
x: Real64  / y: Real64 != 0   -> Real64            // y ruled away from zero
x: Real64>0 / y: Real64>0     -> Real64 > 0         // both positive -> positive
```

That is, the result type of an operation depends on the *constraints* of its
inputs, and a more-constrained signature is a specialization of a
less-constrained one (see
[`09-subtyping-and-variance.md`](09-subtyping-and-variance.md)). This is
genuinely useful — it would let division be total when the divisor is known
non-zero — but it is also a step toward **dependent types**, and full dependent
typing is the kind of heavyweight, compile-slowing, hard-to-port feature the
priorities push back on.

TODO(open): how far constraint propagation goes. Options, roughly in increasing
power:

- **(a) Constraints checked only at construction.** A `Real64 > 0` is verified
  when made; arithmetic on it produces a plain `Real64`. Simple, predictable,
  cheap. Loses the "positive / positive -> positive" inference.
- **(b) Limited propagation** for a fixed, built-in set of cases (non-zero
  divisor, non-negative results, range arithmetic). Useful, but each rule is
  bespoke and the set must be frozen at 1.0.
- **(c) General refinement / dependent types.** Most powerful, hardest to
  implement, slowest to compile, hardest to keep identical across hosts.

Lean: **(a) plus a small, frozen (b) subset** — most of the value, none of the
solver. The full survey of where this call lands — what other languages settled
on, why the SMT/dependent end is rejected for Tel1, and the proposed tier line —
is in [`../tips/0004-how-far-refinement-types-go.md`](../tips/0004-how-far-refinement-types-go.md).
The philosophy chapter does not rule on dependent types; this is a
philosophy gap to flag. Dependent typing is itself "not pinned
down", and the [generativity / branding](#branded-types) pattern is a candidate
*alternative* to `0 <= x < 5`-style type information.

TODO(open): the "branding as an alternative to bounds" remark suggests Tel might
not need value-range types at all if branded indices cover the array-bounds
case. Decide whether refined numeric ranges and branding are competing or
complementary.

## Constraints and the outside world

A constraint enforced in a constructor is only as good as the paths that build
the value: a deserializer, or any host-side code
that produces a Tel value, must also respect the invariant.

The rule: a refined type's constraint is checked **wherever a value of that type
is constructed**, including at the host boundary and by generated
(de)serialization code — there is no back door that produces an unchecked
`Ratio`. A value crossing in from the host is validated as it is adopted into a
Tel refined type.

TODO(open): exact mechanism for making deserializers and host-boundary
conversions go through the constraint check — likely tied to the schema-first /
codegen serialization story and the host-boundary chapter.

## Branded types and generativity

A distinct flavour of "refined" — **branded types**, where each *instance*
carries a unique compile-time identity so an index from one structure cannot be
used on another (the generativity pattern) — is **deferred**. The mechanism is
advanced, brand-preservation is awkward, and its payoff is mainly eliminating
bounds checks. The design sketch and rationale live in
[Deferred Features → Branded types and generativity](../20-appendix/06-deferred-features.md#branded-types-and-generativity).

## Physical units

Unit-of-measure types (`weight`, `velocity`, `kg` vs `mg`) are a specialised
refined-type story and have their own page — see
[`13-units.md`](13-units.md).

## A composable validator catalogue

`TODO(open): **A library of small, composable validators.** Refined-type
predicates are written ad-hoc above (`Real64 > 0`, `len(self) > 0`); a
catalogue of named, composable validators would let the same predicates
appear in three roles: as the predicate of a refined type, as a
[design-by-contract](../02-philosophy/03-features.md) pre/post-condition,
and as a [config-DSL](../19-use-cases/) form-field rule. Baseline
candidates: positive / non-negative, at-least-one-element, sorted,
satisfies-regex, in-range, distinct, one-of-set. Each must be cheap to
write, cheap to combine (`positive and divisible_by(2)`), and surface a
useful error message when it rejects a value. Decide whether these live in
`std` as a `validate` module, or in [`12-refined-types.md`](12-refined-types.md)
as the refined-type predicate library — most likely both, with one
reaching into the other.`

## Example and counter-example values

A refined type is the natural place to pin down its own boundary with
**declared example values** and **counter-examples**. `Mass` has
examples `0.0`, `1.1`, `5_000.0` and the counter-example `-1.0`; the
counter-example states that construction *must* reject a negative mass.
These declarations are not narrative — the property runner exercises the
examples as a seed corpus and verifies that every counter-example is
refused, so the constraint cannot silently widen. They also feed
`tel doc`, so the reference shows concrete instances next to the
predicate. The mechanism and spelling live with the test surface in
[`../17-standard-library/19-testing-utilities.md`](../17-standard-library/19-testing-utilities.md#declared-example-values-and-counter-examples).

## Bugs this prevents

A representative selection from the catalogue
where a refined / newtype wrapper
would have caught the bug at compile time:

- **"Strategy vol" vs "natural vol" silently mixed.** A pricing function
  used one volatility representation; one fund on one day used another.
  A simulation produced updates in the standard representation, then
  looked up a "should be a lookup" value in a different representation
  and got back unaffected vols. A `StrategyVol` / `NaturalVol` newtype
  pair would have refused the cross-call at compile time.
- **`toDate(Int64)` for packed dates, `toDate(long)` for timestamps.** A
  utility class had two overloads with very different meanings; calls
  that should have taken a packed date silently ran the timestamp
  overload because the input was widened from `Int64` to `long`. With
  refined types `PackedDate` and `Timestamp`, the wrong overload is a
  type error.
- **Nanos vs millis.** A configurable time was plain `double` with no
  unit in the name or documentation. With a `Duration` / `Nanos` /
  `Millis` distinction (see [`13-units.md`](13-units.md)), the conversion
  is explicit at construction.
- **Reporting currency vs trade currency.** Greeks differed between two
  reports because one was in the trade's currency and the other in the
  reporting currency. A `Greeks[TradeCcy]` vs `Greeks[ReportCcy]`
  parameterised wrapper would make this a type error rather than a
  hunt-it-down report mismatch.
- **"Use scope X" flag vs the scope built into the object itself.** An
  override of a per-scope value didn't work because the scope was part
  of the object identity, so the override landed in the wrong scope. A
  refined `Scoped[T, Scope]` would have rejected the cross-scope
  assignment at compile time.
- **Same expiry, different timestamp.** Hours of debugging because data
  was collected for an "expiry" that turned out to differ from the
  reported "expiry" only by the time-of-day. A refined `ExpiryDate` /
  `ExpiryDateTime` distinction makes the difference visible at every
  use site.

The pattern: any time a comment, naming convention, or "we know" assumption
keeps two same-shaped values from being mixed up, that's a candidate for a
refined-type wrapper. The language *encourages* the wrapper by making it
costless (anything a primitive does, the wrapper does too); the
[linter](../18-tooling/07-linter.md#built-in-pattern-lints-from-the-bug-catalogue)
*discourages* the bare primitive from the other side, flagging a signature
with several same-typed primitives where a wrapper would stop an argument
swap.

## See also

- [Primitive Types](02-primitive-types.md) — `Decimal` and the numeric base.
- [Records](../10-data-modelling/01-records.md) — record-level invariants.
- [Dataframes](../10a-dataframes/01-overview.md) — a `newtype` is the explicit
  nominal cap on an otherwise structural derived row schema, and column names are
  statically-checked labels (a compile-time-only refinement on field access).
- [Subtyping and Variance](09-subtyping-and-variance.md) — constraint narrowing.
- [Physical Units](13-units.md).

TODO: review
