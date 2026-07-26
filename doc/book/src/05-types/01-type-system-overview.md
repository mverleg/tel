# Type System Overview

Tel is **statically typed**: every value has a type known at compile time, and a
script with a type error does not run. Static typing is non-negotiable — it is
the second-ranked priority after stability (see
[`../02-philosophy/01-priorities.md`](../02-philosophy/01-priorities.md)) and the
main thing that sets Tel apart from the dynamically-typed scripting languages it
competes with.

This page is the map of the type system. It says *what kinds of types exist* and
*how they relate*; the individual topics go into detail.

## What — the kinds of types

Tel's types fall into a few groups:

- **Primitive types** — integers, reals, decimals, booleans, the unit type. See
  [`02-primitive-types.md`](02-primitive-types.md). The **never type** (the
  bottom type) has its own chapter: [`14-never-type.md`](14-never-type.md).
- **Text** — strings and the text-handling story. See
  [`03-strings-and-text.md`](03-strings-and-text.md).
- **Composite types** — tuples, arrays, and collections. See
  [`04-tuples-and-arrays.md`](04-tuples-and-arrays.md) and
  [`../10-data-modelling/09-collection-types.md`](../10-data-modelling/09-collection-types.md).
- **Records (structs)** — named product types: a fixed set of named, typed
  fields. See [`../10-data-modelling/01-records.md`](../10-data-modelling/01-records.md).
- **Union types** — "one of N possibilities", written `(A | B | C)`. Tel's unions
  are *untagged*. See
  [`../10-data-modelling/02-union-types.md`](../10-data-modelling/02-union-types.md).
- **Function types** — see [`05-function-types.md`](05-function-types.md).
- **Refined / newtype types** — wrappers that add a name and value constraints to
  an existing type (`EuroAmt`, `Id[Person]`, `Real64 > 0`). See
  [`12-refined-types.md`](12-refined-types.md).
- **Traits** — *not* the type of any value, but *bounds* on types: a description
  of behaviour a type can implement. See
  [`../10-data-modelling/03-traits-or-interfaces.md`](../10-data-modelling/03-traits-or-interfaces.md).

A useful mental split: **concrete data types** (records, unions, primitives) are
the types values actually *have*; **traits** are constraints used to talk about
*sets* of types in generic code. Union members are concrete types; trait bounds
say what a generic parameter must support. Keeping these apart is what lets
unions stay untagged and unambiguous.

## How types are written

Tel uses one consistent rule for type notation: **a type is written like a value,
with each literal replaced by its type.** The type of a value can then be read
straight off the value's own shape.

```tel
(1, 3, x = 8)        # value   →   (Int64, Int64, x = Int64)   # type
fn(a, b) { a + b }   # value   →   fn(Int64, Real64) : Int64   # type
```

- **Tuples** keep their positional and named layout, literals swapped for types —
  see [tuples and arrays](04-tuples-and-arrays.md).
- **Functions** use the `fn(args) : ret` form; the value's body block is the part
  replaced, by `: <return type>` — see [function types](05-function-types.md).
- **Unions** are the exception. A value inhabits exactly *one* arm, so there is no
  value-shaped layout to mirror; a union type is always written parenthesised,
  `(A | B | C)`, everywhere it is spelled — see
  [union types](../10-data-modelling/02-union-types.md).

