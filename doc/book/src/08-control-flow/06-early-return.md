# Early Return

`return` exits the enclosing function immediately, producing the given value.
Tel is expression-oriented — the last expression in a function body is already
its result — so `return` is reserved for *early* exit: bailing out before the
end on an unhappy path or a handled special case.

> **Decision (reversal).** Implicit / expression return **is supported**: a
> function's final expression is its value, with no `return` keyword (see
> [Return Values](../09-functions/03-return-values.md) and
> [Block Expressions](../07-expressions/08-block-expressions.md)). This reverses
> an earlier decision that would have required an explicit `return` everywhere.
> `return` therefore means *early* return only.

## Explicit `return` vs the implicit tail value

The two ways a function yields a value are deliberately distinct:

- **Implicit tail value.** The last expression of the body — or of a guard's
  failure block — is the result. No keyword. This is the normal, happy-path
  exit and stays unindented.
- **Explicit `return`.** Used only to leave *before* reaching that tail
  expression — a guard at the top, an `Err` bail-out, an early hit inside a
  loop. Seeing `return` tells a reader "this is a deliberate short-circuit," not
  the ordinary path.

```tel
fn lookup_price(id: Id, cache: Cache) -> Price {
    if let cached = cache.get(id) {
        return cached            # explicit early return
    }
    fetch_price(id)              # implicit tail return — the normal path
}
```

## What it is

```tel
fn score(order: Order) -> Result[Score, Reject] {
    if order.total <= EuroAmt(0) {
        return Err(Reject.NonPositiveTotal)   # early exit, skip the rest
    }
    Ok(Score.from(order))                     # normal tail-position result
}
```

The happy path falls off the end of the function as a plain expression; `return`
handles the exceptional path. This keeps the common case unindented and lets
error handling fold away — a guard at the top, the real work below.

## `guard` — bail out on the unhappy path

A `guard` is the dedicated tool for the "check a precondition, leave if it
fails, otherwise carry on" shape. It keeps the failure handling at the top and
the real work unindented below.

```tel
guard X {
    ... # the failure block — must diverge
}
# X held; execution continues here
```

`guard X { ... }` is exactly `if not X { ... }` with one extra rule: the block
**must diverge** — every path through it has to leave the current scope
(`return`, a panic, `break`, or `continue`). The compiler rejects a `guard`
whose block can fall through, because the whole point is that code *after* the
guard runs only when the condition held.

The condition may be a **pattern match that binds**, and — unlike `if let`,
whose bindings are scoped to its block — a `guard` binding stays in scope
*after* the guard:

```tel
fn area(shape: Option[Shape]) -> Result[Area, Reject] {
    guard let Some(s) = shape {
        return Err(Reject.Missing)   # diverges; `s` is not bound here
    }
    # `s` is in scope from here on, and is known to be a Shape
    Ok(s.area())
}
```

This is what makes `guard` more than sugar for `if`: it lets a precondition
*introduce* a binding for the rest of the function rather than nesting the
happy path inside an `if let`. Because the failure block diverges, an implicit
tail expression in it is a function return like any other (see
[Return Values](../09-functions/03-return-values.md)).

`TODO(open): exact divergence set — is `break`/`continue` accepted in a guard
block inside a loop, or only `return`/panic? Lean: any divergence.`

`TODO(open): how does `guard` relate to the [fallback operator](07-error-propagation.md)
and to refined-type narrowing? They overlap for the "unwrap or bail" case;
pin down when to reach for which.`

## `return` from anywhere

`return` works from inside any nested construct — an `if` branch, a
[`match`](02-match-expressions.md) arm, a loop body. It always exits the
enclosing **function**, not the nearest block.

```tel
fn classify(outcome: Result[Score, Reject]) -> Score {
    match outcome {
        Ok(score) -> score,
        Err(_)    -> return Score.zero(),
    }
}
```

Tel deliberately does **not** copy Java's rule forbidding `return` inside a
switch expression — see [match expressions](02-match-expressions.md). A `match`
arm is an ordinary block; it may `return`.

## Lambdas and the enclosing-function question

`return` exits the function it is written in. When the `return` is inside a
**lambda** passed to something like a `for`-each helper, "the enclosing
function" could mean two things — the lambda, or the function that contains the
lambda. Tel resolves this in favour of the obvious default, with an explicit
opt-in for the non-local case (see
[TIP-0009](../tips/0009-inline-lambdas-and-non-local-control-flow.md)):

- A bare **`return`** inside a lambda is **local** — it exits the *lambda*,
  producing its result, exactly as `break` / `continue` inside the lambda act on
  the lambda. This is the settled default across the chapters.
- **`outer return`**, **`outer break`**, and **`outer continue`** leave the
  **declaring function** — the function in whose source the block is written —
  not the helper that received the block. The `outer` keyword is written at the
  jump, so every non-local exit is greppable and never silent.

The non-local form is enabled only when the receiving function is marked
**`inline`**: `inline` on the function grants the permission (the block is
spliced into the caller and must not escape), and `outer` at the jump states the
intent. A bare `break` ends the block (one step of an inline iterator);
`outer break` stops the loop the inline call stands for. This is what lets a
user-defined `with_lock`, `unless`, or early-exit iterator read with built-in
*capability*, while the one-keyword difference from a real built-in keeps the
seam visible — *surprise is a cost — prefer the obvious.* Full mechanics and the
worked `find_ready` example are in
[Closures and Lambdas](../09-functions/06-closures-and-lambdas.md#lambda-return);
the `inline` marker itself is described in
[Return Values](../09-functions/03-return-values.md#returning-from-an-outer-scope).

This is the machinery the results-vs-callbacks analysis in
[`13-error-handling/01-philosophy.md`](../13-error-handling/01-philosophy.md)
refers to: non-local `return`/`break` is exactly what a plain callback cannot do,
and an `inline` block is how a helper regains it.

`TODO(open):` sub-questions the `inline`/`outer` rule leaves open (kept open by
[TIP-0009](../tips/0009-inline-lambdas-and-non-local-control-flow.md)): the
`return`/`yield` disambiguation for generator-style helpers, multi-level
`outer break` targeting a loop beyond the immediately enclosing one, and how an
`inline` function's effect signature stays transparent to the spliced block's
effects.

## `return` is not the error mechanism

`return` moves control; it does not by itself signal an error. Errors are
*values* — an `Err` member of a `Result`-shaped type — and `return Err(...)` is
just an early `return` whose value happens to be an error. The terse way to
propagate an error without writing the `return` out is the propagation operator;
see [error propagation](07-error-propagation.md).
