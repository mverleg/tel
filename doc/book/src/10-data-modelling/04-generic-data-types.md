# Generic Data Types

A **generic data type** is a record, union, or alias parameterised by one or
more *type parameters*. Generic data types are how Tel models containers and
shape-shared families — `List[T]`, `Result[T, E]`, `Pair[A, B]`, `Histogram[K]`.
The generics *machinery* itself — bounds, inference, invariance, const generics
— lives in [`../05-types/07-generics.md`](../05-types/07-generics.md); this
page is about applying it to *data*.

## What — parameterised records and unions

```tel
struct Pair[A, B] {
    first:  A,
    second: B,
}

struct Box[T] { value: T }

type Option[T] = (Some[T] | None)
type Result[T, E] = (Ok[T] | Err[E])
```

The type parameters appear in brackets after the type name and may then be
used wherever a type may appear inside the declaration. A use site supplies
the parameters: `Pair[Int64, Text]`, `Option[User]`, `Result[Order, ParseError]`.

## Why generic data types matter for Tel

Tel's first home was [Apivolve](../01-overview/01-introduction.md), a
schema-evolution system. Schema modelling needs generic containers
(`Option[Field]`, `List[Field]`, `Map[Name, Field]`), and host-portable
data-transformation code needs them just as much. Generics on
data types are a *settled* feature — the open questions are about how far they
go (const generics, refined bounds), not whether they exist.

A second motivation: data-transformation pipelines
constantly produce small shape-shared families (`(K, Result[T])` for results
per key, then merged into `Result[List[(K, T)]]`). Without generics, every
such variant needs its own hand-written type. With generics plus
[tuples](../05-types/04-tuples-and-arrays.md) and a few stock containers, the
pipeline writes itself.

## Wrapper structs as tags

The single sharpest interaction between generics and Tel's union types is the
**collapse problem**: an untagged union over generics can flatten away the
distinction between its members. The fix is a *modelling pattern* — wrap each
side of a generic union in its own single-field record:

```tel
struct Ok[T]  { value: T }
struct Err[E] { error: E }
type   Result[T, E] = (Ok[T] | Err[E])
# Ok[Int64] and Err[Int64] are distinct types even though both wrap Int64.
```

The same trick keeps `Option[Option[T]]` from collapsing. See
[`02-union-types.md`](02-union-types.md) and
[`../05-types/06-option-and-nullability.md`](../05-types/06-option-and-nullability.md)
for the underlying reasoning; the upshot for generic *data type design* is:
**when defining a generic union, give each variant a wrapper struct**. The
wrappers are cheap (see [`../05-types/12-refined-types.md`](../05-types/12-refined-types.md)),
and they give each case a separately-addressable name.

## Bounds on the parameters

A type parameter may carry a [trait bound](03-traits-or-interfaces.md) that
constrains what types may be supplied:

```tel
struct SortedList[T: Ord] { items: List[T] }
struct Cache[K: Eq + Hash, V] { ... }
```

A bound is checked at the use site: `SortedList[Foo]` does not compile unless
`Foo: Ord`. As elsewhere, traits are *bounds*, never values, so a bound
restricts the parameter without becoming a member of any union.

A recurring frustration with bounds in containers: if a
constraint `T: Send` is needed for one operation on a generic struct, it tends
to end up in the struct's declaration *and* propagate to every method, even
methods that never use it. Tel's stance: bounds belong on the **method** that
needs them, not on the struct declaration, unless the struct itself cannot
exist without them. This is a Rust-style call (compare `impl` blocks gated on
extra bounds).

TODO(open): how method-level bounds compose with dynamic dispatch — a trait
object hides which extra bounds the concrete type satisfies, so methods that
require extra bounds beyond the trait may not be callable through it. Lean:
accept this — it is the price of dynamic dispatch.

## Generic invariance

`List[Cat]` is *not* a `List[Animal]` even though a `Cat` is usable where an
`Animal` is. This is **invariance**, the default for every generic parameter
in Tel — see [`../05-types/09-subtyping-and-variance.md`](../05-types/09-subtyping-and-variance.md)
for the reasoning. The data-modelling consequence: when a generic type's
parameter should *flex* over a set of related concrete types, write the union
explicitly (`List[Cat | Dog]`) or take a generic function rather than relying
on a variance rule.

