# Conversion Expressions

<!-- TODO: review -->

> **No "cast".** Tel avoids the word *cast*. There is no downcasting — narrowing
> a union to one of its members is a [`match`](#pattern-matching-on-type), not a
> cast — and the "upcast" direction (widening to a supertype) is implicit
> subtyping, not a conversion at all. What remains is always a *conversion*: a
> named, explicit transformation between two distinct types.

Tel has **no implicit conversions**. If you want an `Int64` to become a `Float`,
a `User_v3` to become a `User_v4`, or a `Text` to be parsed as a number, you
*say so* at the point it happens. The expression-level surface for that is
small and uniform.

## What

The exact syntax is not pinned down, but a few behaviours are clear and
committed. The placeholder used here is a `.to[Type]()` / `Type.from(value)`
pair:

```tel
let n: Int64   = 42
let r: Real64  = n.to[Real64]()              # widening, but still explicit
let s: Text  = n.to[Text]()              # formatted as Decimal

let parsed: Result[Int64, ParseErr] = Int64.from("42")   # may fail — Result
```

Three categories the design is explicit about:

- **Infallible total conversions** (e.g. `Int64 -> Real64`, `Int64 -> Text`) return
  the converted value directly.
- **Fallible conversions** (e.g. `Text -> Int64`, `Real64 -> Int64`) return a
  [`Result`](../08-control-flow/07-error-propagation.md) — Tel does not throw,
  ever.
- **Newtype wrap / unwrap** (e.g. `Int64 <-> EuroAmt`) uses the wrapper type's
  constructor and an `.inner()` (or named accessor), not a generic conversion.

`TODO(open): the conversion spelling.` Candidates:

- `value as Type` — short, familiar from Rust/Kotlin; ambiguous when the
  target type is generic or when the value type already has a `.to`.
- `value.to[Type]()` — uniform with method syntax; pairs with `Type.from(value)`.
- A trait-driven `.into()` that picks the conversion from context — flexible
  but harder to read.

Lean: the explicit `.to[Type]()` / `Type.from(value)` pair, because it never
hides which conversion is happening. `as` may exist as sugar for the
unambiguous infallible case.

## No implicit widening

Even "safe" widenings are written:

```tel
let n: Int64  = 3
let r: Real64 = n            # REJECTED — no implicit widening
let r: Real64 = n.to[Real64]() # ACCEPTED
```

This is firm — it is in the [antifeatures](../02-philosophy/04-antifeatures.md)
list. "No implicit numeric widening" pays for the
rest of the safety story: if `n + 1.0` quietly worked, the type system would
not be telling you the truth about what your code does.

## Fallible conversions return `Result`

Anything that can fail returns a `Result`:

```tel
match Int64.from(user_input) {
    Ok(n)  => use(n)
    Err(e) => report(e)
}
```

There is no `Int64.from` that aborts on bad input as the default — silent or
panicking conversion is one of Tel's antifeatures
([antifeatures](../02-philosophy/04-antifeatures.md)). A panicking
*assert-this-converts* helper (`Int64.from(s).expect("digits only")`) exists,
but it is opt-in and visibly aggressive.

## Newtype wrap and unwrap

Tel actively encourages refined / newtype wrappers
(see [Features](../02-philosophy/03-features.md)). One candidate is a
`create Fraction like Real64 where 0 <= it <= 1` declaration form:

```tel
# illustrative, not pinned syntax
create Fraction like Real64 where (0.0 <= it) and (it <= 1.0)
```

…which automatically gives the wrapper the underlying type's operations *and*
serialisation as the underlying value. Conversions then go through the
wrapper:

- **Into wrapper:** `Fraction.from(0.5)` — fallible if the `where` clause
  could be violated, infallible if a constructor proves it (e.g. the value is
  itself a `Probability`).
- **Out of wrapper:** `frac.to[Real64]()` or a named `.inner()` accessor.

`TODO(open): newtype declaration syntax and the auto-conversion rule.` The
`create … like … where …` form, and which operations are inherited (does
`Fraction + Fraction` produce a `Fraction` or a `Real64`?), are
data-modelling decisions. Defer the spelling to
[Records / Newtypes](../10-data-modelling/01-records.md); the *expression*
side just uses whatever those decisions name.

## Trait-driven conversion (`Into`-style)

`TODO(open): how-much-magic for `into`-style conversions.` A trait-driven
conversion keeps coming back as an option (a `From[A] for B` that
auto-implements `Into[B] for A`), so library code can write:

```tel
fn paint(c: Colour) { ... }
paint(some_rgb.into[Colour]())     # explicit, but trait-driven
```

This is powerful for collection conversions and for crossing API boundaries.
The risk is that *implicit* `.into()` (chosen by context) starts looking like
the implicit coercion Tel just forbade. Lean: trait-driven conversions
exist, but the conversion call is always *visible* at the source — never
inserted silently by the compiler.

## Pattern matching on type

Narrowing a union to one of its members is a `match` on the member *type* — Tel
has no separate narrowing form:

```tel
match value {
    img: Image => render(img)
    t: Text    => print(t)
}
```

There is no `if value is Image` form to recover the member — the pattern binds
and refines the type in one step. See
[Match Expressions](../08-control-flow/02-match-expressions.md).

**How does this work with no tag?** Tel's unions are *untagged*: there is no
stored discriminant. The match resolves because **the member's own type is its
tag** — a value of `(Image | Text)` *is* an `Image` or *is* a `Text`, and the
runtime tells them apart by their representation. This is the model in
[Union Types § The variant's type is its tag](../10-data-modelling/02-union-types.md#the-variants-type-is-its-tag).
It is **not** something [TIP-0002](../tips/0002-untagged-unions-and-sealed-traits.md)
covers — that tip only argues the union-vs-sealed-trait distinction, not the
runtime mechanism. The consequence of the model: members must be
representationally distinguishable, which is why `(Float | Float)` collapses to
`Float` and why two members that share a representation cannot be told apart.
`TODO(open): pin down how the runtime carries member identity for host-mapped
and primitive types where there is no spare discriminant.`

`TODO(open): instanceof-like syntax.` Whether a Java-style
flow-sensitive `if x is Image then ...` should be expressible as library
code. Probably not in 1.0 — `match` covers it and adds exhaustiveness.

## Why

- **Explicit conversion at every step** is what makes "no implicit
  conversions" liveable: the conversion is *one method call*, not a `match`
  block.
- **Result for fallible conversions** keeps error-handling explicit and
  exhaustive — see [error handling](../13-error-handling/).
- **Newtype wrappers** are cheap precisely because their conversion surface
  is small and uniform.

## See also

- [Antifeatures](../02-philosophy/04-antifeatures.md) — no implicit
  conversions.
- [Conversions and Coercions](../05-types/11-conversions-and-coercions.md)
- [Records / Newtypes](../10-data-modelling/01-records.md)
- [Error Propagation](../08-control-flow/07-error-propagation.md)
- [Match Expressions](../08-control-flow/02-match-expressions.md)
