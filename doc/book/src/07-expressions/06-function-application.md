# Function Application

<!-- TODO: review -->

Applying a function to arguments is the most common expression in Tel. The
core form is conventional — **every call is written with parentheses** — with
two narrowly-scoped conveniences for the cases that earn them: a single trailing
block or lambda written outside the parens, and lazy arguments. Both are bounded
so that a call reads the same everywhere.

Space (juxtaposition) application — writing `f a b` to mean `f(a, b)` — was
considered and **rejected**; see
[Antifeatures](../02-philosophy/04-antifeatures.md).

## What

The basic call is familiar:

```tel
double(21)
clamp(value, low, high)
an_order.total()
```

Arguments are evaluated left to right (except for *lazy* arguments — see
below), then the function runs. Because functions are values bound to names
([Function Declaration](../09-functions/01-function-declaration.md)), the thing
being called can be any expression that produces a function.

Calls also support default and keyword arguments
([Default and Named Arguments](../09-functions/04-default-and-named-arguments.md))
and method-style call syntax
([Method Syntax](../09-functions/08-method-syntax.md)).

## Parentheses are required

Every call shows its parentheses — `f()`, `f(x)`, `an_order.total()`. There is
**no bare-name call**: a name on its own is the *value* it is bound to, never an
implicit zero-argument invocation.

```tel
an_order.total      # the field (or computed property) `total` — NOT a call
an_order.total()    # calls total(an_order)
double              # the function value — pass it straight to a higher-order fn
xs.map(double)
```

This is the deliberate resolution of a would-be ambiguity. If `point.x` could
mean either "read field `x`" or "call zero-arg function `x`", the two would be
indistinguishable at the call site. Requiring `()` on every call removes the
question outright:

- `point.x` is **always** a field or computed-property read, never a call.
- `point.x()` is **always** a call.
- a bare name is **always** the value, so passing a function to a
  [higher-order function](../09-functions/07-higher-order-functions.md) needs no
  special "do not call it" syntax — `double` already *is* the function.

The cost is that a stored field `x` and a computed getter `x()` are written
differently, so swapping one for the other is a visible change at every call
site rather than a transparent one. Tel takes that trade: a call site that means
exactly one thing is worth more than source-compatible field↔getter swaps, and
it matches the language's general refusal of implicit anything. This is settled
together with [Field and Index Access](07-field-and-index-access.md) and
[Method Syntax](../09-functions/08-method-syntax.md).

## Trailing block outside the parens

The one exception to "parentheses are required": the **last argument may be a
trailing block or lambda written outside the `()`**, and if it is the *only*
argument the empty `()` may be dropped. This is what lets a library function read
like a built-in control structure or a DSL.

```tel
with_lock(lock) {              # block is the last arg; the other args stay in ()
    update(state)
}

html {                         # block is the only arg — empty () dropped
    head { title("Hi") }
    body { p("...") }
}

# the in-parens form is always equivalent
with_lock(lock, { update(state) })
html({ ... })
```

Two rules keep this unambiguous:

