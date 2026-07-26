# Return Values

<!-- TODO: review -->

A function produces its result as the value of its body. Tel is
expression-oriented, so an explicit `return` is the exception, not the rule.

> **Decision (reversal).** Implicit / expression return — the Rust-style rule
> where a block's final expression *is* its value — **is supported**. This
> reverses an earlier decision that rejected implicit return in favour of an
> always-explicit `return`. The `return` keyword still exists, but only for
> *early* exit. The same tail-expression rule applies uniformly in
> [block expressions](../07-expressions/08-block-expressions.md), function
> bodies (here), and guard expressions (below).

## What

The body of a function is a [block expression](../07-expressions/08-block-expressions.md);
the function's result is the value of that block's last expression — no `return`
keyword is written:

```tel
fn double(arg: Int64) -> Int64 {
    2 * arg            # implicit return — this expression is the result
}

fn label(id: Id) -> Text {
    let base = lookup(id)
    "[" & base.trim() & "]"     # final expression is the function's value
}
```

`return` exists for **early exit**, typically a guard:

```tel
fn score(an_order: Order) -> Result[Score, Reject] {
    if an_order.total <= EuroAmt(0) {
        return Err(Reject.NonPositiveTotal)   # bail out early
    }
    Ok(Score.compute(an_order))               # normal result, no `return`
}
```

The declared return type after `->` is checked against every path that
produces a result — whether that path ends in an explicit `return` or simply
falls off the end as a tail expression.

### Multiple results are a tuple

A function returning several values returns a **tuple**
(`-> (Int64, Int64)`, or named `-> (sum = ..., count = ...)`) — the same labelled
row shape as a [call's argument bundle](../05-types/04-tuples-and-arrays.md#tuples-as-argument-bundles).
That makes the result feed straight into the next call by
[splatting](../07-expressions/06-function-application.md#composition-by-splatting-a-return-tuple),
positional returns lining up by position and named returns by name. A **single**
result is not a one-tuple (`-> Int64` returns an `Int64`), so the common case
stays free of tuple ceremony.

### A guard's failure block returns its final expression

The implicit-return rule extends to a [`guard`](../08-control-flow/06-early-return.md).
A guard's failure block must diverge, and the ordinary way to diverge is to
supply the function's return value as its tail expression — no `return`
keyword needed:

```tel
fn first_word(text: Text) -> Text {
    guard let Some(head) = text.words().first() {
        ""                       # tail value is the function's return
    }
    head                         # `head` is in scope; the happy path
}
```

The guard's failure block returns its last expression exactly as a block
expression's tail does, keeping the three sites consistent: block, function
body, and a guard's failure block all return their final expression. The guard
form itself is defined in [early return](../08-control-flow/06-early-return.md).

## Returning from an outer scope

A normal `return` exits the function it textually sits in. Two features
complicate this, and each has its own topic:

- A [lambda](06-closures-and-lambdas.md)'s bare `return` exits the *lambda*, not
  the function that created it.
- An **inline function** can be defined so that an `outer return` written in a
  block-argument passed to it exits the **declaring function** — the function in
  whose source the block is written. This is what lets user code define
  control-flow-like helpers (a custom `unless`, an early-exit iterator). See
  [Trailing block](02-parameters-and-arguments.md#trailing-block).

The rule is settled
([TIP-0009](../tips/0009-inline-lambdas-and-non-local-control-flow.md)): a bare
`return` / `break` / `continue` in a block is **local** to that block, while
`outer return` / `outer break` / `outer continue` reach the declaring function.
Only a block passed to an `inline`-marked, **non-escaping** parameter may use
the `outer` forms — you cannot jump into a frame that has already gone. The
`outer` keyword at the jump makes the non-local exit visible; the `inline`
marker on the receiving function grants the permission. See
[Closures and Lambdas](06-closures-and-lambdas.md#lambda-return) for the worked
example. (The `return`/`yield` disambiguation for generator-style helpers, and
`inline`'s interaction with effect inference, remain open — tracked there.)

## Errors are returned, not thrown

Tel has no exceptions. A function that can fail returns a `Result`-shaped
value; the caller propagates or handles it explicitly. There is no hidden
control flow out of a function other than an outright abort. See
[Error Handling](../13-error-handling/) and
[Error Propagation](../08-control-flow/07-error-propagation.md).

## Panic and non-termination are not part of the return type

A function can also leave by *not returning a value at all* — it can **panic**
(abort the task) or run forever. Neither is a component of the declared return
type `R`. They are tracked elsewhere, by two separate mechanisms, so the return
type stays "the value a normal completion yields":

- **Panic is the `panics`
  [effect](../05-types/05-function-types.md#effects-belong-on-the-function-type)**,
  not a return-type alternative. Most ordinary functions carry it (almost
  anything can, in principle, panic — out-of-memory, an overflow check, deep
  recursion); a `pure`/`total` function provably does not. The effect is
  inferred for concrete functions and only spelled at a generic bound. A panic
  is delivered through a default-on ambient
  [capability](../02-philosophy/03-features.md) and is *handled* at the
  [task boundary](../14-concurrency-and-parallelism/02-tasks.md#tasks-are-the-panic-boundary),
  where it surfaces as a `Result[R, PanicInfo]` at `join`.
- **Non-termination** is the `total` property's concern (a `total` function is
  guaranteed to terminate). It is likewise tracked as a function property, not
  encoded into `R`.

A function that *never* returns normally — one that always aborts or loops
forever — has return type [`Never`](../05-types/14-never-type.md),
the uninhabited bottom type. Note the notation trap: an earlier note wrote
every return type as `(R | ! | ∞)` with `!` meaning *panic*, but `!` is a
candidate spelling for `Never` (the bottom type), which is a different idea —
`Never` is "produces no value," whereas the `panics` effect is "may abort." Keep
them distinct: divergence is typed `Never`; the ability to panic is an effect.

TODO(open): How much of the effect/termination tracking ships in 1.0. The
`panics` / `pure` / `total` alphabet is the decided home (see
[function types](../05-types/05-function-types.md#effects-belong-on-the-function-type)),
but whether `total` (full termination checking) lands in 1.0 is open — the
ordinary `Result` story plus an opt-in [`const`](../15-metaprogramming/04-compile-time-evaluation.md)
discipline already covers most real needs, and `const` is the cleanest
"prove this terminates" point we already have.

## Why

- **Expression-oriented results** keep small functions tiny — a one-line
  function is one line.
- **`return` for early exit only** means a reader sees `return` and knows it
  marks a deliberate short-circuit, usually a guard, not the normal path.
- Errors-as-values keeps every exit from a function visible in its type.

## See also

- [Function Declaration](01-function-declaration.md)
- [Block Expressions](../07-expressions/08-block-expressions.md)
- [Early Return](../08-control-flow/06-early-return.md)
- [Trailing block](02-parameters-and-arguments.md#trailing-block) — non-local return.
