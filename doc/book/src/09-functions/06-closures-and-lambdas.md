# Closures and Lambdas

A *lambda* is a function written as an expression, with no name. A lambda that
refers to bindings from the scope where it was created is a *closure*. Tel
makes lambda syntax deliberately light, because closures are the backbone of
[higher-order functions](07-higher-order-functions.md), iteration, and
DSL-style code.

> **Decision (reversal).** Lambda syntax was the single most-revisited topic in
> the design notes. It is now settled on the three forms below. This reverses
> earlier sketches — in particular the `{ it * 2 }` bare-block form, the
> Kotlin-style `{ a, b -> … }` form, and the terminator-less `\x -> body`
> form are all **dropped**. The governing rule: **parameters are never written
> inside `{ }`** — braces are only ever a block body.
>
> **Decision (`it` folded into the receiver).** The implicit lambda parameter
> `it` is **also dropped.** A block's single input is now its **receiver**,
> reached by bare name and named `self` — the same `self` a method body uses.
> `\` and receiver blocks are one feature, not two: see
> [§2](#2--shorthand--a-self-receiver-block) and
> [Lambda receivers](#lambda-receivers).

## What — three forms

A lambda is the same construct as a named function
([Function Declaration](01-function-declaration.md)) minus the name. There are
three spellings, lightest to heaviest.

### 1. Pipe form — `|params| body`

The common form. Parameters sit between pipes; the body is either a single
expression (terminated by the newline) or a `{ }` block (terminated by its
closing brace):

```tel
|x, y| x + y                 # expression body — ends at the newline

|x, y| {                     # block body — ends at the closing brace
    x
    + y
}

|| compute()                 # zero parameters
|x: Int64| x + 1               # types allowed where they help, optional where inferable
```

The pipes are self-delimiting: `||`, `|x|`, `|x, y|` all tell the parser "a
lambda starts here" at the first `|`, with no lookahead.

### 2. `\` shorthand — a self-receiver block

A block's single input is its **receiver**, reached by bare name and named
`self` — the same `self` a method body uses (see
[method syntax](08-method-syntax.md#self-is-implicit-inside-methods)). `\` marks
an **expression-bodied** trailing lambda that takes no *named* parameter; whether
it also has a receiver `self` is set by the parameter *type* — a receiver type
`Recv.fn() : R` supplies one, a plain `Fn() -> R` does not (see
[Lambda receivers](#lambda-receivers)):

```tel
orders.keep \ total > zero     # self = the order; `total` is self.total, bare
orders.map  \ id               # self = the order; bare field
xs.map      \ self * 2         # scalar element: name it `self` (no bare member)
```

There is **no implicit `it`.** The earlier `\` = `|it|` sugar is dropped: the one
thing a single-input block gets implicitly is its receiver `self`, unifying
lambdas, methods, and builder blocks under one rule — a bare name is a lexical
local, else a member of `self`. To *name* the input instead — turning **off**
bare resolution — use the pipe form `|x|`; to take more than one input, `|a, b|`:

```tel
|x| x ** 2         # pipe form — the input is named `x`, no bare fall-through
fn(x) { x ** 2 }   # full form
```

`\` inherits the pipe form's termination exactly: an expression body ends at the
newline, and `\{ … }` is the block-bodied version.

```tel
orders.map \ total                   # terse single-line use
orders.map \{ log(id); total }       # block body when you need statements
```

(A subtlety that needs no special rule: a block is itself an expression in Tel,
so `\ { … }` — read as "expression body that happens to be a block" — and
`\{ … }` — read as "block-bodied lambda" — denote the *same* function. There is
nothing to disambiguate.)

### 3. Full form — `fn` / `fun`

The same shape as a named function, just without the name. Reach for it when an
explicit signature or return type aids clarity. Like the others, it captures
surrounding bindings closure-style:

```tel
fn(x: Int64, y: Int64) -> Int64 { x + y }
```

TODO(open): the keyword spelling — `fn` versus `fun` — is not yet pinned down;
see [Function Declaration](01-function-declaration.md). `fn` is used throughout
for consistency.

## How far the body reaches

An **expression body** (`\ a`, `|x| a` — no braces) is a **single line.** It
runs to the end of the line (or to the enclosing `)` / `,`), and the newline
*ends* it. A brace-less closure body may **not** cross a newline. This is not an
arbitrary restriction: it is exactly what makes a leading-`.` chain continuation
unambiguous (a following `.method` can only continue the surrounding chain,
because the closure body has already closed at the newline) — see
[Whitespace and Newlines](../03-lexical-structure/08-whitespace-and-newlines.md):

```tel
\ self * 2                # body is `self * 2`
\ a +                     # ERROR: the body ended at the newline; `+` has no rhs
  b