This is a rule about *notation*. It is separate from the question of whether some
values are themselves types (data-less unit types such as `:NotFound`), discussed
under [nominal, structural, or a bound](#nominal-structural-or-a-bound) below.

## Why — the shape of the system

Several design rules follow from the priorities and recur across every type
topic:

- **No `null`.** Optionality is a type, `Option[T]` — a value either is a `T` or
  is the explicit absence value. See
  [`06-option-and-nullability.md`](06-option-and-nullability.md).
- **No implicit conversions.** A value of one type never silently becomes
  another — no numeric widening, no truthy/falsy, no string ↔ number. Every
  conversion is written. See
  [`11-conversions-and-coercions.md`](11-conversions-and-coercions.md).
- **Invalid states should be unrepresentable.** The type system is a tool for
  *modelling*, not just for catching slips. Refined types, exhaustive unions, and
  records with invariants let a script make a wrong value impossible to
  construct rather than something checked for later.
- **Anything built-in types can do, user types can do too — with one
  exception.** Wrapping a primitive in a meaningful type (`EuroAmt`) costs
  nothing and loses no *value-level* capability — operators, ordering, hashing
  all carry over, which is what makes refined types cheap enough to use freely.
  The deliberate exception is the concurrency capability `Sync`:
  interior-mutable types shared across tasks (`Mutex`, atomics, a concurrent
  map) are stdlib-only and cannot be defined in user code, because Tel seals
  the aliasable-and-mutable quadrant inside the standard library for data-race
  safety (see
  [Memory Model for Concurrency](../14-concurrency-and-parallelism/07-memory-model-for-concurrency.md)).
- **Stability constrains inference.** Type inference is convenient but it couples
  a script's meaning to inference rules; making those rules cleverer later can
  silently change what existing scripts mean. Tel therefore keeps inference
  *limited and local* — see [`08-type-inference.md`](08-type-inference.md).

## Nominal, structural, or a bound?

Tel's type-shaping constructs fall into **three clearly separated kinds**. The
split is deliberate: it is what lets unions stay untagged and unambiguous while
records keep their declared identity, and it is the backbone of several later
rules.

| Construct | Kind | Type identity |
|---|---|---|
| **Records / structs** (and `newtype`) | **Nominal** | the type you *declared*, by name — two records with identical fields are still *different* types |
| **Unions** `(A \| B)` | **Structural** | defined by their member *set* — two unions with the same members are the *same* type, however written or named |
| **Traits** | **Bound** | *not a type a value can have at all* — a constraint on what a type must support |

**Records are nominal.** `type Meters = { v: Real64 }` and
`type Seconds = { v: Real64 }` are distinct despite identical shape; a value is a
`Meters` only if it was constructed as one. `newtype` is the same nominal rule
over a single wrapped type ([refined types](12-refined-types.md)). This nominal
identity is exactly what turns domain mix-ups (currencies, ids, units) into
compile errors.

**Unions are structural.** A union is just a *set of member types*, so two unions
are equal precisely when their member sets are equal — `type A = (X | Y)` and
`type B = (X | Y)` are the **same, interchangeable** type, and a named union is a
transparent *alias*, not a new type. A single-member union collapses to its
member: `(X)` *is* `X`. Structurally, a union also auto-exposes the interface its
members *share* — any field, method, or trait present on *every* member is
available on the union with no `match`:

```tel
type SchoolPerson = (Teacher | Student)
# Teacher and Student both have `name: Text`, so this works without matching:
fn greet(p: SchoolPerson) -> Text { "Hi, " & p.name }
```

When you want a *distinct* closed family instead of a structural alias, wrap the
union in a `newtype` — that opts into nominal identity on top of the structural
set.

**Traits are bounds, not types.** A trait describes behaviour a type can
implement; it is never the type of a value (`let x: Drawable` is rejected). It
constrains generic parameters and union members, and supplies the named
capability surface a union can be *required* to keep (see
[union types](../10-data-modelling/02-union-types.md)). The one way a trait
reaches value level is an explicit **trait object** `dyn Trait` — a single erased
concrete type with a known representation, which *is* a type (and so may even be a
union member). Static dispatch (the bound) and dynamic dispatch (the trait
object) never hide behind the same syntax (see
[traits](../10-data-modelling/03-traits-or-interfaces.md)).

The three kinds carry the later rules: union subtyping and set algebra fall out
of *structural*; domain-safety newtypes fall out of *nominal*; and "no trait as a
value, no downcasting out of an open trait object" falls out of *bound*.

TODO(open): "all values are also types" is a recurring idea — e.g. `Bool` as
the union `(True | False)`, unit-type literals like `:NotFound` being both a type
and its sole value. Tel adopts this *only* for data-less types (see
[`02-primitive-types.md`](02-primitive-types.md) on the unit type and
[`../10-data-modelling/02-union-types.md`](../10-data-modelling/02-union-types.md)
on named-boolean enums). A general value-as-type / type-level-value machinery
(mapping `'Int64' | 'text'` literals to `IntCol | TextCol`) is powerful but edges
toward dependent types and was not pinned down — re-justify against the
"conservative, frozen language" priority before adopting.

## How it looks

```tel
# A concrete record type.
struct Order {
    id:    Id[Order],
    total: EuroAmt,
    lines: List[OrderLine],
}

# A union of concrete types.
type Outcome = (Accepted | Rejected | NeedsReview)

# A refined primitive: a Real64 that is constrained positive.
type Ratio = Real64 > 0.0

# A trait: a bound, used to constrain generics, never a value's type.
trait Summable {
    fn add(self, other: Self) -> Self
}
```

## See also

- [Priorities and Trade-offs](../02-philosophy/01-priorities.md) — the ranking
  behind every call on this page.
- [Features Tel Embraces](../02-philosophy/03-features.md) and
  [Antifeatures](../02-philosophy/04-antifeatures.md).

TODO: review
