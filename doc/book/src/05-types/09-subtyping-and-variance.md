# Subtyping and Variance

Subtyping is when a value of one type may be used wherever another is expected.
Tel keeps subtyping **minimal and predictable** — there is no general
subtype hierarchy among user types, and the one place subtyping does appear,
union membership, is simple set inclusion.

## What — where subtyping exists

Tel has no class inheritance and no general subtyping between user-defined types
(see [antifeatures](../02-philosophy/04-antifeatures.md)). A `Cat` is not a
subtype of an `Animal` record. The places a value of one type is accepted where
another is named:

- **Union membership.** A value of type `A` may be used where `(A | B | C)` is
  expected — `A` is a member of that union. More generally a smaller union is
  usable where a larger one containing all its members is expected: `(A | B)` is
  accepted where `(A | B | C)` is wanted. This falls straight out of unions being
  *sets of member types* (see
  [`../10-data-modelling/02-union-types.md`](../10-data-modelling/02-union-types.md)).
- **The never type.** `Never` has no values, so it is usable in any context —
  see [`14-never-type.md`](14-never-type.md).
- **Refined types narrowing.** A more constrained refined type is usable where a
  less constrained one is expected — `Real64 > 0` where `Real64` is wanted. See
  [Refined types and subtyping](#refined-types-and-subtyping) below.

That is the whole list. There is no top type that everything is a subtype of,
and no subtyping you can introduce yourself.

## Why — untagged unions make subtyping fall out, and that is the point

Untagged unions have a genuine benefit over tagged enums: with
untagged unions, `Text` *is* assignable to `(Text | None)`. So if a trait declares
a method returning `(Text | None)`, a concrete implementation may legitimately
*always* return `Text`, and a caller holding that concrete type statically knows
it never sees `None` — no redundant check. A tagged enum cannot express this:
`(A | B)` would not be a subtype of `(A | B | C)`, and the variants are not types of
their own.

This is exactly the subtyping Tel wants — narrow, structural, falling out of the
union model — and exactly the kind it avoids elsewhere. General nominal subtype
hierarchies (inheritance) leak implementation across layers and complicate the
type checker; that cost is rejected. The union case costs nothing extra because
a union *is* a set.

## Variance

Variance is how subtyping of a generic type relates to subtyping of its
arguments. Tel's answer is short because there is so little subtyping to be
variant over: **generic type parameters are invariant** (see
[`07-generics.md`](07-generics.md)). `List[A]` and `List[(A | B)]` are unrelated
types even though `A` is usable where `(A | B)` is.

Invariance is the safe default — it avoids the soundness holes variance opens up
once mutation is involved — and with no general subtyping there is little to
lose by it. A function that should accept either writes a generic parameter or a
union explicitly, rather than relying on variance.

TODO(open): one consequence worth confirming — because generics are invariant, a
`List[Text]` is *not* a `List[(Text | None)]`. The union-subtyping benefit above
applies to a value's *own* type, not to a collection's element type. Confirm
this is the intended, documented behaviour (it should be — it matches
invariance) and that the asymmetry is acceptable.

## Refined types and subtyping

Refined types ([`12-refined-types.md`](12-refined-types.md)) form a natural
narrowing relation: `Real64 > 0` is "more specific than" `Real64`, so a `Real64 > 0`
value flows into a `Real64` slot freely, while the reverse needs a checked
conversion. This is a wanted feature — the type of
`x: Real64 / y: Real64 > 0` should itself be known more precisely (`Real64`, not
`(Real64 | inf)`) because the divisor is constrained away from zero.

This is a *narrowing* subtype relation on constraints, not a hierarchy of named
types. How far Tel's type checker can *propagate* such constraints through
arithmetic — i.e. how dependent-ish the system gets — is the central open
question for refined types; see [`12-refined-types.md`](12-refined-types.md).

## See also

- [Generics](07-generics.md) — invariance in context.
- [Union Types](../10-data-modelling/02-union-types.md) — union membership as the
  main subtyping rule.
- [Refined Types](12-refined-types.md).
- [Antifeatures — no inheritance, no subtyping](../02-philosophy/04-antifeatures.md).

TODO: review