1. **The trailing block opens on the same line as the call.** Its opening
   token — the `{`, or the `|`/`\` of a block-bodied lambda — sits on the same
   line as the callee name or the `)` before it. So `html {` is a trailing-block
   call, while `html` alone on a line is just the value `html`, and a `{ … }` on
   the *next* line is a separate
   [block expression](08-block-expressions.md). The decision is a single local
   lookahead — "is the next token, on this line, a block opener?" — needing no
   indentation rules.

2. **A newline ends the trailing argument unless a grouping is still open or
   the next line begins with `.`.** Tel has no implicit line-continuation by a
   trailing operator or a `...`; an expression spans several lines only inside
   `(`/`{`/`[`, or as a method chain continued by a leading `.` on the next line
   (see [Whitespace and Newlines](../03-lexical-structure/08-whitespace-and-newlines.md)).
   So a brace-less trailing lambda (`xs.map \ total`, `xs.filter |x| x > 0`)
   is a single line — its body ends at the newline, which is exactly what lets a
   following `.step` continue the chain; for a multi-line *body*, give the lambda
   a `{ }` block. There is never a question of how far down the page a trailing
   argument reaches.

### Param-less blocks: builders and control flow

When the trailing argument takes no parameters it is a bare `{ … }` block.
Because [parameters are never written inside braces](../09-functions/06-closures-and-lambdas.md#the-rule-no-parameters-inside--),
a bare block is unambiguously param-less — exactly what a builder or a
control-flow helper wants. A user-defined `unless` then reads almost like an
`if`, with no marker on its block:

```tel
fn unless(cond: Bool, body: Block) { if not cond { body() } }

unless(online) {
    queue_for_later(request)
}
```

The markup DSL is the same shape nested: `html`, `head`, and `body` each take a
block, and the calls inside each block run in sequence to build the tree.

A param-less builder block typically runs against a **receiver** — an implicit
`self` declared in the parameter *type* (`Html.fn() : Unit`), not at the call
site — so the calls inside it (`head`, `title`, `p`) resolve as the builder's
members by bare name. This is the same receiver mechanism used everywhere (the
element of `list.map`, a dataframe row, this builder — all `self`; there is no
implicit `it`); it is what makes the nested markup read like dedicated syntax,
and the block stays a bare `{ … }` because the context rides the type. See
[lambda receivers](../09-functions/06-closures-and-lambdas.md#lambda-receivers)
and [TIP-0010](../tips/0010-lambda-receivers-and-builder-dsls.md). A block that
instead needs a *named* parameter uses the `|x| { … }` form below;
`TODO(open):` pin the grammar for a receiver block that *also* binds a `|name|`.

### Block-bodied lambdas: naming the input

When the trailing block **names** its input it self-marks with `|x|` (and still
closes at its `}`); when it wants that input as the implicit receiver `self` it
marks with `\`:

```tel
items.keep |x| { x.price > EuroAmt(0) }     # named input — sole arg, () dropped
items.keep \ price > EuroAmt(0)             # same, self = the item (bare field)
fold(0, xs) |acc, x| { acc + x }            # two inputs stay named; seed in ()
```

The marker is what distinguishes a block with an input (a lambda) from a
param-less bare `{ … }`; a bare block never silently gains one, and there is no
implicit `it`.

To chain after a trailing block, the closing `}` ends it, so a `.method` on the
**same line** attaches to the call, not the block. A chain that spans lines
needs no wrapping — a leading `.` on the next line continues it (see
[Closures and Lambdas](../09-functions/06-closures-and-lambdas.md#how-far-the-body-reaches)):

```tel
xs.map \{ total }.filter(\ self > zero).sum()    # one line — } then .filter

orders                                            # multi-line — leading `.`
    .keep \ total > zero
    .map \ total
    .sorted()
```

## Splatting a bundle into a call

A call's argument list and a tuple have the **same shape** (see
[tuples as argument bundles](../05-types/04-tuples-and-arrays.md#tuples-as-argument-bundles)),
so a tuple can be **splatted** into a call with a leading `...` on the argument:
each member is matched to a parameter, position by position and name by name.

```tel
let b = (value, low, high)          # a 3-tuple
clamp(...b)                         # ≡ clamp(b.0, b.1, b.2)

let opts = (port = 9000, retries = 5)
connect("example.org", ...opts)     # ≡ connect("example.org", port = 9000, retries = 5)
```

Two rules keep this safe, and both are deliberate:

- **The splat is written, never inferred.** `f(b)` always passes `b` as **one**
  tuple-typed argument; only `f(...b)` spreads it. There is no auto-spreading and
  no auto-tupling — the single rule that keeps Tel clear of the Swift
  "tuples-as-argument-lists" regret
  ([tuples](../05-types/04-tuples-and-arrays.md#arguments-are-tuple-shaped-kept-distinct-by-the-fn-marker)).
  A reader never has to guess whether a tuple at a call site spreads.
- **The shape is statically known, so it desugars to an ordinary call.** Splat is
  **shape-checked sugar**: the compiler checks the bundle's members against the
  parameters and emits the same code as the spelled-out call — no runtime
  machinery. A generic wrapper may forward a bundle when its row matches the
  target *exactly* (`f(...args)` where `args: f::Args`); Tel adds no row
  *subtyping*, so where shapes differ you reshape explicitly or write a lambda
  (see [tuples-as-argument-bundles](../05-types/04-tuples-and-arrays.md#tuples-as-argument-bundles)).

The `...` here is in **argument position** and cannot be confused with the
`...rest` destructuring marker (which appears in a *binding*) or the `vararg`
parameter marker (which appears in a *signature*) — different positions, so the
token is unambiguous at a call site.

### Composition by splatting a return tuple

Because a function's return type is itself a tuple
([return values](../09-functions/03-return-values.md)), one call's result feeds
straight into the next when the shapes line up — positional returns by position,
named returns by name:

```tel
fn divmod(a: Int64, b: Int64) -> (Int64, Int64) { (a / b, a % b) }
fn show(q: Int64, r: Int64) -> Text { q & " rem " & r }

show(...divmod(17, 5))              # positional return → positional args

fn stats(xs: List[Int64]) -> (sum = ..., count = ...) { ... }
fn report(*, sum: Int64, count: Int64) -> Text { ... }

report(...stats(xs))                # named return → keyword args, by name
```

A **single** return value is not a one-tuple (`-> Int64` returns an `Int64`, only
`-> (Int64, Int64)` returns a row — the same grouping-vs-tuple rule as
[tuple literals](../05-types/04-tuples-and-arrays.md#literal-forms-grouping-vs-tuple)),
so splatting a single-value return is just an ordinary one-argument pass.

## Lazy arguments

An argument may be declared **lazy**: it is not evaluated at the call site, but
each time the function uses it. The classic motivation is logging — building an
expensive message string only when the log level is actually enabled:

```tel
log.debug("scores: " & expensive_dump(state))
# if `message` is a lazy parameter, expensive_dump runs only when debug is on
```

A lazy argument is closer to "pass the expression, evaluate on demand" than to
"pass the value." It underpins:

- cheap logging and assertion messages,
- the [fallback operator](11-fallback-operator.md), whose right side is only
  evaluated when needed,
- defining control-flow-like functions (an `if` whose branches are lazy
  arguments — see [Trailing block](../09-functions/02-parameters-and-arguments.md#trailing-block)).

TODO(open): much about lazy arguments is open:

- Surface marking. Candidates are a per-parameter keyword, a postfix symbol
  on the *argument* (`@` / `&`), and a `block`-typed parameter. It also asks
  whether a lazy argument is re-evaluated on every use or memoised on first use
  (the `\\` "cached lambda" idea).
- Whether laziness is the *parameter's* property (caller writes an ordinary
  expression) or the *caller's* — the cleaner default for readability is the
  parameter declaring it, so call sites stay plain.
- Lazy-argument ownership semantics: should an owned non-copy
  value passed lazily give an `FnOnce`-shaped thunk (used at most
  once) while a copy value gives an `FnMut` shape? Worth pinning down only if
  ownership-style semantics actually land — see
  [Mutability](../06-bindings-and-scope/02-mutability.md).
- A possible unification with `Lazy[T]` / future-like deferral and with
  conversion traits (`Into`-style), so a costly conversion happens only if the
  value is used. Speculative — re-justify against embedding before adopting.

### Lazy + interpolation = cheap logging

The lazy-argument feature exists mainly to make logging terse without
paying string-building cost on the happy path. The shape is:

```tel
log.debug(f"scores: ${expensive_dump(state)}")
```

The `f"..."` is an interpolating literal (see
[String Operations](05-string-operations.md)); `debug`'s `message` parameter
is lazy; so the call site reads as one argument and the interpolation runs
only when the log level emits.

## How it looks

```tel
fn price(an_order: Order, log: Log) -> EuroAmt {
    let total = sum_items(an_order)         # ordinary call
    log.debug("priced " & an_order.id & ": " & total)   # message may be lazy
    total
}
```

## See also

- [Function Declaration](../09-functions/01-function-declaration.md)
- [Closures and Lambdas](../09-functions/06-closures-and-lambdas.md) —
  trailing-lambda syntax.
- [Default and Named Arguments](../09-functions/04-default-and-named-arguments.md)
- [Method Syntax](../09-functions/08-method-syntax.md) — `x.f(y)` and `x:f(y)`.
- [Fallback Operator](11-fallback-operator.md)
