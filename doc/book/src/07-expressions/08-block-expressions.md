# Block Expressions

<!-- TODO: review -->

Tel is expression-oriented: a block `{ ... }` is itself an expression that
produces a value. This keeps small scripts small and is what lets `if`,
`match`, and loop arms yield values.

> **Decision (reversal).** A block's value is its **last expression** — the
> Rust-style implicit / expression return. This reverses an earlier decision
> that rejected implicit return. The same rule is what makes a function body
> return its final expression (see
> [Return Values](../09-functions/03-return-values.md)) and a guard expression
> return its `else` tail; the block is the foundational case the other two build
> on. An explicit `return` is reserved for early exit
> ([Early Return](../08-control-flow/06-early-return.md)).

## What

A block is a sequence of statements and bindings wrapped in braces. Its value
is the value of its **last expression**:

```tel
let label = {
    let base = lookup(id)
    let trimmed = base.trim()
    "[" & trimmed & "]"      # this is the block's value
}
```

- A block introduces a new [scope](../06-bindings-and-scope/05-scoping-rules.md).
  Bindings declared inside it are not visible outside.
- If the last item is not an expression (or the block is empty), the block
  yields the unit value.
- Blocks compose: an `if` is an expression whose arms are blocks, so
  `let x = if c { a } else { b }` works directly.

```tel
let fee = if an_order.is_rush { EuroAmt(5) } else { EuroAmt(0) }
```

Because a function body is itself a block, the same rule gives a function its
return value — the last expression is returned with no `return` keyword:

```tel
fn discounted(p: Price) -> Price {
    let off = p.value * 0.1
    p.value - off          # block's tail = function's return value
}
```

TODO(open): the exact unit/empty type — and its spelling — is an open question
across the docs (`()` versus a named `Unit`/`Nothing`).
Settle in [`05-types`](../05-types/02-primitive-types.md).

## Blocks as `{}` and as lazy code

Tel's syntax uses `{ ... }` for several things — block
expressions, lambda bodies, and the bodies of declarations. A `{}` block can
therefore stand in as a *lazy expression* / no-argument closure: a chunk of
code passed somewhere to be run later (see
[Closures and Lambdas](../09-functions/06-closures-and-lambdas.md) and
[Trailing block](../09-functions/02-parameters-and-arguments.md#trailing-block)).

TODO(open): whether a bare `{ ... }` in argument position is a *value-producing
block evaluated now* or a *deferred closure* depends on context, and the
disambiguation rule is not pinned down. This overlaps with lazy arguments
([Function Application](06-function-application.md#lazy-arguments)) and with
`04-syntax`. The braces-everywhere choice is a parsing decision that remains
unsettled.

## Why

- **Expression orientation** is a core feature
  ([philosophy](../02-philosophy/03-features.md)) — fewer statement/expression
  special cases, and small scripts avoid scaffolding.
- A block gives a name a small private workspace: helper bindings that should
  not leak into the surrounding scope live inside the braces.
- Returning a value from a block is also what makes Tel's control-flow
  constructs usable in `let` bindings and as arguments.

## See also

- [Expressions vs Statements](../04-syntax/02-expressions-vs-statements.md)
- [Blocks](../04-syntax/03-blocks.md)
- [Scoping Rules](../06-bindings-and-scope/05-scoping-rules.md)
- [Closures and Lambdas](../09-functions/06-closures-and-lambdas.md)