\ (a +                    # OK: the open `(` holds the body across the newline
   b)
```

Because the newline closes the body, a brace-less lambda is for a single-line
body. To go multi-line, give it a `{ }` block body, which spans lines and ends at
its `}`:

```tel
nums.map \{
    let d = self * 2
    d + offset
}.filter(\ self % 3 == zero)      # `}` ends the block; `.filter` chains the call
```

A multi-line *chain* needs no wrapping: a leading `.` on the next line
continues the chain, and because each brace-less lambda body ends at its own
newline, the chain and the closures never run together:

```tel
nums
    .map(\ self * 2)
    .filter(\ self % 3 == zero)
    .sum()
```

So the division is simple: **brace-less bodies are one line; a multi-line chain
uses a leading `.`; a multi-line closure body takes braces.** To call a method
on the lambda *value* itself (rare), wrap the whole lambda: `(|x| a).f()`.

## The rule: no parameter *list* inside `{ }`

This is what "no parameters inside `{ }`" means: the old param-declaration
spellings — `{ it \ body }`, `{ a, b -> body }` — are **removed**. A `{ }`
never holds a parameter *list*. Named parameters are introduced *before* the
braces, by the pipes (`|x, y|`) or a `fn(...)` signature; the braces are only
ever the body. This is what keeps a bare `{ … }`
([block expression](../07-expressions/08-block-expressions.md)) and a lambda
distinguishable.

A bare trailing `{ }` after a call takes **no named parameter**. What it *does*
carry is a receiver `self` when its parameter type declares one — so `{ total }`
inside a receiver block reads `self.total` by bare name (see
[Lambda receivers](#lambda-receivers)), while a bare block over a plain
`Fn() -> R` parameter simply runs with no context. Either way there is no hidden
value argument and no implicit `it`: to bind a **named** parameter, mark it with
`|x| { … }`; for a single-expression self-receiver body, use `\ …`. See
[Function Application](../07-expressions/06-function-application.md#param-less-blocks-builders-and-control-flow).

## `self` — the implicit receiver

A single-input block reaches its input as `self`, by bare name — the *same*
`self` a method body uses, not a separate lambda parameter. **There is no `it`.**
Inside a receiver block a bare name is a lexical local first, otherwise a member
of `self` (a collision between the two is a compile error — see
[Lambda receivers](#lambda-receivers)); write `self` to name the context
explicitly, to disambiguate or to pass it along. A block that needs a *named*
parameter, or more than one, uses the pipe or full form instead — and a named
`|x|` parameter turns bare fall-through **off** (you write `x.field`).

## Why

- **The decision is made at the opening token.** `|`, `\`, and `fn` each mark a
  lambda unambiguously, so the parser never has to look ahead — or worse, scan
  the body for an `it` or a `->` — to decide whether a `{ … }` is a lambda or a
  plain block. A bare `{ … }` therefore stays cleanly a
  [block expression](../07-expressions/08-block-expressions.md).
- **Braces never bind parameters.** The dropped `{ it * 2 }` and `{ a, b -> … }`
  forms collided with block expressions (a bare `{ … }` already produces a
  value in Tel); resolving that collision would have required lookahead. Keeping
  braces purely as block bodies removes the ambiguity at the source.
- **`\` keeps the terse single-input case** — a self-receiver block whose body is
  one expression (`\ total`, `\ self * 2`) — without reintroducing a
  terminator-less form. It rides the same newline/brace termination as the pipe
  form, so the old `\x -> body` objection (no terminator) does not apply. The
  input is the receiver `self`, not a separate `it`, so `\` and receiver blocks
  are **one feature**, not two.
- **Light syntax matters** — closures are used constantly (`map`, `keep`, custom
  control flow), so the common cases (`\ total`, `|x| …`) stay nearly
  ceremony-free while the full `fn` form is there when clarity needs it.
- **Closures are values** — uniform with named functions, and the reason
  higher-order functions and trailing-closure DSLs work at all.

## Closures capture by binding

A lambda may read bindings from its surrounding scope:

```tel
fn make_adder(n: Int64) -> Fn(Int64) -> Int64 {
    |x| x + n                         # captures `n`
}
let add10 = make_adder(10)
add10(5)                              # 15
```

Captured bindings follow the usual rules: they are immutable unless `uniq`. A
lambda's parameters are an explicit binding site, so they may
[shadow](../06-bindings-and-scope/04-shadowing.md) an outer name (a bare
assignment cannot) — a lambda can therefore deliberately rebind a name for its
body, but never *accidentally* hide one. An immutable capture is a value the
closure simply carries; sharing it (even across tasks) is safe.

### The loop-variable capture trap (and why Tel avoids it)

A bug pattern common to JS, Python, and other languages that capture by
*variable* rather than by *value*: a lambda built inside a loop captures the
loop binding, so by the time the lambda runs, every copy reads whatever the
binding *now* holds — usually the last iteration's value.

```text
# JS / Python shape (illustrative — not Tel):
#   handlers = [(lambda: name) for name in names]
#   [h() for h in handlers]      # prints the LAST name N times
```

Tel sidesteps this by making each loop iteration a **fresh binding** (see
[`../06-bindings-and-scope/05-scoping-rules.md`](../06-bindings-and-scope/05-scoping-rules.md)),
not a reassignment of one shared variable. A closure built in iteration `i`
captures the *value* `names[i]` had at that iteration, not a cell that other
iterations also write to. The same rule is what makes parallel-loop bodies
safe: each iteration's captures are its own.

## Controlling capture

A capture's **mode** — by value, by borrow, or by move — is **inferred**, and
the default is almost always right:

- An **immutable** binding is captured **by value**: the closure carries a
  snapshot. There is no copy to worry about — the runtime is free to box the
  value and hand the closure a pointer — and sharing it, even across tasks, is
  safe because immutable values are `Alias`.
- A **`uniq`** binding is captured by a reusable exclusive borrow that keeps the
  outer binding live and lets the body mutate through it across calls. That is
  what makes such a closure `Fn` (see
  [Function Types](../05-types/05-function-types.md)).

Inference covers the common path with nothing written. When you want to
**override** it — most often to *move* a `uniq` value into a closure so the
closure can outlive the binding (be returned, or handed to a task) — an
optional **capture clause** lists *only the overrides*; every name not listed
stays inferred:

```tel
let add = |x| x + base                 # default — `base` captured by value

