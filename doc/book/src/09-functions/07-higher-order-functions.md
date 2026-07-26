# Higher-Order Functions

<!-- TODO: review -->

A *higher-order function* takes a function as an argument, returns one, or
both. Because functions are ordinary values in Tel
([Function Declaration](01-function-declaration.md)), this needs no special
machinery — it falls out of functions being values plus light
[lambda syntax](06-closures-and-lambdas.md).

## What

A parameter can have a function type, so a function can be passed behaviour:

```tel
fn apply_twice(f: Fn(Int64) -> Int64, x: Int64) -> Int64 {
    f(f(x))
}
apply_twice(fn(n) { n + 3 }, 10)     # 16
```

A function can also return a function:

```tel
fn make_scaler(factor: Int64) -> Fn(Int64) -> Int64 {
    fn(x: Int64) -> Int64 { x * factor }
}
```

Function types are written `Fn(ArgTypes) -> ReturnType`; see
[Function Types](../05-types/05-function-types.md) for the type-side detail.

The common collection operations are higher-order functions, typically called
with a [trailing lambda](06-closures-and-lambdas.md):

```tel
let totals = orders.map \ total
let big    = orders.keep \ total > EuroAmt(100)
let grand  = totals.fold(EuroAmt(0), |acc, t| acc + t)
```

TODO(open): two naming choices for stdlib higher-order
functions are open — `keep` rather than `filter` ("`filter` does not exist but is a
common guess"), and Java's `Collectors`-style assembly is questioned. Naming
belongs to [`17-standard-library`](../17-standard-library/); `keep` is used in
examples here per that lean.

## Passing a named function

A named function is a value, so it can be passed directly — not only as a
lambda:

```tel
fn is_positive(x: Int64) -> Bool { x > 0 }
numbers.keep(is_positive)
```

Referring to a function *without calling it* needs explicit syntax, because Tel
calls zero-argument functions without `()` — so a bare name in argument position
could be read as "call it." Tel uses a **method-reference** form (tentative
spelling `::`):

```tel
let f = order::total        # bound: a closure that calls order.total()
let g = Order::total        # unbound: takes the receiver as its first argument
numbers.keep(is_positive)   # a plain named function is already a value
```

**Committed semantics (syntax `::` tentative).** A method reference is defined to
be *exactly* the equivalent lambda — it is pure sugar, with no special capture
rule of its own:

- `x::f`     ≡ `|a, b, …| x.f(a, b, …)` — `x` is captured by the ordinary
  closure rules, nothing implicit or strong-by-default.
- `Type::f`  ≡ `|self, a, b, …| self.f(a, b, …)` — the receiver becomes the
  first parameter (the unapplied / "currying" form).

Because the reference *is* the lambda, the hazards Swift hit with bound methods
disappear: there is no hidden capture (it follows closure rules), and a
`mutating` / `&!self` method simply makes the closure hold a unique borrow of
`x` — the same affine constraint the explicit lambda faces. Tel's
[no-overloading rule](09-overloading-and-dispatch.md) removes the other Swift
problem (which overload does the bare reference pick?) outright. See
[Function Application](../07-expressions/06-function-application.md#referring-to-a-function-without-calling-it).

TODO(open): final spelling — `::` is tentative (a postfix `.` -form or a keyword
were alternatives). The *semantics* above are committed; only the surface is open.

`TODO(open): operator-as-function sections.` Haskell's
`(+1)` and `(+)` are a terse way to pass an operator as a function
(`map (+1) xs`, `foldl (+) 0`). This is short, but conflicts with both
*familiarity* (most mainstream languages do not have it) and the
[no-precedence rule](../04-syntax/04-precedence-and-associativity.md). Lean:
not in 1.0; a lambda `\ self + 1` or `|a, b| a + b` is two extra
characters and far more familiar.

## Why

- **No special mechanism.** Functions are values; passing one is passing a
  value. Fewer concepts.
- **Higher-order functions plus closures replace a lot of boilerplate** — a
  single-method "callback object" is unnecessary when a closure is a one-liner.
- They are the foundation of Tel's iteration and DSL story; combined with
  [trailing lambdas](06-closures-and-lambdas.md) they let library code read like
  language syntax.

## A note on dispatch

Passing a function as a value is *not* dynamic dispatch. The function called is
exactly the value passed. Tel's polymorphism over *types* is traits, not
function values, and Tel has no class inheritance or virtual methods — see
[Overloading and Dispatch](09-overloading-and-dispatch.md).

## See also

- [Closures and Lambdas](06-closures-and-lambdas.md)
- [Function Types](../05-types/05-function-types.md)
- [Method Syntax](08-method-syntax.md)
