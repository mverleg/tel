# Parameters and Arguments

<!-- TODO: review -->

A *parameter* is a name in a function's signature; an *argument* is a value
supplied at a call. Tel keeps the basics conventional and concentrates its
design effort on the features that keep signatures **backwards-compatible** —
covered in their own topics.

## What

```tel
fn clamp(value: Int64, low: Int64, high: Int64) -> Int64 { ... }

clamp(7, 0, 10)        # positional arguments
```

- Each parameter has a name and a type.
- Arguments are matched to parameters positionally, or by name (see
  [Default and Named Arguments](04-default-and-named-arguments.md)).
- Parameters are **immutable bindings** inside the body, like any `let`. A
  parameter is not a place to write back to; a function communicates results
  through its [return value](03-return-values.md).

## Parameters are immutable; host values are never mutated

A parameter behaves as an immutable binding unless the function explicitly
needs otherwise. Two rules matter here:

- Tel code does not mutate a value it received as a parameter; it produces a
  new value (see [Mutability](../06-bindings-and-scope/02-mutability.md)).
- **Mutable arguments coming from the host are not accepted.** When a host
  passes data in, Tel treats it as immutable. Tel cannot police what host
  functions do, but it *can* guarantee Tel code does not mutate host-owned
  data — and that is enough to keep the boundary predictable.

## Argument evaluation

