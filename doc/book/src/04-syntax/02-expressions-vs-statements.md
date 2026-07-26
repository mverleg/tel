# Expressions vs Statements

Tel is **expression-oriented**. Most constructs produce a value, small scripts
stay small, and *a single expression is a valid program* — no `main`, no
boilerplate wrapper. Statements still exist (a binding, a bare block used for
its effects), but the default mental model is "an expression that yields a
value", not "a sequence of commands."

This page covers the *syntactic* shape of expressions and statements. The
semantics of each construct live in [Expressions](../07-expressions/) and
[Control Flow](../08-control-flow/).

## Why expression orientation

It serves *embedded scripts over standalone projects* and *readability*. A
modding hook or a per-message transform is often a few lines that compute one
result; making that the natural unit — rather than a function full of
statements — keeps the common case tiny. Java's refusal to `return` from a
`switch` expression is a mistake Tel does not copy: control-flow constructs
are expressions and yield values.

## Statement termination

A statement ends at a newline, or at an explicit separator when statements share
a line. Indentation is irrelevant. The lexical detail is in
[Whitespace and Newlines](../03-lexical-structure/08-whitespace-and-newlines.md)
and [Layout Rules](05-layout-rules.md).

## Blocks and the `{}` rule

`{ ... }` always delimits a **body** — see [Blocks](03-blocks.md). The same
braces serve a bare block, a declaration body (function / struct / enum), and a
lambda body. The decisive rule: **braces never carry parameters.** Whether a
`{ ... }` is a block, a declaration body, or a lambda body is fixed by the
token that *precedes* it (a `fn(...)` / `|...|` / `\` opener, or a declaration
head), never by scanning what is inside. A bare `{ ... }` in expression
position is therefore unambiguously a
[block expression](../07-expressions/08-block-expressions.md), not a lambda.

## Lambdas

A lambda is a function written as an expression. Lambda syntax is meant to be
**light** — passing a small function to `map` or `keep` should be unremarkable.
Tel settles on three spellings, each of which marks itself at its opening token
so the parser needs no lookahead:

- **`|params| body`** — the common pipe form. `|x, y| x + y` (expression body,
  ends at the newline) or `|x, y| { ... }` (block body). `||` is the
  zero-parameter form.
- **`\ body`** — a self-receiver block: the block's single input is its receiver
  `self`, reached by bare name (`\ total` is `self.total`; `\ self * 2` for a
  scalar element). There is no implicit `it`; to *name* the input, use the pipe
  form, which turns bare fall-through off.
- **`fn(a, b) -> T { ... }`** — the full form, identical to a named function
  minus the name; used when an explicit signature aids clarity.

No form ever writes parameters inside `{ }`. That is what removes the old
block-versus-lambda ambiguity; the full treatment is in
[Closures and Lambdas](../09-functions/06-closures-and-lambdas.md).

```tel
# pipe form as an argument
xs.map(|x| x * x)

# `\` shorthand, here as a trailing lambda outside the parens
xs.map \ self * self

# full form for the multi-arg case
fold(0, xs, fn(acc, x) { acc + x })

# zero-argument thunk
defer(|| cleanup())
```

A lambda passed as the **last** positional argument may sit outside the call
parentheses (the Kotlin / Ruby trailing-lambda idiom), letting library functions
read like control flow. A **param-less** trailing argument is a bare `{ … }`
block; one whose input is taken as the receiver `self` carries a `\`, and one
that **names** its input carries `|x|`, right before the brace. The marker is the
only difference — a bare `{ … }` never silently gains an input, and there is no
implicit `it`:

```tel
with_lock(lock) { update(state) }            # param-less trailing block
unless(online)  { queue_for_later(request) }

items.keep \{ price > zero }                 # trailing lambda; self = the item
fold(0, xs) |acc, x| { acc + x }             # trailing lambda, named params
```

Lambdas **do** carry a Kotlin-style receiver/context (the `self` above): a
block's single input is its receiver, settled in
[Closures and Lambdas](../09-functions/06-closures-and-lambdas.md#lambda-receivers)
and [TIP-0010](../tips/0010-lambda-receivers-and-builder-dsls.md).

`TODO(open): trailing-lambda `()` elision and eager-vs-deferred braces.` Whether
the parentheses may be dropped entirely for a trailing marked lambda
(`xs.map |x| ...` vs `xs.map(|x| ...)`), and whether a trailing `{ }` is ever a
deferred thunk rather than an eager block, are tracked in
[Function Application](../07-expressions/06-function-application.md) and
[Block Expressions](../07-expressions/08-block-expressions.md).

## Method-call chaining

Tel encourages **left-to-right chaining** with `.`:

- `x.f(y)` calls `f(x, y)` — the receiver is just the first argument. There is
  *no dynamic dispatch, just `f(x, y)`*: any function whose first parameter
  matches `x`'s type can be called in method position. (How this
  interacts with trait dispatch is a [Functions](../09-functions/) question.)
- A chain that spans lines needs no wrapping: a leading `.` on the next line
  continues it (see
  [Whitespace and Newlines](../03-lexical-structure/08-whitespace-and-newlines.md)).

```tel
orders.keep(|o| o.total > EuroAmt(0)).map(|o| score(o)).sorted()   # one line

orders                                                             # or multi-line
    .keep(|o| o.total > EuroAmt(0))
    .map(|o| score(o))
    .sorted()
```

`TODO(open): the extra call-position operators.` There are sketches for
`x:f(a)` meaning `f(x, a)` and `x.f(a, it)` meaning `f(a, x)` — placing the
receiver in a non-first argument slot. These are powerful for DSLs but multiply the ways to
spell a call, which cuts against *one good way*. Decide whether `x.f(y)` is the
only chaining form or whether `:`-call and placeholder-`it` calls also ship.

## Postfix operators

A handful of *operators* (not keywords) are written postfix so they layer
into a left-to-right chain without forcing the reader to re-read backwards:

- A postfix `?` for early-return on `Result` / `Option` (see
  [Error Propagation](../08-control-flow/07-error-propagation.md)). Reading
  `parse(s)?.process()` left-to-right: "parse, then if Ok, process."

`TODO(open): postfix `not`.` Should `not` also be postfix to fit the chain
("do X, then negate")? Lean: no —
`not x` reads as English and breaks the pattern only mildly. The same goes
for unary minus: `-x` is too entrenched to flip to postfix.

## Postfix keywords

Some keywords are written **after** their operand so a transformation reads
left-to-right and chains naturally — `return`, `assert`, `then`
(with `if`), and a `for`/`forEach` form (also `print`, but see the caveat
below).

```tel
score(order)            return        # "compute, then return it"
total > EuroAmt(0)      assert         # "this must hold"
```

`TODO(open): postfix-keyword scope and spelling.` This is exploratory. Mixing
prefix and postfix keyword forms can surprise readers from mainstream languages,
which cuts against *familiarity*. Also: `print` is listed as a postfix keyword,
but Tel has **no ambient output** — output is a capability the host injects, so
`print` is almost certainly an ordinary function that merely *reads* postfix via
chaining, not a keyword. `TODO(open): pre-pivot — re-justify `print`; treat it
as a capability-provided function, not a keyword.` Decide the final postfix set
in [Keywords](../03-lexical-structure/04-keywords.md).

`TODO(open): return/yield/break disambiguation.` How are `return` and
`yield` from inside a lambda told apart from `return`/`yield` of the
enclosing function? (`break`/`continue` just target the nearest loop.) This is a
control-flow question — defer to [Control Flow](../08-control-flow/).

## Named arguments

Calls may pass arguments **by name**, which ties directly to
stability: named and default arguments let a function signature gain parameters
without breaking existing callers. There is also a shorthand where a bare local
name supplies the argument of the same name.

```tel
let p = Point(x = 4, y = 2, z = 7)

# shorthand: a bare name means `name = name`
let x = 4
let y = 2
let p = Point(x, y, z = 7)
```

`TODO(open): named-argument detail.` Open points: whether named arguments are
freely reorderable, how they interact with positional arguments, and whether the
"struct literal from same-named locals" punning (the `Point { x, y, *old }`
form, including spread of another value's fields) is part of *call* syntax or
only *record-construction* syntax. This matters mainly for codegen
and API-evolution code. Detail belongs in
[Default and Named Arguments](../09-functions/04-default-and-named-arguments.md)
and [Records](../10-data-modelling/01-records.md); keep this page to the syntax
sketch.

TODO: review
