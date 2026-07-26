# Method Syntax

<!-- TODO: review -->

Tel lets a function call be written in *method position* — `x.f(y)` — even
though `f` is an ordinary free function. This is **pure syntax**: `x.f(y)`
means `f(x, y)`. There is **no dynamic dispatch**.

## What

Any function whose first parameter has the type of `x` can be called on `x`
with a dot:

```tel
fn days_since(a_clock: Clock, a_date: Date) -> Int64 { ... }

a_clock.days_since(an_order.placed_at)   # exactly days_since(a_clock, placed_at)
```

This is *uniform function call syntax*: the dot form and the prefix form are
the same call. It enables fluent chains without every type having to "own" the
methods:

```tel
( orders
    .keep(\ total > EuroAmt(0))
    .map(\ id)
    .sorted() )
```

Each step is a free function taking the previous result as its first argument.

### The `:` form

There is also a proposed `x:f(a)` for `f(x, a)`, alongside `x.f(a, it)` for
`f(a, x)` — variants that place the receiver in a different argument slot, so
chaining still reads left-to-right when the "subject" is not the first
parameter. The most-recent notes additionally float `:` as a "chain through a
void return" marker — `builder():add(1):add(2):build()` — so a builder whose
intermediate `add` calls return `Unit` instead of `Self` can still be chained
without `let`s. This is similar to PowerShell's `|%` / `|?`.

TODO(open): the exact set of method-call forms and what each means is not
settled. The candidate mapping is `x.f(y)` = `f(x, y)`, `x:f(a)` = `f(x, a)`, and
`x.f(a, it)` = `f(a, x)` — but these overlap and the distinction between `.`
and `:` is unclear (the first two look identical). The cleaner story:

- **`.`** — single chaining form: `x.f(y)` = `f(x, y)`. This is the only one
  examples need.
- **A `tap`-style helper** in the standard library covers "chain through a
  void return" without inventing `:` for it (see
  [Pipelines](../07-expressions/10-pipelines.md)).

`04-syntax` must pick a coherent minimal set. Examples here use only
`x.f(y)` = `f(x, y)`.

### `self` is implicit inside methods

`self` can be omitted when used, and is not declared as part of the function
signature. Inside an `impl` block, a method body refers
to fields and other methods of the implementing type without prefixing them
with `self.`:

```tel
impl Order {
    fn discount(rate: Real64) -> EuroAmt {
        # `total` is `self.total`; `apply` is `self.apply(...)`
        apply(rate, total)
    }
}
```

A leading `.` does **not** stand for `self.`: a line-leading `.` is reserved for
[method-chain continuation](../03-lexical-structure/08-whitespace-and-newlines.md),
the far more established meaning. To reach `self` explicitly, write the `self`
keyword (`self.total`).

**Shadowing a field with a local — resolved (strict start).** A local
`let total = ...` (or a parameter `total`) that collides with a field `total`
reached through implicit `self` is a **compile error**, not a silent shadow:
qualify the field as `self.total`, or rename the local. This is the *same*
collision rule receiver blocks use — a method *is* a receiver context (see
[closures](06-closures-and-lambdas.md#lambda-receivers)) — and it is deliberately
stricter than the "local wins" of Kotlin / Swift / C# / Scala, on the
start-strict-relax-later reasoning (relaxing is backwards-compatible; tightening
is not). `TODO(open):` the sharpest case is the constructor idiom
`fn new(total) { self.total = total }`, where a parameter shadowing the field is
idiomatic elsewhere; if the strict rule proves too noisy, the natural relaxation
is "the local wins for a bare name, the field is reached by `self.`".

### Receiver closures (for DSLs)

A *receiver closure* is a block whose `self`/`this` is rebound to a builder, so
its members are reachable by bare name inside the block — the mechanism behind
builder DSLs like `html { head { title("Hi") } }`. The key point for this chapter:
**a receiver closure is the same thing as a method** — a function with a context —
and uses the *same* rule above (bare names resolve through the context; `self`
reaches it explicitly; no leading `.`). It introduces no second
method-resolution rule.

The design is settled in
[TIP-0010](../tips/0010-lambda-receivers-and-builder-dsls.md): explicitness is
set at the *declaration* (the block parameter's *type* carries the receiver —
`Html.fn() : Unit`, see [function types](../05-types/05-function-types.md) — so
the literal stays a bare `{ … }`, and a named `|h| { … }` opts out of the
implicit context); there is **one** active context (**innermost only**, no
outer-receiver scope chain); and a bare name that **collides** with a context
member is a **compile error** demanding qualification, rather than a silent
capture. A nested block's `this` shadows an enclosing method's `self` for the
block's extent; the outer `self` is reached only by an explicit name.

`TODO(open): receiver / context functions.` This single-builder receiver is
settled; what stays open is whether Tel *also* gains *ambient* context threaded
by type (Scala `given` / Kotlin context parameters) beyond it. Kotlin's removal
of anonymous context receivers (see
[TIP-0010](../tips/0010-lambda-receivers-and-builder-dsls.md)) is a caution
against a stack of implicit ones; lean toward **named** context parameters for
that case rather than a second implicit `this`.

## No dynamic dispatch

`x.f(y)` does **not** look up `f` on `x`'s runtime type. The function is
resolved statically from the name `f` and the static types involved. Tel has:

- no class inheritance,
- no virtual method tables driven by an object's runtime class.

Polymorphism over types is provided by **traits** — a trait method *is*
resolved per implementing type, but that is the trait mechanism, not
method-call syntax. See [Overloading and Dispatch](09-overloading-and-dispatch.md)
and [Traits](../10-data-modelling/03-traits-or-interfaces.md).

## Field-vs-getter and zero-argument calls

Method syntax composes with calling zero-argument functions without `()`
([Function Application](../07-expressions/06-function-application.md)):
`text.length` is `length(text)`. That reuse is deliberate — but it also
inherits the unresolved field-vs-getter ambiguity flagged in
[Field and Index Access](../07-expressions/07-field-and-index-access.md).

## Why

- **Fluent chains without owned methods.** A pipeline of transforms reads
  top-to-bottom, yet every step is a plain function — no need to attach methods
  to a type or to define an interface just to get dot-chaining.
- **Extensible after the fact.** A new operation on an existing type is just a
  new free function; the type need not be reopened.
- **No dispatch surprises.** Since `x.f(y)` is statically `f(x, y)`, *what
  looks correct is correct* — there is no hidden override changing which `f`
  runs.

## See also

- [Function Application](../07-expressions/06-function-application.md)
- [Higher-Order Functions](07-higher-order-functions.md)
- [Pipelines](../07-expressions/10-pipelines.md)
- [Overloading and Dispatch](09-overloading-and-dispatch.md)
- [Traits](../10-data-modelling/03-traits-or-interfaces.md)
