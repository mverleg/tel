# Traits or Interfaces

A **trait** (working name; *interface* is the close synonym) describes
*behaviour* — a set of methods and associated items a type may implement.
Traits are how Tel does polymorphism. They are not a kind of type values can
have; they are **bounds** that constrain generic parameters and abstract over
sets of types that share a shape.

## What — a description of behaviour

A trait declares method signatures (and optionally associated types or
constants). A type **implements** the trait by supplying those methods. The
implementation is written explicitly — Tel does not adopt Go-style structural
trait satisfaction.

```tel
trait Drawable {
    fn draw(self, ctx: DrawCtx)
}

struct Circle { center: Point, radius: Real64 }

impl Drawable for Circle {
    fn draw(self, ctx: DrawCtx) { ctx.circle(self.center, self.radius) }
}
```

Methods take an explicit `self` (final spelling open). A trait may also declare
**associated items**:

- *Associated types* — `trait Iter { type Item; fn next(...) -> Option[Self.Item] }`.
- *Associated constants and functions* — a `new() -> Self` constructor, a
  *neutral element* for an `Add`-style operation. The
  neutral element is a real-world case (the `0` for `add`, the `1` for `mul`)
  that lets generic code over an `Add[T]` trait pick a meaningful starting
  value without the caller passing one. See
  [`08-ordering.md`](08-ordering.md) for related associated items on `Ord`.

## Why — traits, not classes, not inheritance

Tel has **no class inheritance** (see
[antifeatures](../02-philosophy/04-antifeatures.md)). Polymorphism is
trait-based, period. The argument runs from several directions:

- **Inheritance leaks implementation.** A subclass depends on a superclass's
  internals — every superclass change is a possible subclass break. Traits
  describe a *contract* without leaking implementation.
- **Equality and substitutability don't compose.** There is a classical
  result: you cannot add a value-carrying subclass while preserving
  the equals contract without giving up object-oriented substitutability. Use
  composition (a field whose type is a trait bound) instead.
- **One mechanism is enough.** Replacing inheritance, mixins, and abstract
  classes with a single *trait* concept serves
  [one good way over many clever ones](../02-philosophy/01-priorities.md).

A trait is also the answer to "how does an untagged union expose shared
behaviour": every member implementing a trait makes that trait's methods
available on the union — see
[`02-union-types.md`](02-union-types.md).

## Traits are bounds, not value types

This is the load-bearing design call, and it propagates through the rest of
the type system.

A trait is **not** a type a value can have. You cannot declare
`let x: Drawable = ...`. A trait shows up as:

- A **bound on a generic parameter** — `fn paint[T: Drawable](item: T, ...)`.
- A **bound on a struct field's type parameter** — `struct Scene[T: Drawable]`.
- A **bare trait name in a value position** — `fn render(d: Drawable)` — read
  as "some type that implements `Drawable`," i.e. an anonymous generic.

Tel has **no `impl Trait` spelling**: a bare trait name in a value position
*is* the way to say "some type that implements `Thing`". (Rust writes this
`impl Thing`; Tel leaves it implicit — only the dynamic form, `dyn Thing`, is
ever spelled out. See the
[design history](../20-appendix/05-design-history-and-changelog.md).) This stays
simple at function boundaries; it stops working for *struct fields*, because a
struct that stores such a value would need a generic parameter to carry the
concrete type. There, the user writes the generic explicitly.

```tel
fn render(d: Drawable) { d.draw(default_ctx) }     # == render[T: Drawable](d: T)

struct Scene[T: Drawable] { items: List[T] }       # generic must be explicit
```

This avoids two pitfalls of trait-as-value-type:

1. **Ambiguity in untagged unions.** If traits could be members of a union, one
   value might satisfy two trait members and `match` would have no unambiguous
   arm. Members are concrete types; traits constrain them (see
   [`02-union-types.md`](02-union-types.md)).