## Const generics on data types

A type parameter may be a *value* rather than a type — see
[const generics](../05-types/07-generics.md). The canonical data-modelling use
is fixed-length arrays:

```tel
type Matrix[T, const Rows: Int64, const Cols: Int64] = Array[Array[T, Cols], Rows]
```

A more adventurous use the const-generics page records:
mutability as a const generic — `TextBuf[const Mut: Bool]` where one variant
carries a capacity field and the other does not. That idea is open and pulls
in significant complexity; the conservative data-modelling story is to use
const generics only for *size-like* parameters until the broader call is
made.

TODO(open): the same const-generics open question recorded in
[`../05-types/07-generics.md`](../05-types/07-generics.md). When the call is
made, propagate the decision back here.

## Generic methods on generic types

A method on a generic type may itself be generic:

```tel
impl[T] List[T] {
    fn map[U](self, f: (T) -> U) -> List[U] { ... }
}
```

`map` does not constrain `T`; it does require a function `(T) -> U` from the
caller. This is the everyday shape — most "shape changes the element type"
operations look this way.

A subtler case: methods that should return a *more specific*
generic when the input is more specific. A `NonEmpty[List[T]].first()` should
return `T`, not `Option[T]`. This is the "specialise the return type by
constraint" wish that ties back to refined types (see
[`../05-types/12-refined-types.md`](../05-types/12-refined-types.md)). Union
widening expresses it — `T` flowing into `Option[T]` =
`(Some[T] | None)` is allowed, so a more specific impl can legitimately
return `T` where a trait method says `Option[T]`.

TODO(open): whether Tel supports *trait method refinement* — an `impl`
returning a narrower type than the trait declares, the caller seeing the
narrow type when they hold the concrete type. Lean: yes, this falls out of
union subtyping. Confirm and document.

## Generic functions creating generic instances

A generic function may *make* values of its type parameter, when the bound
provides a constructor:

```tel
trait Default { fn default() -> Self }
fn make[T: Default]() -> T { T.default() }
```

Java cannot do this (`Class[T]::new` is awkward),
because Java's interfaces lack `Self`-typed methods. Tel's traits include
`Self` and associated functions, so `make[C]()` is straightforward — `C` must
satisfy the bound, and the bound supplies the constructor. This is how
Tel-side code constructs values inside generic helpers without a `factory`
parameter.

## Recursive generic data types

Generic types appear constantly inside *recursive* data structures — trees,
JSON, expression ASTs. The mechanics are covered in
[`05-recursive-types.md`](05-recursive-types.md); the relevant note here is
that the wrapper-struct discipline for generic unions stays in force when
recursion is involved. A recursive `Tree[T] = (Leaf[T] | Branch[Tree[T]])` works
exactly because `Leaf[T]` and `Branch[...]` are wrapper-tagged.

## Self-referential and complex bounds — kept simple

Several theoretically-attractive directions Tel deliberately
does **not** pursue on data types:

- **Higher-kinded type parameters** — `M[_]` taking a one-argument type
  constructor as a parameter (Haskell's `Functor` style). Useful but slow to
  compile, slow to learn, and a frequent source of inscrutable errors. Tel
  commits to *no* higher-kinded types.
- **Specialisation** — two `impl` blocks for the same trait, one more specific
  than the other, the compiler picking the most specific. Tempting for
  performance but adds significant complexity and frozen-language risk.
  Rejected.
- **Variadic generics** — a type parameter that stands for *any number* of
  types (Diesel-style tuples-of-columns). Rejected; if you need a heterogeneous
  variadic, use a list of a union.

Each rejection is a deliberate *less power for more stability* call per the
[priorities](../02-philosophy/01-priorities.md).

## See also

- [Generics](../05-types/07-generics.md) — the machinery.
- [Union Types](02-union-types.md) — the collapse problem and wrapper structs.
- [Recursive Types](05-recursive-types.md) — generic data types that contain
  themselves.
- [Traits or Interfaces](03-traits-or-interfaces.md) — bounds.
- [Records](01-records.md) — the non-generic base.
- [Entity Identity, Queries, and Projections](../19-use-cases/09-entity-identity-and-projections.md)
  — `Id[T]` as the canonical phantom-parameterised wrapper.

TODO: review
