# Generics

Generics let a type or function be written once and used with many element
types — `List[T]`, `Option[T]`, `Result[T, E]`. Apivolve, Tel's original home,
needs them to model schemas, so generics are a settled feature; the open
questions are about *how much* generic power Tel exposes.

## What — type parameters

A type or function may take **type parameters**, written in brackets, that stand
for a type supplied at the use site:

```tel
struct Pair[A, B] {
    first:  A,
    second: B,
}

fn first_of[T](items: List[T]) -> Option[T] { ... }
```

A type parameter may carry a **bound**: a trait the supplied type must
implement. This is the *only* role traits play in the type system — they
constrain generic parameters, they are never the type of a value (see
[`../10-data-modelling/03-traits-or-interfaces.md`](../10-data-modelling/03-traits-or-interfaces.md)).

```tel
fn max[T: Ord](a: T, b: T) -> T { if a >= b { a } else { b } }
```

A bound may require several traits via the `+` trait-list (`[T: Eq + Hash]`), and
that list may be a named [bound alias](../10-data-modelling/03-traits-or-interfaces.md#bound-aliases)
(`[T: Ordinal]`) — the same grammar used for newtype inheritance and `derive`.

## Why — and what is deliberately left out

Generics earn their place by serving *data modelling*, not by being maximally
expressive. Tel takes the conservative subset:

- **Generics on data types are essential** — `List[T]`, `Result[T, E]` and the
  like are how Apivolve schemas are modelled.
- **Generic functions exist** but stay simple. A function over `List[T]` is
  already generic; that covers most needs.
- **No higher-kinded types, no variadic generics.** These are
  the kind of "latest-and-greatest" feature the priorities reject — they
  complicate the type checker, hurt compile speed, and are hard to support
  identically across many host runtimes. (Specialisation *is* supported, in a
  confined form — see [Specialisation](#specialisation) below.)

### Invariance

Generic type parameters are **invariant**: `List[Cat]` and `List[Animal]` are
unrelated types. Note there is no "a `Cat` is an `Animal`" subtyping for variance
to propagate in the first place — Tel has no subtyping between user types, and a
`Cat` implementing a trait `Animal` is a *bound* (`Cat` satisfies `Animal`), not a
subtype relation (see
[`09-subtyping-and-variance.md`](09-subtyping-and-variance.md)). The one place a
value of one type *is* usable where another is named — union membership, e.g. a
`Cat` where `(Cat | Dog)` is wanted — deliberately does not extend to element
types: a `List[Cat]` is still not a `List[(Cat | Dog)]`. Invariance is the simple,
predictable rule and it sidesteps the soundness traps variance creates with
mutability, so it is the natural default rather than a restriction.

### Specialisation

A generic impl may be **specialised** by a more specific one — a general
`impl[T: Show] Show for List[T]` alongside a tuned `impl Show for List[byte]`,
resolved *most-specific-wins*. "More specific" is a simple partial order:
**concrete beats generic** (and two concrete types never overlap, since there is
no inheritance), and a **strict-superset bound set** beats a weaker one
(`impl[T: Eq + Hash]` refines `impl[T: Eq]`). Incomparable bound sets
(`T: Eq + Hash` vs `T: Eq + Display`) do not rank — a type matching both with no
winner is a declaration-site error.

Tel allows specialisation **only within a single crate**: the general impl and
every specialisation must be co-located with one owner — and declared **adjacent**
(grouped, not scattered across modules) — so resolution draws on a fixed,
import-independent candidate set a reader can see in one place, and is never
scope-sensitive. *Cross-crate* specialisation — a foreign crate refining
someone else's generic impl — is rejected, because which impl wins would then
depend on what is imported. This narrows an earlier blanket "no specialisation"
stance: same-owner specialisation is in, cross-crate is out. The orphan and
coherence rules this rests on are in
[`../10-data-modelling/03-traits-or-interfaces.md`](../10-data-modelling/03-traits-or-interfaces.md#coherence-the-orphan-rule-and-specialisation)
and [TIP-0005](../tips/0005-trait-coherence-and-the-orphan-rule.md).

### Monomorphisation terminates: no polymorphic recursion

Generic code compiles by **monomorphisation** (see
[traits](../10-data-modelling/03-traits-or-interfaces.md#static-dispatch-is-the-default)):
each concrete instantiation — `sum[Int64]`, `sum[Text]` — gets its own
compiled copy, and each copy's body demands the instances *it* calls. For that
to be a build step at all, the demanded set must be **finite** — and it is
finite exactly when recursion never *grows* its own type arguments. Tel
guarantees this with one rule:

> Inside a recursive call cycle, a call to a function in the cycle must pass
> the caller's own type parameters **unchanged**, or fully concrete types —
> never a type *built from* a parameter.

Walk the two cases to see why that is the precise line:

- **Ordinary recursion is already finite.** `fn sum[T: Add](xs: List[T])`
  calling `sum[T](rest)` stays at the caller's own `T`. However deep the
  recursion runs at run time, no *new* instantiation is demanded at compile
  time: `sum[Int64]` only ever needs `sum[Int64]`. The instance set is bounded
  by the finite set of concrete types the program actually mentions.
- **Polymorphic recursion mints a new type per level.** The rejected shape is
  a recursive call at a type *constructed from* the parameter:

  ```tel
  # Loose sketch. A nesting that deepens the type with the data:
  type Nested[T] = (Leaf | Wrap[Nested[T]])

  fn depth[T](x: Nested[T]) -> Int64 {
      match x {
          Leaf        => 0,
          Wrap(inner) => 1 + depth[Nested[T]](inner)   # ERROR: Nested[T], not T
      }
  }
  ```

  `depth[Int64]` demands `depth[Nested[Int64]]`, which demands
  `depth[Nested[Nested[Int64]]]`, and so on: the program text is finite, but
  the set of instances it demands is not, because each level of the recursion
  wraps the type argument one constructor deeper.

The check runs at type-checking time, where the cycle is written: the compiler
finds the groups of functions that (transitively) call each other, and within
such a group every call to a group member must pass the caller's parameters
through unchanged or use concrete types. The error points at the offending
call site — never at some later instantiation depth. Calls through
`dyn Trait` are unaffected: dynamic dispatch compiles one type-erased copy, so
nothing is instantiated per level, and it is the escape hatch for the rare
algorithm that genuinely wants a type that deepens with the data.

Why reject outright rather than cap the instantiation depth: a depth limit
would be an arbitrary constant frozen into the language spec forever, its
failures would surface during code generation far from the recursive call that
caused them, and a host that picked a different limit would compile different
programs. Prior art matches: languages that type-erase generics (Haskell) can
allow polymorphic recursion because they never enumerate instances; Rust,
which monomorphises, accepts the source and then fails during code generation
against an internal recursion limit — an error far from its cause, at a limit
that is a compiler constant rather than a language rule. Tel monomorphises, so
it takes the honest version of the restriction: state it in the language,
check it at the declaration.

## Const generics

A type may be parameterised by a **value**, not just a type — a *const generic*.
The motivating case is fixed-size arrays (`Array[T, 8]`) and small SIMD-style
vectors, where the length is part of the type.

One neat idea pushes this further: **mutability as a const generic**.
A single type declaration could have a mutable and an immutable variant selected
by a boolean const parameter — e.g. a string type that, when mutable, carries a
capacity field (`Int64`) and, when immutable, does not (carrying the [unit
type](02-primitive-types.md) instead). The type of a *field* would then depend
on the const generic.

```tel
# Sketch — not settled syntax.
# `Mut` is a const generic; the capacity field's type depends on it.
struct TextBuf[const Mut: Bool] {
    data:     Bytes,
    capacity: if Mut { Int64 } else { Unit },
}
type Text  = TextBuf[false]
# the mutable form !Text is TextBuf[true]
```

A refinement: instead of a bare `Bool`, use a small
data-less enum with an associated const, which reads better and extends past two
states.

TODO(open): commit to or reject const generics, and separately the
mutability-as-const-generic idea. It is elegant but it makes a field's type
*depend on a parameter*, which is a real jump in type-system complexity and
edges toward dependent types — weigh hard against compile speed and the "frozen,
conservative" priority. The mutability model itself is unresolved (see
[antifeatures](../02-philosophy/04-antifeatures.md)); this idea is one candidate
for it.

## Generics, unions, and wrapper structs

Untagged unions interact awkwardly with generics. If `Either[A, B] = (A | B)` is
instantiated as `Either[Int64, Int64]`, the two members collapse — `(Int64 | Int64)` is
just `Int64` — and you cannot tell which side a value came from.

The fix is **not** a language feature but a modelling habit: wrap each side in
its own newtype/record so the wrappers act as the tags.

```tel
struct Ok[T]  { value: T }
struct Err[E] { error: E }
type Result[T, E] = (Ok[T] | Err[E])   // Ok[Int64] and Err[Int64] stay distinct
```

This is the same reasoning that keeps `Option[Option[T]]` from collapsing — see
[`../10-data-modelling/02-union-types.md`](../10-data-modelling/02-union-types.md)
and [`06-option-and-nullability.md`](06-option-and-nullability.md). The cost is
that generic code ends up defining a fair number of small wrapper structs; the
benefit is each variant becomes a separately-addressable type.

## Generic parameters are not accessed as type members

You **cannot** name a type's generic parameter as a member — there is no
`Optional[Int64].Wrapped` or `Dict[K, V].Value`. This is a deliberate decision,
not an oversight, and it avoids a specific ambiguity that Swift regrets shipping
around (Jordan Rose, *Generic Parameters Aren't Members*).

The hazard: a generic parameter can share a name with an **associated type** the
type implements, but resolve to a *different* type. Given a generic `S[Element]`
that implements a trait whose associated type is also `Element` but whose witness
is, say, `Set[Element]`, a bare `S[Int64].Element` could mean either `Int64` (the
parameter) or `Set[Int64]` (the associated type). Tel removes the ambiguity at the
root rather than inventing a resolution rule:

1. **Associated types are reached only trait-qualified**, never as a bare
   `.Name` — you write the trait whose associated type you mean (spelling TBD,
   e.g. `S[Int64]::(Iter.Item)`). A bare `.Name` is therefore *never* an
   associated type.
2. **A generic-parameter name that collides with an implemented associated
   type — resolving to a different type — is a declaration-site error.** The
   compiler fails where `S` is declared (rename the parameter to fix it), never
   as a spooky failure in a distant caller. This mirrors Tel's stance on
   [overload ambiguity](../10-data-modelling/03-traits-or-interfaces.md): fail
   where the conflict is introduced.
3. With (1) and (2) the collision cannot occur, so exposing generic parameters
   as members would now be *safe* — but Tel **doesn't**, by default. If you need
   to name a type argument, name the type directly or introduce a `type` alias.
   Revisit only if a concrete use case appears.

## Generics like ordinary arguments

Const generics and comparator/hasher parameters blur the
line between type parameters and value parameters — a `HashMap` could take a
hashing strategy as a generic argument much as Java passes a `Comparator`.
Functions can themselves be const. This raises the question of whether Tel
should unify generic and ordinary parameters.

TODO(open): whether to unify type parameters, const generics, and ordinary
parameters into one mechanism, or keep them separate. Unifying is conceptually
tidy; against it, generic/const arguments can be *inferred* and ordinary ones
generally are not, and unification is exactly the sort of novel surface the
familiarity priority warns against. Lean: keep them separate, distinct syntax.

## See also

- [Type Inference](08-type-inference.md) — how generic parameters get filled in,
  and why inference is kept limited.
- [Subtyping and Variance](09-subtyping-and-variance.md) — why invariance.
- [Generic Data Types](../10-data-modelling/04-generic-data-types.md).
- [Traits or Interfaces](../10-data-modelling/03-traits-or-interfaces.md) — bounds.

TODO: review