2. **The static-vs-dynamic confusion.** Mixing dispatch strategies behind the
   same syntax (`Box[dyn T]` also implements `T`) is convenient but obscures
   whether a call is static or virtual dispatch. Tel keeps traits as bounds,
   leaves static dispatch implicit (a bare trait name), and spells out only the
   dynamic-dispatch story separately (below).

The [philosophy chapter](../02-philosophy/04-antifeatures.md) settles this:
traits are bounds, never union members or value types. A trait object
(`dyn Trait`) is a concrete type and *can* be a union member — see
[union types](02-union-types.md#members-are-concrete-types).

## Open vs closed polymorphism, and where each fits

The open/closed line is drawn carefully. Tel offers both, and they are
distinct mechanisms:

- **Open (trait dispatch).** Any number of types implement a trait; new
  implementations can be added by code that did not know about the others.
  Static dispatch via generics; dynamic dispatch via a trait-object form (see
  below). The trait declares *behaviour*, the implementers are *anyone*.
- **Closed (untagged unions).** A fixed, fully known set of member types.
  Exhaustive `match`. No "anyone can extend"; instead, the *set* is the
  abstraction. Members can be types declared elsewhere (a union over types
  from other modules), and shared behaviour comes from a trait the members all
  implement (auto-exposed on the union).

The distinction maps to the question "is the set of cases known to me, or do I
publish a contract for others to fulfil?". Both have a place; conflating them
is what makes some languages awkward (Rust's `enum` vs `dyn Trait` distinction
is essentially the same split, just spelled differently).

## Static dispatch is the default

A trait bound on a generic parameter normally **monomorphises**: the compiler
generates a specialised version of the function for each concrete type used.
Static dispatch, no virtual call, no boxing. This is the default: generics are
encouraged precisely because they stay fast.

## Dynamic dispatch when it's worth the indirection

When monomorphisation is not what the script wants — when the *set* of
implementing types is unknown at the call site, or stored heterogeneously in a
collection — Tel exposes a **trait-object form**, spelled `dyn Trait`. A few
things are settled about it:

- The keyword is a **prefix**, `dyn Trait`, never a suffix. This keeps it
  stacking cleanly with the prefix borrow sigils — `&dyn Drawable`,
  `&!dyn Drawable` read left-to-right with no parentheses — and keeps
  multi-trait objects (`dyn Drawable + Serialize`) unambiguous.
- It is a *separate* spelling, so reading code makes static vs dynamic
  dispatch obvious. Static is the default (a bare trait name); dynamic is
  opt-in.
- It typically lives behind a pointer/box (the type is not statically sized).
  This is an implementation detail, not user-visible, except that storing many
  of them in a `List` works.
- Only methods that are **dispatchable** — roughly Rust's *object safety* —
  can be called through it. Methods that take `Self` by value, return `Self`,
  or have non-self generic parameters cannot, because the concrete type is
  unknown at the call site.

### Constructing a trait object

A trait object is produced through an **explicit construction step**, not by
implicit coercion. Writing `Circle { ... }` yields a `Circle`; obtaining the
`dyn Drawable` view of it is a separate, visible operation. Three consequences
follow:

- **The static/dynamic boundary is legible.** A reader sees exactly where a
  concrete value becomes a type-erased one — there is no silent widening from
  `Circle` to `dyn Drawable` the way `Box[dyn T]` coercion hides it elsewhere.
- **The set of constructible trait objects is known ahead of time.** Each
  `dyn Trait` (and each multi-trait combination) is named where it is created,
  so the compiler has a *closed list* of which trait-object types exist and
  generates vtables only for those. There is no open-ended vtable space — a
  combination that is never constructed costs nothing. This suits AOT
  compilation and embedding.
- **Object-safety is enforced at construction.** Asking for `dyn Trait` is what
  requires `Trait` to be dispatchable; a trait that is never made into a trait
  object is never constrained by object-safety at all.

There is **no downcasting** from an open trait object back to its concrete type:
no `as`-style runtime test, no reflection (see
[antifeatures](../02-philosophy/04-antifeatures.md)). Once a value is viewed as
`dyn Drawable`, the only thing callable is the trait's dispatchable methods;
recovering the concrete type would need runtime type information Tel does not
keep. When you need to recover the cases, model them as an untagged
[union](02-union-types.md) (a closed set, matched exhaustively) rather than as a
trait object (an open set).

TODO(open): Tel's word for "object safety" and whether the constraint is
checked per *method* (C++-style virtual marker) or per *whole trait*
(Rust-style). Lean: per whole trait — splitting a trait into "dyn-callable"
and "non-dyn-callable" methods is more granular than scripts actually need,
and a clean line lets traits be either fully open or fully closed at a glance.

### Bare trait vs `dyn Trait`, and "can I force a concrete type?"

The concrete-vs-abstract distinction is carried by exactly one keyword, `dyn`;
static dispatch is the unmarked default. There is no extra sigil and no `impl`
keyword (both an earlier `#Fruit`-vs-`Fruit` proposal and an `impl Fruit`
spelling were dropped — see the
[design history](../20-appendix/05-design-history-and-changelog.md)):

- **`Fruit`** (bare) — "some type that implements `Fruit`." Static: the compiler
  monomorphises a fresh copy of the function per concrete caller type. Fast, no
  indirection. This is the default way a trait appears at a value position.
- **`dyn Fruit`** — an explicit, type-erased trait object. One copy of the
  code, a virtual call, constructed by a visible step.

A `dyn Fruit` value (or a `dyn Citrus`, since `Citrus: Fruit`) *also* satisfies
a bare `Fruit` parameter — a trait object implements the trait, after all. So
`Fruit` accepts both a bare `Apple` and a `dyn Fruit`, and there is **no
syntax to force a parameter to be a concrete, non-erased type**. That is
deliberate: the only thing concreteness buys is monomorphisation
(performance), and that is the caller's choice — pass a concrete value and it
monomorphises; pass a `dyn` and it dispatches dynamically. The signature
constrains *behaviour* (`Fruit`), not *representation*. A function that truly
needs the concrete type for a reason other than speed (returning `Self`, etc.)
already expresses that with an explicit generic `[T: Fruit]`.

Because a bare trait name in a value position is meaningful, `x: Fruit` reads
the same whether `Fruit` is a `struct` or a `trait` — a concrete value in the
first case, an anonymous generic over implementers in the second. This
ambiguity is **accepted deliberately**: at a use site the only thing that
matters is "it is a `Fruit`," and whether that monomorphises is a performance
detail the caller controls — the same reasoning that rejected `#Fruit`. When
the distinction matters to a reader, tooling distinguishes trait names from
concrete types; when it matters to the *code* (naming or relating the type,
returning `Self`), an explicit generic `[T: Fruit]` says so.

## Trait lists and bound aliases

Wherever Tel names a **set of traits**, it uses one grammar: traits joined with
`+`. A trait bound on a generic may require several at once:

```tel
fn dedup[T: Eq + Hash](items: List[T]) -> List[T] { ... }
```

The same `A + B + C` trait-list appears in every place a group of traits is
named — deliberately one spelling, not several:

- **Generic / parameter bounds** — `[T: Eq + Hash]`, what a type argument must
  satisfy.
- **Newtype inheritance** — `newtype Decimal : Eq + Ord + Add` lists which of the
  wrapped type's traits carry over (see
  [refined types](../05-types/12-refined-types.md#trait-inheritance-for-newtypes)).
- **Derivation** — `derive Eq + Hash` asks the compiler to synthesise those impls
  ([below](#auto-traits-derived-traits-and-the-derive-story)).

### Bound aliases

A trait list can be **named** — with the ordinary `type` keyword, the same one
that names a union ([type alias](../05-types/01-type-system-overview.md#nominal-structural-or-a-bound)).
The kind follows from the RHS with no ambiguity: `|` joins *types* (a union), `+`
joins *traits* (a bound).

```tel
type Shape   = (Circle | Square)              # a type   — `|` joins types
type Ordinal = Eq + Ord + Hash                # a bound  — `+` joins traits
type Numeric = Ordinal + Add + Sub + Neg + Mul
```

A *bound alias* like `Ordinal` is **not** a new trait — nothing `impl`s it. It is
a transparent synonym: `T: Ordinal` means exactly `T: Eq + Ord + Hash`, and any
type implementing those three already *is* an `Ordinal`, with no extra
declaration (structural over the set, just as a union is over its members). The
one rule a reader keeps in mind: a `type` alias whose RHS is a *trait list* is a
**bound** — usable where a bound is expected (`T: Ordinal`, or as a bare-trait
parameter `fn f(x: Ordinal)`, the implicit-generic default), not as a stored
value type without `dyn`. Because
it is the same trait-list grammar, a bound alias works in every position above —
as a generic bound, as a newtype inheritance list, and inside a `derive`:

```tel
fn sorted[T: Ordinal](xs: List[T]) -> List[T] { ... }
type UserId = newtype Int64 : Ordinal
```

This is what lets `Ordinal` name "sorting and equality but not subtraction" once
and reuse it everywhere a capability tier is wanted.

There is also the question of **negative bounds** — `T: A + !B` — useful for
specialisation ("this generic, but only when `B` is *not* implemented").
Negative bounds compose badly with dynamic dispatch (a trait object hides
which other traits the concrete type does not implement) and with library
evolution (adding a trait impl can break callers downstream).

TODO(open): commit to or reject negative trait bounds. Lean: reject for Tel1
— they are a power feature that conflicts with frozen-language stability. Note
they are *not* needed for specialisation: Tel supports same-owner specialisation
via most-specific-wins resolution (see
[coherence](#coherence-the-orphan-rule-and-specialisation) below and
[`../05-types/07-generics.md`](../05-types/07-generics.md)), not via `!B` negative
bounds.

## Auto-traits, derived traits, and the `derive` story

Some properties of a type are pervasive enough that requiring an `impl` for
each one is friction:

- **Marker traits with structural rules** — e.g. "is safe to share between
  threads", "has no `Drop` cleanup", "can be moved". These are
  *auto-traits* the compiler infers from a type's fields, with an explicit
  opt-out when the inference is wrong.
- **`derive`-generated traits** — equality, hashing, formatting, ordering,
  serialisation. The author writes `derive Eq + Hash` (the same
  [trait-list grammar](#trait-lists-and-bound-aliases) used for bounds) and the
  compiler synthesises a sensible implementation. This is
  the *one* metaprogramming concession Tel commits to (see
  [`../02-philosophy/03-features.md`](../02-philosophy/03-features.md)).

TODO(open): the exact list of auto-traits Tel exposes, and the exact set of
`derive`-able traits. Strong candidates: `Eq`, `Hash`, `Ord`,
`Show`/`Debug`, `Clone`. Pre-pivot check: thread-safety auto-traits assume a
concurrency model that is not yet settled — see
[`../02-philosophy/04-antifeatures.md`](../02-philosophy/04-antifeatures.md).

TODO(open): how trait implementations interact with a hypothetical
borrow/reference story — whether `impl Eq for Cat` and `impl Eq for &Cat` are
the same thing, separate things, or the latter is automatic — a real Rust pain
point. Tel does not yet have a reference model; flag and defer.

## A common motivation: mocks in tests

One usage pattern drives a lot of trait declarations in
the wild: an interface exists *only* so a test can swap in a mock. By one
estimate — about 40% of application-code interfaces — this is large
enough to want a cleaner story.

Tel's answer leans on **capabilities** for the I/O side (a script receives a
`Clock`, an `Rng`, an `Http` — easy to fake in tests because they are values
the test supplies) and on traits for cross-cutting domain logic. There is
also a floated "implies-a-sealed-interface, only test impls allowed"
construct; that is interesting but very narrow.

TODO(open): whether Tel offers a dedicated mock-only-interface construct, or
treats the mocks-via-capabilities + plain-traits combo as enough. Lean: enough.
Capabilities cover most of the test-injection cases.

## Overload ambiguity across repositories

A specific real-world hazard: in Java, a class `A implements B`
existed; another repo defined two overloads `from(B)` and `from(C)`; later
someone added `interface C` and made `A implements B, C`. Now every call
`from(new A())` is ambiguous — and the compile failure surfaces in the
*caller's* repo, not in the change that introduced it.

Tel's stance on this class of bug:

- **Generic functions over trait bounds, not overloading by trait.** A
  generic function `fn from[T: B](t: T)` is one function, not an overload
  set. Adding a new trait `C` and another generic `fn from[T: C](t: T)`
  is a *different* function unless their bounds overlap on a concrete
  type — and Tel's no-overloads policy (see also
  [`../05-types/07-generics.md`](../05-types/07-generics.md)) means the
  author has to name them differently.
- **The remaining ambiguity is contained to the declaration site.** If
  Tel ever does allow overloading on bound traits, the check fires
  where the overloads are declared, not at every distant call site
  whose argument type changed.

TODO(open): confirm Tel does not allow function overloading on trait
bounds, and document the deliberate restriction in
[`../09-functions/`](../09-functions/). Lean: no overloading on trait
bounds at all — different operations get different names. This is
mentioned but not yet pinned down across the docs.

## Coherence: the orphan rule and specialisation

Tel guarantees **one resolved implementation per (trait, applied-type) across a
whole program** — by construction, with no runtime conformance registry. The
rules (full rationale in
[TIP-0005](../tips/0005-trait-coherence-and-the-orphan-rule.md)):

- **Orphan rule.** A concrete `impl T for D` may live only in the crate that
  owns `T` *or* the crate that owns `D`. To extend a foreign trait on a foreign
  type, wrap it in a [newtype](../05-types/12-refined-types.md).
- **Covering rule (generic impls).** A generic `impl[…] T for Head<…>` is allowed
  where the crate owns `T`, or a type it owns appears in the head before the
  first uncovered type parameter. A *blanket* impl over an unconstrained parameter
  (`impl[X] T for X`) is allowed **only in the trait's own crate**.
- **Specialisation, same-owner only.** A general impl and a more specific one may
  overlap, resolved most-specific-wins, **provided they are co-located in one
  crate and declared adjacent** (grouped, not scattered). "More specific" =
  concrete beats generic, and a strict-superset bound set beats a weaker one
  (`T: Eq + Hash` > `T: Eq`); incomparable bound sets are a declaration-site error
  where they overlap. Cross-crate specialisation is rejected — it would make
  resolution depend on imports.
- **`Eq`/`Hash`/`Ord` need no special rule.** They follow the orphan rule like any
  trait; one resolved impl per type already makes them "constant per type" for the
  `Set`/`Map`/sorted collections that store them. What matters is each impl's
  *internal* Eq–Hash consistency, kept by the `identity` key-set / `derive` (see
  [Equality and Hashing](07-equality-and-hashing.md)).
- **Two unrelated behaviours ⇒ a value.** Genuinely different orderings of one
  type are not a second `Ord` impl but an explicit comparator value passed to the
  operation — see [Ordering](08-ordering.md).

## See also

- [Union Types](02-union-types.md) — closed polymorphism, and how traits
  expose shared behaviour on a union.
- [Generics](../05-types/07-generics.md) — bounds in context.
- [Subtyping and Variance](../05-types/09-subtyping-and-variance.md).
- [Equality and Hashing](07-equality-and-hashing.md) — the canonical
  derive-driven traits.
- [Antifeatures — no inheritance](../02-philosophy/04-antifeatures.md).

TODO: review
