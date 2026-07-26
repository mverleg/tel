# Pipelines

<!-- TODO: review -->

A *pipeline* is a left-to-right chain of operations on a single value: each
step takes the previous step's result and produces the next. Tel encourages
pipelines because they keep data-transformation code readable in the
direction it runs — top-down or left-to-right, not deeply nested.

## What

The simplest pipeline is just chained
[method-call syntax](../09-functions/08-method-syntax.md):

```tel
orders
    .keep \ total > EuroAmt(0)
    .map \ score(self)
    .sorted()
    .take(10)
```

Each step calls a free function with the previous result as its first
argument (`x.f(y)` is `f(x, y)`). No type has to "own" the methods; the
pipeline is just a chain of plain functions composed by `.`.

A multi-line chain needs **no** wrapping parentheses: a leading `.` on the next
line continues the chain (see
[Whitespace and Newlines](../03-lexical-structure/08-whitespace-and-newlines.md)).
Each step may use a brace-less lambda directly — `.keep \ …` — because a
brace-less body ends at its own newline, so the following `.step` continues the
chain rather than running on into the closure. A single-step pipeline that fits
one line also needs no wrap: `orders.map \ total`.

## Pipelines on void / chainable returns

Chaining keeps working even when a step returns *nothing
useful* (a `void`-returning method, a builder `.add()` that returns `Unit`
not `Self`). The inspiration is [Rust's `tap`](https://crates.io/crates/tap):
insert a step purely for its effect, then keep flowing the
original value.

The cleanest spelling is a small helper that takes a lambda by reference and
returns its receiver:

```tel
( orders
    .also(\ log.info("kept ${it.length()}"))   # observe, do not change the value
    .map(\ score(it)) )
```

`TODO(open): "tap"-style helpers.` Whether this is a standard-library helper
(`also`, `tap`, `inspect`) or a dedicated chain operator is open. Lean: a
small set of stdlib functions, not a new operator — *one good way*.

## Result / error-flowing pipelines

A pipeline that fails partway is a recurring shape:

```tel
load(path)
    .and_then(parse)
    .and_then(validate)
    .and_then(store)
```

Each `and_then` takes a step that returns a `Result`; the pipeline
short-circuits to the first `Err`. The error-propagation operator (`?`-style
postfix) does the same job inside a function body — see
[Error Propagation](../08-control-flow/07-error-propagation.md). Pipelines
and `?` are two views of the same idea.

There is no dedicated monadic-bind operator. `.and_then(...)` handles most
chains, and where a step needs to branch or bind intermediate values, an inline
lambda using the postfix `?` operator for early exit covers the rest:

```tel
load(path).and_then(|raw| {
    let parsed = parse(raw)?
    let checked = validate(parsed)?
    store(checked)
})
```

Both are library/`?` shapes, not a new language construct — consistent with
*one good way*.

## Pipelines and iterators

Many pipelines operate on a *stream of values*: each step is a transformation
on each element. Tel's iterators are themselves pipeline-friendly — they
expose `map`, `keep`, `take`, `fold` as ordinary higher-order functions:

```tel
let summary = ( orders
    .iter()
    .keep(\ total > EuroAmt(0))
    .map(\ total)
    .sum() )
```

"Vector-style" element-wise arithmetic is a separate, exploratory feature —
see [Arithmetic and Numeric](02-arithmetic-and-numeric.md#element-wise-vector-arithmetic).

## Why

- **Left-to-right reads the way the data flows.** A pipeline of `map`,
  `keep`, `fold` mirrors how the values move; deeply-nested calls
  (`fold(map(keep(orders, ...), ...), ...)`) read backwards.
- **New operations need no access to the type.** To add a step that chains on
  `Order`, you just write a free function taking an `Order` as its first
  argument — you do not modify, subclass, or re-open the type the way adding a
  method requires in many languages. Since `x.f(y)` is exactly `f(x, y)`, that
  new function drops straight into a `.`-chain alongside the built-in ones.
- **One pipeline mechanism.** `.` chaining covers ordinary and iterator
  pipelines alike. Tel deliberately does not ship a separate pipeline
  operator (`|>`-style) on top of method chaining — *one good way*.

## See also

- [Method Syntax](../09-functions/08-method-syntax.md)
- [Higher-Order Functions](../09-functions/07-higher-order-functions.md)
- [Closures and Lambdas](../09-functions/06-closures-and-lambdas.md)
- [Function Application](06-function-application.md) — trailing closures.
- [Error Propagation](../08-control-flow/07-error-propagation.md)
- [Fallback Operator](11-fallback-operator.md)
- [Whitespace and Newlines](../03-lexical-structure/08-whitespace-and-newlines.md) —
  newline termination and leading-`.` chain continuation.
