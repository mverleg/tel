# Ordering

**Ordering** is what lets values be sorted, kept in a sorted collection, used
as a key in an order-preserving map, or compared with `<`, `<=`, `>`, `>=`.
Like equality, it is a *value-shaped* property, derived structurally by
default and overridable when the structural answer is wrong.

## What — total and partial orderings

Tel exposes two ordering traits:

- **`Ord`** — total ordering: `cmp(a, b)` returns `Less`, `Equal`, or
  `Greater` for any two values. Implies the standard mathematical properties
  (antisymmetry, transitivity, totality), and implies equality consistent with
  the ordering.
- **`PartialOrd`** — ordering that may not be total. Some pairs may be
  *incomparable*: `cmp(a, b)` returns `None`. The motivating case is `Real64`
  with `NaN` — `NaN` is not less than, equal to, or greater than anything,
  including itself.

`Int64`, `Text`, `Decimal`, and most user records satisfy `Ord`. `Real64`
satisfies `PartialOrd`. Containers that need a total order — a sorted set, a
binary search — require `Ord` and reject `PartialOrd`-only types.

```tel
fn sort[T: Ord](items: List[T]) -> List[T] { ... }
fn min[T: PartialOrd](a: T, b: T) -> Option[T] { ... }   # may be incomparable
```

## Why — total vs partial is a real distinction the type system tracks

To be explicit: bitwise equality of `Real64` is not the same as
mathematical equality, and the same goes for ordering. A type system that
hides the difference (Java's `Comparable` quietly returning whatever for
`NaN`) lets sorting routines produce undefined results. Tel surfaces the
difference at the bound — `sort` requires `Ord`, so `List[Real64]` does not
sort with the obvious call.

The fix at the user level is the same pattern as for equality:

- Use `Real64` and accept that comparison is partial — handle the `None` case
  where it matters.
- Wrap in a refined type that excludes `NaN` (`type Finite = Real64 where !is_nan(self)`)
  and implement `Ord` for it.

## Auto-derivation

Like equality, ordering is structurally derivable:

- A **record** compares lexicographically by its fields, in declaration order.
  This is the obvious default, and the same rule Rust uses.
- A **tuple** compares lexicographically — positional slots by index first, then
  named fields **by name** (the named tail is unordered, so name order is the
  canonical, write-order-independent traversal).
- An **array** or **list** compares lexicographically by element.
- A **union** compares by member-type rank first (in declaration order) and by
  the inner value second. The default ranking matches the union's declaration
  order, with the option to override.

The author opts in with `derive Ord` (final spelling open); the compiler
synthesises the comparison. For records with fields that ought to compare in
a different order — or with fields that should not contribute at all — the
author writes the impl by hand.

## `Ord` implies `Eq`, and the relationship between the pairs

Two derivations fall straight out of the maths:

- **Implementing `Ord` implies `Eq`.** Two values are equal iff `cmp(a, b)`
  returns `Equal`. A type may *override* `Eq` for performance (a direct check
  faster than the full comparison), but the default is free.
- **Implementing `PartialOrd` implies `PartialEq`.** Same reasoning.
- **`Ord` implies `PartialOrd`** (a total order is also a partial order).
- **`Eq` implies `PartialEq`** (full equality is also partial equality).

These derivations save the user from writing trivial wrappers and prevent the
classic contract slip where `<` says one thing and `==` says another.

TODO(open): confirm the auto-derivation chain end-to-end. Note also
that overriding `Eq` while inheriting `Ord` is *allowed but
discouraged*, since the two must remain consistent. Lean: allowed with a
lint, not a hard error.

## Custom orderings

When the default lexicographic order is wrong — sort by a single field, ignore
case for strings, reverse a direction — the author writes a `cmp`
implementation, or, more commonly, passes a comparator to the operation.

```tel
items.sort_by(|a, b| cmp(a.priority, b.priority))
items.sort_by_key(|x| x.priority)               # convenience
```

The pattern follows the trait-bound rule: a sort takes either a `T: Ord` and
uses the default, or a comparator function over `T` and uses that. A
comparator is essentially an *associated function* a
`HashMap`-like type could take as a generic parameter (a la Java's
`Comparator`), tying back into the const-generics discussion in
[`../05-types/07-generics.md`](../05-types/07-generics.md).

A type that must be sorted two *genuinely different* ways (by age, by surname)
does **not** get two `Ord` impls — that would be an ambiguous second
implementation, the coherence hazard
[TIP-0005](../tips/0005-trait-coherence-and-the-orphan-rule.md) rules out. The
default `Ord` is the one canonical order; any other is a comparator value chosen
visibly at the call site, or carried by a collection that must retain it
(`SortedSet.with(cmp = …)`). This is distinct from *specialisation*, where one
impl refines another; unrelated orderings refine nothing.

## Operators and overloading

`<`, `<=`, `>`, `>=` are sugar for the corresponding `cmp` result; a type
that implements `Ord` (or `PartialOrd` with care) gets the operators for
free. Tel does not allow defining *new* operators (see
[antifeatures](../02-philosophy/04-antifeatures.md)), but the standard ordering
operators are overloadable through the trait.

For unrelated types — `Cat < Dog` — the operators do not compose. There is no
implicit cross-type comparison; the author writes the conversion (see
[`../05-types/11-conversions-and-coercions.md`](../05-types/11-conversions-and-coercions.md)).

## Sorted as a type-level property

An open question: can **"sorted-ness" be a property of a list's type**? —
sorting a `List[T]` produces a `Sorted[List[T]]` (a refined type) the type
system tracks. A function that needs a sorted input — binary search — would
then take `Sorted[List[T]]` and the compiler refuses unsorted inputs.

This is appealing and falls naturally out of refined types; the catch is that
such a "sorted" property only stays sound on
**immutable** values. The moment a mutating operation runs, the property is
lost. Tel's records are immutable by default, so this is less of a problem
than it looks — a "sorted list" is just a value carrying that constraint —
but interaction with any mutable-container story will need care.

TODO(open): commit to or reject `Sorted[List[T]]` as a built-in refined type
the standard library exposes. Lean: yes — it is cheap once refined types
exist (see [`../05-types/12-refined-types.md`](../05-types/12-refined-types.md)),
and the bounds-elimination benefits for binary search are real. Tied to the
constraint-propagation open question.

## Ordering and host portability

The ordering of `Text` must be **identical** across hosts. Tel cannot let one
host sort strings by Unicode code point order and another by a locale-aware
collation; reproducibility forbids it. The language pins one
host-independent ordering for `Text` (likely code-point order); locale-aware
collation is a separate, optional, *standard-library* concern, never the
default `Ord` impl.

TODO(open): commit to the canonical `Text` ordering (code-point order is the
working assumption — it matches what the language can guarantee across every
host, and locale-aware ordering is a presentation-layer concern). Tie back to
[`../05-types/03-strings-and-text.md`](../05-types/03-strings-and-text.md)
once the indexing unit is decided.

## See also

- [Equality and Hashing](07-equality-and-hashing.md) — the partner of `Ord`.
- [Traits or Interfaces](03-traits-or-interfaces.md) — `derive` and trait
  composition.
- [Refined Types](../05-types/12-refined-types.md) — `Sorted[List[T]]`,
  finite-real wrappers.
- [Collection Types](09-collection-types.md) — sorted maps and sets.
- [Primitive Types](../05-types/02-primitive-types.md) — `Real64` and `NaN`.

TODO: review
