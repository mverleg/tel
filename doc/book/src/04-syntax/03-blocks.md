# Blocks

A **block** is a brace-delimited body: `{ ... }`. Tel uses a single bracket pair,
`{}`, for every kind of body — and uses it *only* for bodies. Grouping is `()`
and generics/indexing is `[]` (see
[Operators and Punctuation](../03-lexical-structure/06-operators-and-punctuation.md)).

## One brace, several roles

The same `{ ... }` serves, with the role decided by context:

- a **bare block** — a sequence of statements, evaluating to its last expression;
- a **declaration body** — the body of a function, struct, or enum;
- a **lambda** — a block used as an expression is an anonymous function (see
  [Expressions vs Statements](02-expressions-vs-statements.md)).

```tel
# declaration body
fn double(arg: Int64) -> Int64 { 2 * arg }

# bare block as an expression — yields its last expression
let label = {
    let n = count()
    "items: ${n}"
}

# lambda
xs.map({ x \ x * x })
```

This unification is deliberate. Because every body looks the same, a `{ ... }`
can also be passed as a trailing block argument, letting user-defined functions
read like built-in control structures (a deliberate Kotlin-style DSL idiom).

### Braces are for bodies, not data

One common convention reads `()` as grouping *code* (calls, expressions) and
`{}` as enclosing *data* (struct literals, maps) — the JSON / JavaScript
reading. **Tel does not follow it.** Here `{}` is for *bodies* (blocks,
lambdas, function and type bodies), `()` is for grouping and calls, and the
everyday data literals — lists and maps — get their own bracket, `[ … ]` (see
[Collection literals](../03-lexical-structure/05-literals.md#collection-literals----for-lists-and-maps)).

Tel takes the *familiar* body reading on
[familiarity](../02-philosophy/01-priorities.md) grounds: it keeps mainstream
code readable and keeps trailing-closure / DSL forms natural. The one
consequence worth stating plainly is that a JSON-style `{ "k": v }` is **not**
valid Tel — bare braces never mean "map" — so Tel map literals are deliberately
not JSON-syntax-compatible. Carrying JSON as *data* (in a string, parsed by a
function) is unaffected; the trade-off is detailed under
[Not JSON-literal-compatible](../03-lexical-structure/05-literals.md#not-json-literal-compatible).

Record literals still use braces — `Point { x = 1, y = 2 }` — because a record
literal reads as a *body* attached to a type name, not as a bare data blob. The
exact field spelling is covered in
[Records](../10-data-modelling/01-records.md).

## Blocks are expressions

Consistent with Tel's expression orientation, a block **evaluates to a value** —
its final expression. There is no statement/expression split at the block level:
an `if`, a `match`, or a bare `{ ... }` all yield values and can sit on the
right of a binding.

## Block structure is explicit, never layout

A block's extent is given by its braces, never by indentation —
[indentation is not significant](05-layout-rules.md). Two programs that differ
only in how their blocks are indented are the same program. Explicit braces are
chosen so Tel code survives being pasted into host config UIs and in-browser
editors, where leading whitespace is routinely mangled.

Blocks are delimited **only** by `{}`. There is no second way to open one:

- A colon (`:`) never opens a block. An earlier sketch floated `:` as a
  block-opener (colon-blocks, `fn(...): expr`); that idea is **abandoned**. The
  colon's one job is separating a name from its type (`name: Type`) — see
  [Operators and Punctuation](../03-lexical-structure/06-operators-and-punctuation.md).
- Significant-whitespace blocks (Python-style indentation as structure) are
  **abandoned** too, for the editor-robustness reason above and in
  [Layout Rules](05-layout-rules.md).

So `{ ... }` is the sole block delimiter, everywhere.

## Open questions

`TODO(open): empty and single-expression blocks.` Whether an empty `{}` is
legal as a block (it is clearly legal as an empty struct/enum body), and whether
a one-expression block needs braces at all in contexts like a single-line `if`.
Resolve alongside the control-flow syntax in [Control Flow](../08-control-flow/).

`TODO(open): bare-block scoping.` Whether a bare `{ ... }` introduces a new
binding scope (it almost certainly should). This is a
[Bindings and Scope](../06-bindings-and-scope/) question; the scoping
rule — writes default to declaring in the current scope, an explicit
keyword reaches an outer scope — is tracked there, not here.

TODO: review
