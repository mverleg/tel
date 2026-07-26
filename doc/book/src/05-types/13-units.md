# Physical Units

A **unit type** attaches a physical dimension — weight, length, velocity,
temperature — to a numeric value, so the compiler can stop a script from adding a
weight to a length, or from forgetting a unit conversion. It is a specialised
kind of [refined type](12-refined-types.md).

This splits into a committed part and a deferred part:

- **Committed — construct-time unit newtypes.** Wrapping a scalar so a `Celsius`
  is not a `Fahrenheit` and a `Meter` is not a `Second` is just a
  [refined / newtype](12-refined-types.md) (rung 1b in
  [TIP-0004](../tips/0004-how-far-refinement-types-go.md)), with the operators
  you want carried over by [operator overloading](../09-functions/09-overloading-and-dispatch.md).
  That is available in Tel1.

- **Deferred — the dimension-aware algebra.** A dedicated `unit` construct that
  knows `weight * velocity` yields `momentum`, rejects `Temperature * Temperature`,
  and tracks SI prefixes and derived units is **deferred**. The retired
  `100 kg` suffix syntax, the `unit`-declaration sketch, and the open questions
  (SI scaling, display units, a built-in catalogue, keyword-vs-library) now live
  in [Deferred Features → Dimensional-analysis units](../20-appendix/06-deferred-features.md#dimensional-analysis-units-a-dedicated-unit-construct).

## See also

- [Refined and Newtype Types](12-refined-types.md) — the committed mechanism.
- [Primitive Types](02-primitive-types.md) — the numeric base, including
  `Decimal` for currency.
- [Deferred Features](../20-appendix/06-deferred-features.md#dimensional-analysis-units-a-dedicated-unit-construct)
  — the deferred dimensional-analysis design.