Arguments are evaluated left to right before the call, *except* arguments
declared lazy, which are evaluated on use inside the body — see
[Function Application](../07-expressions/06-function-application.md#lazy-arguments).

## Parameter sections (positional, vararg, keyword-only, block)

A signature is read as ordered **sections**. Every parameter belongs to exactly
one, and the sections always appear in this order:

```text
fn f( positional..., vararg name, *, keyword-only..., block ) -> T
```

1. **Positional** parameters come first. They are matched by position only: a
   caller cannot pass them by name, and they cannot carry a default. Because no
   call site names them, they may be **reordered or renamed** between releases
   without breaking callers.
2. An optional **`vararg`** parameter (see
   [Variadic Functions](05-variadic-functions.md)) collects any number of
   trailing positional arguments. It ends the positional section — nothing
   positional may follow it.
3. **Keyword-only** parameters follow. Each must be passed **by name** at the
   call site, and each **may** carry a default (but need not). Their order
   among themselves does not matter to callers. The keyword-only section is
   opened by a preceding `vararg` parameter, or — when there is no vararg — by a
   bare **`*`** marker sitting between commas.
4. An optional **block** comes last (see [Trailing block](#trailing-block)).

```tel
# vararg present — it opens the keyword-only section, so no `*` is needed
fn draw(x, y, vararg points, color = "black", size = 1) -> Canvas
draw(0, 0, p1, p2, color = "red")

# no vararg — `*` marks where keyword-only parameters begin
fn connect(host: Text, *, port: Int64 = 8080, retries: Int64 = 3) -> Connection
connect("example.org", port = 9000)
```

The split is deliberately **binary**: a parameter is *either* positional (by
position, never named, no default) *or* keyword-only (always named, may
default). Tel does **not** offer a Python-style "positional-or-keyword" middle
ground — keeping the two disjoint is exactly what makes positional parameters
safe to rename and keyword parameters safe to reorder.

`TODO(open): `*` as the keyword-only marker.` `*` is Python's choice and is
unambiguous here — a parameter list is a declaration context, so a bare `*`
between commas can never be a multiplication. Two alternatives were weighed and
rejected: a `keyword` marker (for symmetry with `vararg`, lost on familiarity),
and a **`|` separator** dividing the positional and keyword-only sections
(`fn draw(x, y | color = "black")`). The `|` reads nicely but **collides** with
`|` in a parameter's *type annotation* (`x: A | B`) and with a lambda default
(`= |a| …`), so it would not be unambiguous at the comma level — `*` avoids that
entirely. The `...`-ellipsis vararg spelling was likewise rejected; the `vararg`
keyword is settled (see [Variadic Functions](05-variadic-functions.md)) because
`...` clashes with the continuation and destructuring markers. Confirm `*`.

`TODO(open): redundant `*` after a vararg.` Because a `vararg` already opens the
keyword-only section, an explicit `*` after it is redundant; the lean is to
*disallow* it rather than require it for symmetry. Confirm.

### Trailing block

The final parameter may be a **block**: a single trailing
[closure](06-closures-and-lambdas.md) the caller supplies as a `{ ... }` body
after the argument parentheses.

```tel
fn repeat(n: Int64, block) -> Unit
repeat(3) { print("hi") }
```

- A block parameter is always **last**, after any keyword-only parameters.
- There is **at most one** — Tel has no multi-block call form.
- It may be **required or optional**; if optional, the call may omit the block.

`TODO(open): how the block parameter is spelled.` The sketch uses a magic
parameter named `block`. Open whether that is a reserved name, a modifier
keyword, or simply "the last parameter, typed as a function." Decide alongside
[Closures and Lambdas](06-closures-and-lambdas.md).

`TODO(open): parenthesis elision for a block-only call.` When (and whether) the
`()` may be dropped, and the leading-`.` chaining hazard that comes with it,
are deferred — out of scope here.

`TODO(open): which signature changes stay backwards-compatible.` Renaming or
reordering *within* a section is safe (positional names are invisible to
callers; keyword parameters match by name). *Moving* a parameter across the `*`
boundary is breaking. The precise rule lives with
[Overloading and Dispatch](09-overloading-and-dispatch.md#api-evolution-amp-function-references).

### No keyword-arguments dictionary

There is **no `**kwargs`** — no parameter that soaks up arbitrary extra named
arguments into a dictionary. A function's accepted arguments are exactly the
parameters in its signature; passing one that does not match is a compile error,
never silently collected. When a call genuinely needs an open-ended bag of
options, the Tel way is an **explicit config record** passed as one parameter —
the options then have names, types, and defaults the compiler checks, instead of
an untyped string-keyed map. Silently accepting and dropping an unknown argument
is the opposite of what static typing buys.

A wrapper that forwards a bundle of **statically-known shape** does so by
[splatting](../07-expressions/06-function-application.md#splatting-a-bundle-into-a-call)
it — `inner(...args)` — checked against the callee and desugared to an ordinary
call, no kwargs bag.

`TODO(open): forwarding.` Forwarding an *arbitrary, unknown*-shape argument
bundle is the larger row-polymorphism question (width/permutation/optional
subtyping over rows), not part of the monomorphic splat above; the exact
mechanism is left to that decision and to
[Overloading and Dispatch](09-overloading-and-dispatch.md).

## Constructors use the same parameter model

Constructing a record is not a separate mechanism: a record's constructor obeys
the **same** parameter rules as any function — positional fields, keyword fields
with defaults, the same ordering, and the same name-matching at the call site.
"A struct initialiser *is* a function call" is the guiding equivalence, so there
is one set of argument rules to learn, used for both calls and construction.

`TODO(open): construction surface.` The argument *semantics* are unified, but the
surface spelling is not yet reconciled: [Records](../10-data-modelling/01-records.md)
document **brace** construction (`Point { x = 1, y = 2 }`, with name-fill and
`*`-spread), while "init == function-call syntax" suggests a **paren** form
(`Point(1, 2)` / `Point(x = 1)`). Decide whether braces and parens are two
surfaces over one constructor, or one wins — the brace form's name-fill/spread
superpowers argue for keeping it; the unification argues for paren calls.
Resolve with [Records](../10-data-modelling/01-records.md) and
[Blocks](../04-syntax/03-blocks.md).

## Related topics

The features that make parameters expressive each have their own topic:

- [Default and Named Arguments](04-default-and-named-arguments.md) — optional
  parameters and call-by-name, the key to evolving a signature without breaking
  callers.
- [Variadic Functions](05-variadic-functions.md) — a parameter accepting any
  number of arguments.
- [Trailing block](#trailing-block) — supplying a single closure as the last
  argument.

## Why

- **Immutable parameters** keep a call site honest: passing a value to a
  function does not risk it coming back changed.
- **No host mutation** keeps the embedding boundary safe and reasoned-about —
  it dovetails with the assumption that there is no implicit cross-task sharing.
- The interesting design work is in *signature evolution*; see the linked
  topics.

## See also

- [Function Declaration](01-function-declaration.md)
- [Return Values](03-return-values.md)
- [Mutability](../06-bindings-and-scope/02-mutability.md)