let step = |x| capture(move acc) {     # override — `acc` is moved into the closure
    acc.push(x)
}
```

Two override modes, deliberately spelled as **keywords, not sigils**:

- **`move <name>`** — the closure takes ownership; the value is moved in and the
  outer binding is consumed. This is how a closure escapes its defining scope
  (returned, or sent to a task that outlives it).
- **`borrow <name>`** — the closure captures a non-owning borrow; the outer
  binding stays usable and the closure is scope-bound.

The clause is never mandatory and never enumerates every capture — that is the
C++ capture-list trap. It exists for the rare override, and because an explicit
form makes capture *teachable*, the same reason lifetimes and the `Send` bound
are inferred-but-writeable (see
[TIP-0001](../tips/0001-mutability-and-borrowing.md)).

### Capture and call-arity (`Fn` vs `FnOnce`)

Capture mode decides whether a closure can be called more than once. A closure
is **`FnOnce`** iff calling it **consumes** (moves out) one of its captures;
otherwise it is **`Fn`**. So `move`-ing a value in and then handing it away on
the call makes the closure one-shot, while capturing by value-snapshot or by
borrow keeps it `Fn`. The arity is inferred from what the body does to its
captures; the `capture(...)` clause is how an author *states* — and forces — it.

## Lambda receivers

A lambda's single input is a **receiver** — the context object `self` its body
runs against — reached by bare name, so a DSL block like `html { … }` operates on
a builder, and `orders.map \ total` reads a row's field, without naming the
subject on every line. The mechanism is the **same everywhere**: receivers are
**not** confined to builders or records — the element of `list.map`, a dataframe
row, an HTML buffer, and a matched route are all just `self`, and there is no
separate implicit `it`. The design is settled in
[TIP-0010](../tips/0010-lambda-receivers-and-builder-dsls.md); the short of it:

- **A receiver closure is a method with `self` rebound.** Bare names resolve
  through `self` exactly as a method body does
  ([method syntax](08-method-syntax.md#self-is-implicit-inside-methods)) — so
  there is no *new* implicitness and no new resolution rule, just the method rule
  applied to a block. The one difference from a method is *who binds `self`*: a
  method's `self` comes from the caller (`x.f()`); a receiver block's is supplied
  by the function that runs it — once per row, or the builder buffer.
- **The receiver lives in the parameter type, not the literal.** A block
  parameter declares its receiver with the method-as-value shape from
  [function types](../05-types/05-function-types.md) — `Html.fn() : Unit` is "a
  block whose `self` is an `Html`." The literal stays a bare `{ … }` (or `\ …`);
  a named `|h| { … }` opts out, turning bare fall-through off.
- **The receiver can be owned, borrowed, or uniquely borrowed** — the *same*
  modes a method's `self` has, since a receiver *is* a rebound `self`. The mode
  rides the type: `Recv.fn() : R` is a shared (`Alias`) borrow — the read-only
  DSL / dataframe-row case; `!Recv.fn() : R` is a unique borrow — the mutating
  **builder** case; and a consuming form takes `!Recv` by value and ends it (a
  finaliser that returns the built value). Mutability follows from the receiver
  type as everywhere else — `!` / `uniq` track ownership, not mutation.
- **The leading `.` is left to chaining** — reusing it for the receiver would
  make `html { .head() / .body() }` parse as the chain `body(head(self))`,
  colliding with leading-`.`
  [chain continuation](../03-lexical-structure/08-whitespace-and-newlines.md).
- **One context, innermost only** — no outer-receiver scope chain. Reach an
  *outer* builder by switching that one level to a named `|h| { … }` lambda.
- **Name clashes resolve by whether the outer name is re-spellable:**
  - a **local or parameter** (which has no qualified spelling) that collides with
    a receiver member is a **compile error** — qualify the member as `self.x`, or
    rename the local. This is stricter than any comparable language (Kotlin,
    Swift, C#, Scala all let the local win); Tel starts strict because relaxing
    later is backward-compatible and tightening is not.
    `TODO(open):` relax if dogfooding shows the error is too noisy — the sharpest
    case is a parameter that shadows the receiver's *own* field (the `self.x = x`
    constructor idiom), where "local wins, member via `self.`" is the natural
    fallback.
  - a **free function** does not really clash: UFCS makes `foo` and `self.foo`
    the same call.
  - a **global with a qualified path** (a type, a module constant) loses to the
    receiver member for the bare name; reach the global by its path. Nothing is
    lost or silent, so this needs no error.
- **Receiver-ness is orthogonal to escape-ness.** A receiver closure is an
  ordinary value: it may be stored, returned, or run later — binding a context
  says nothing about control flow. (Contrast an `inline` block, below, which may
  *not* escape; a receiver block that is *also* inline must obey that
  non-escaping condition to keep its `outer` powers.)

The three directions never collide: the `.` stays chaining, a **bare name**
reaches into `self`, and **`outer`** (below) reaches up into the declaring
function. `TODO(open):` confirm the exact `Recv.fn() : R` receiver spelling when
the block *also* binds an ordinary `|x|` parameter — left open by
[TIP-0010](../tips/0010-lambda-receivers-and-builder-dsls.md).

## Lambda `return`

A `return` inside a lambda exits the *lambda*, producing its result — it does
not exit the function that built the lambda. Exiting the enclosing function
from a block argument is a separate, opt-in feature of *inline* functions; see
[Return Values](03-return-values.md#returning-from-an-outer-scope).

A block passed to an **`inline`** function may carry non-local control flow, so
a user-defined `unless` / `with_lock` / early-exit iterator reads with built-in
*capability* — not built-in *invisibility*. The rule (settled in
[TIP-0009](../tips/0009-inline-lambdas-and-non-local-control-flow.md)): a bare
`return` stays **local** to the block, while an explicit `outer return` /
`outer break` / `outer continue` leaves the **declaring function** — the
function whose source the block is written in. `inline` on the function grants
the permission (the block must not escape); `outer` at the jump states the
intent, so the seam between a real built-in (`for` breaks with a bare `break`)
and an inline helper (`outer break`) stays visible. Never implicit.

The block below is written inside `find_ready`, so `outer return` leaves
`find_ready`, while falling off the end of the block simply advances the
(inline, user-defined) `each` iterator to the next row.

```tel
# `each` is `inline`, so its block may carry `outer` jumps.
fn find_ready(rows: List[Row]) -> Option[Row] {
    rows.each |row| {
        if row.ready {
            outer return Some(row)   # leaves find_ready with the first hit
        }
        # fall off the block end → `each` moves to the next row
    }
    None                             # implicit tail: nothing was ready
}
```

The two levels compose: a bare `return v` would hand `v` back to `each` as this
row's per-item value; `outer return v` makes `find_ready` itself return `v`. A
reader greps `outer` to find every non-local exit, and the `outer` modifier
matches the `move` / `borrow` capture keywords — never implicit.

Reach for `outer return` when the block is genuinely *driving the caller's
control flow* — early-exit iteration, `with_lock`, `log.sub(...)`. For code that
merely *picks one of several results* — a request router, say — prefer a
[`match`](../08-control-flow/02-match-expressions.md) in tail position over a
chain of blocks that each `outer return`: the `match` is exhaustive, dispatches
on type, and needs no non-local jump.

`TODO(open):` three sub-questions the `inline`/`outer` rule leaves open — kept
open by [TIP-0009](../tips/0009-inline-lambdas-and-non-local-control-flow.md):
multi-level `outer break` (reaching a loop *beyond* the immediately enclosing
one); **effect-row sharing** (an `inline` function's effect signature must stay
transparent to the spliced block's effects — it cannot claim `pure` while
splicing effectful caller code); and the **`return`/`yield` disambiguation** for
a block driving a generator-style helper (resolve with
[for-loops-and-iteration](../08-control-flow/04-for-loops-and-iteration.md)). The
relationship to [lazy arguments](../07-expressions/06-function-application.md#lazy-arguments)
— both are "code passed unevaluated, run in the caller's context" — is likewise
open.

## See also

- [Function Declaration](01-function-declaration.md)
- [Higher-Order Functions](07-higher-order-functions.md)
- [Function Application](../07-expressions/06-function-application.md) — trailing-closure calls.
- [Function Types](../05-types/05-function-types.md)
- [Block Expressions](../07-expressions/08-block-expressions.md)
