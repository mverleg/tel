# If Expressions

`if` in Tel is an **expression**: it produces a value, like every other branching
construct. It is the simplest way to choose between two values or two pieces of
behaviour.

## What it is

An `if` evaluates a boolean condition and runs one of two branches. Because it is
an expression, the chosen branch's value becomes the value of the whole `if`.

```tel
let label = if score >= EuroAmt(0) { "ok" } else { "rejected" }
```

When the branches are used as statements — run for effect, not for a value —
the `else` may be omitted and the `if` produces nothing (the unit value).

```tel
if order.flagged {
    log.warn("flagged order", order.id)
}
```

The condition must be a genuine `Bool`. Tel has **no truthy/falsy coercion**: a
number, a string, an `Option`, or a collection is *not* a condition. Write the
comparison out. This is a direct consequence of *no implicit conversions* — see
[antifeatures](../02-philosophy/04-antifeatures.md). For collapsing `none` or a
falsy-like value to a fallback, use the fallback operator described in
[`07-expressions/11-fallback-operator.md`](../07-expressions/11-fallback-operator.md),
not `if`.

## The condition stops at the `{`

In an `if`, the first `{` after the condition is **always** the then-block — the
parser does not look inside it or scan ahead. That keeps `if` unambiguous with no
lookahead, but it has one consequence worth stating: if the condition itself ends
in a [trailing-block call](../07-expressions/06-function-application.md#trailing-block-outside-the-parens)
(`f { … }`), wrap that call in parentheses so its block is not mistaken for the
`if` body.

```tel
if cache.get_or(key) { compute() } { use(it) }      # AMBIGUOUS — don't
if (cache.get_or(key) { compute() }) { use(it) }     # OK — () closes the condition
```

The same rule applies to any control-flow head followed by a `{ }` body
(`while`, `for`): the `{` opens the body, so a trailing-block call in the head
goes in `()`. In practice conditions rarely use trailing-block calls, so this is
a corner the parser handles cleanly rather than a case you hit often.

### Why no `then` keyword

Some languages separate the condition from the body with a keyword —
`if c then { … }`, or a `do`. Tel needs none, and the reason is the *same*
decision that shapes lambdas: **every lambda wears an opening marker**
(`|x|`, `\`, or `fn` — see
[Closures and Lambdas](../09-functions/06-closures-and-lambdas.md)), so a bare
`{` can only ever open a *block body*, never a parameter-bearing DSL block. The
first `{` after the condition is therefore unambiguously the then-block, with
nothing left for a `then` to disambiguate.

The alternative was the mirror image: allow bare, *unmarked* DSL blocks (a
`widget { … }` that declares its own parameters). That would make
`if cond { body }` genuinely ambiguous — is `{ body }` the then-block, or a
block argument to `cond`? — and force a delimiter (`then` / `do`) on **every**
`if`, `while`, and `for` to mark where the condition ends. Tel rejects that:
marking lambdas is a cost paid once, at the lambda; a mandatory `then` is a tax
on every conditional. Keeping the common case ceremony-free wins (*readability
and terseness for the form scripts use most*).

## Why an expression

Expression orientation keeps small scripts small — the priority *embedded
scripts over standalone projects* favours forms that compress a 30-line hook.
A statement-only `if` forces a mutable temporary:

```tel
# Statement style — needs a mutable binding just to carry the result.
let uniq label = ""
if score >= EuroAmt(0) { label = "ok" } else { label = "rejected" }
```

The expression form has one binding, no mutation, and the type checker verifies
both branches produce a compatible type.

When an `if` is used for its value, **both branches are required**: a missing
`else` would leave the value undefined on one path. A value-producing `if`
with no `else` is a compile error, not a silent `none`.

## Chaining

`else if` chains read the familiar way. There is no separate `elif` keyword —
*familiarity over a novel surface*.

```tel
let band =
    if n < 10 { "small" }
    else if n < 100 { "medium" }
    else { "large" }
```

When the choice is over the *type* of a value rather than a boolean, reach for
[`match`](02-match-expressions.md) instead — it is exhaustive and the compiler
checks that every case is handled.

## How it relates to `match`

A two-armed `if` and a two-armed `match` overlap. The guideline:

- Use `if` for a boolean test.
- Use `match` to dispatch on which member of a union a value is, or to compare
  against several patterns.

The block delimiter itself is settled: `{ }` is the sole block delimiter
everywhere (see [Blocks](../04-syntax/03-blocks.md)), so an `if` body, an `else`
body, and a then-block all use the same braces with no alternative spelling.
