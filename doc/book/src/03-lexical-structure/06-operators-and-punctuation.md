# Operators and Punctuation

Tel's operators and punctuation are a **fixed, language-defined set**. A program
cannot define new operators, cannot change what an existing operator does at the
lexical level, and cannot alter precedence or associativity — see
[Antifeatures](../02-philosophy/04-antifeatures.md) and
[Precedence and Associativity](../04-syntax/04-precedence-and-associativity.md).
Existing operators can be *overloaded* for user types, but that changes meaning,
never lexing or parsing.

A fixed set keeps a Tel script readable wherever it is found, keeps the lexer
simple enough to re-implement across many hosts, and supports the *one good way*
priority.

## The three bracket pairs

Tel uses three bracket pairs, each with **one job**. This is a deliberate,
load-bearing decision from the design notes:

| Pair | Used for |
|------|----------|
| `{}` | **bodies** — function/struct/enum bodies, statement blocks, and lambda bodies (never the parameter list) |
| `()` | **grouping** in expressions, and call argument lists |
| `[]` | **generic type arguments and indexing** |

Two consequences worth stating:

- **`{}` is overloaded across one consistent idea: "a delimited body."** Whether
  a `{ ... }` is a struct body, a function body, a bare block, or a lambda body
  is fixed by the *preceding* token (a declaration head, or a `|...|` / `\` /
  `fn(...)` lambda opener) — never by what is inside the braces. Braces never
  hold a parameter list. So a bare `{ ... }` trailing a call is a **param-less**
  block (what a builder or control-flow DSL wants — `html { ... }`,
  `unless(cond) { ... }`); a trailing block that *takes* a parameter marks
  itself before the brace (`call \{ it.f }`, `call |x| { ... }`). The marker is
  the only thing that adds a parameter — a bare `{ ... }` never gains one
  silently.
- **`<>` is *not* used for generics.** Generic arguments go in `[]`, the same
  brackets as indexing — so `List[T]` for a type and `xs[i]` for an element.
  Dropping `<>` removes the classic ambiguity between "less-than" and "open
  generic bracket" and keeps the lexer free of unbounded lookahead.

```tel
# {} declares
fn double(arg: Int64) -> Int64 { 2 * arg }

# lambdas mark themselves with `|...|` or `\` — not bare braces
xs.map(|x| x * x)
xs.map \ self * self

# () groups and calls
score((a + b) * c)

# [] does generics, indexing, and list/map literals
let first: Item = items[0]
let pair: Pair[Int64, Str] = make_pair()
let xs = [1, 2, 3]                  # list literal
let m  = ["en" => "hi"]             # map literal — `=>` entries
```

> **Decision (rejected): JSON-compatible map literals.** Tel will **not** make a
> JSON object such as `{ "a": 1 }` valid Tel. It was tempting — an existing JSON
> config could then drop straight into a Tel file — but `{}` is already the
> delimiter for blocks and every kind of body, and a map literal would force the
> parser to decide *at the opening `{`* whether it is reading a map or a block.
> That cannot be done with bounded lookahead:
>
> - `{"` is not a reliable tell. `{"a"}` is not obviously a map (it could be a
>   block whose first expression is a string), and "starts with a string literal"
>   only covers *string*-keyed maps — enough for JSON, but an ugly, inconsistent
>   rule once map keys can be other types.
> - Any content-based disambiguation is unbounded lookahead, which Tel rejects on
>   principle — the lexer must stay fixed-lookahead.
>
> So the idea is abandoned: a map literal does **not** reuse bare `{}`, and Tel
> is therefore **not** a JSON superset.

**Decision (settled): map and list literals share `[ … ]`.** Maps do not get a
brace, a sigil, or a magic constructor — they live in the same brackets as
lists, distinguished by `key => value` entries. `[1, 2, 3]` is a list,
`["a" => 1]` is a map, `[]` is the empty list, and an empty map is `map_of()`.
The opener stays unambiguous (a `[` always begins an expression sequence; the
`=>` after the first element picks map over list with no backtracking). Other
collections use plain functions (`set_of`, `set`, `map_of`) — no JSON
superset, and one way to build each shape. Full spec in
[`05-literals.md`](05-literals.md#collection-literals----for-lists-and-maps) and
[`../10-data-modelling/09-collection-types.md`](../10-data-modelling/09-collection-types.md);
this gives up the config-with-logic "JSON config is valid Tel" goal.

## Operator set

The full operator list is not yet enumerated, but several properties are fixed:

- Arithmetic (`+ - * /` …), comparison, and boolean operators exist as a fixed
  set, with **no per-file precedence customisation**.
- **No cross-operator precedence.** Tel does not ask the reader to memorise how
  `+`, `*`, `/`, `%`, `==`, and `<` rank against each other. When **different**
  binary operators are mixed, explicit parentheses are **required** — the
  compiler does not silently apply a precedence table. Repeated use of the same
  operator, the additive group (`+` and `-` together), and boolean chains are
  the exceptions that need no parentheses. See
  [Precedence and Associativity](../04-syntax/04-precedence-and-associativity.md)
  for the full rule and rationale.
- **No `++` / `--`** increment/decrement operators, and no implicit conversions
  — consistent with [Antifeatures](../02-philosophy/04-antifeatures.md).
- **Whitespace around binary operators is required.** `a + b`, never `a+b` —
  one of several canonical-spacing rules collected under [Spacing](#spacing)
  below. The arithmetic detail lives in
  [Arithmetic and Numeric](../07-expressions/02-arithmetic-and-numeric.md).

`TODO(open): the full operator inventory.` The exact symbols, plus any
DSL-oriented extra overloadable operators, are not settled.
Track the inventory and the trait each binds to in
[Precedence and Associativity](../04-syntax/04-precedence-and-associativity.md)
and [Overloading and Dispatch](../09-functions/09-overloading-and-dispatch.md).

## Punctuation

- **`.`** — member access and method-call chaining: `x.f(y)`. Written **tight**,
  with no surrounding space (`order.total`, never `order . total`) — see
  [Spacing](#spacing). The `.` may begin
  a continuation line so a long method chain can be wrapped; this is the one
  place a newline is *not* a statement terminator. See
  [Whitespace and Newlines](08-whitespace-and-newlines.md).
- **`,`** — separates list elements, arguments, and parameters. A comma
  between arguments is **required**, because it keeps the grammar parseable
  with fixed lookahead.
- **`:`** — separates a name from its type in a binding or parameter
  (`name: Type`), chosen so the parser always knows whether it is reading a name
  or a type. This is the colon's **only** job. Earlier sketches floated `:`
  doing double duty as a block-opener (`fn(...): expr`, colon-blocks) and as a
  chain marker (`x:f(a)`); both are **abandoned**. Blocks are delimited only by
  `{}` (see [Blocks](../04-syntax/03-blocks.md)) and method chaining uses `.`
  alone. With `:` reserved for type annotations, the parser never has to guess
  between a type annotation, a block start, and a method call.

- **Statement separator (`;`).** A statement ends at a newline or, where two
  statements share a line, at a `;`. A statement must end with
  `;` and/or a newline — `;` is the explicit form and a newline is the
  default. A trailing `;` before `}` is allowed but not required; an entirely
  empty statement (`;` with nothing before it) is not a statement. See
  [Whitespace and Newlines](08-whitespace-and-newlines.md) and
  [Layout Rules](../04-syntax/05-layout-rules.md).

- **Line continuation.** A line may be continued onto the next when:
  - it ends in a symbol that cannot be the end of a statement (`+`, `(`, `=`,
    `&`, a comma, etc.),
  - the next line begins with `.` (method chaining),
  - or the line ends with an explicit `...` continuation marker.

  The continuation marker is a fallback for cases that are not covered by the
  other two rules — see [Whitespace and Newlines](08-whitespace-and-newlines.md).
  `TODO(open): `...` continuation marker.` Whether this survives, given the
  push toward trailing-comma–friendly literals, is open.
- **`\`** — introduces a self-receiver block: `\ body` runs with the block's
  single input as the receiver `self`, reached by bare name (`\ total` ≡
  `\ self.total`; `\ self * 2` for a scalar). There is no implicit `it`. It is the
  only expression-level use of `\` (string escapes are lexed inside string
  literals), so `\` in expression position unambiguously opens a lambda.
  See [Closures and Lambdas](../09-functions/06-closures-and-lambdas.md).

The earlier worry that `\` was terminator-less no longer applies: it inherits the
pipe form's termination — an expression body ends at the newline (or the
enclosing `)` / `,`), and `\{ ... }` is the block-bodied form.

## Spacing

Spacing in Tel is **canonical, not free**. Whitespace never changes how source
*parses* — `a+b` and `a + b` lex to the same three tokens (see
[Whitespace and Newlines](08-whitespace-and-newlines.md)) — but the
non-canonical form is still **not accepted**, so there is exactly one approved
way to space each construct:

- **Binary operators are surrounded by spaces.** `a + b`, `x > y`, `n = 1` —
  never `a+b`, `x>y`, `n=1`. Besides readability, this reserves the no-space
  forms for prefix/unary uses (`-x`) without ambiguity.
- **`.` is tight.** Member access and method chains carry no surrounding space:
  `order.total`, `xs.map(f)`, never `order . total`.
- **No space just inside parentheses.** A `(` is followed immediately by its
  content and a `)` is preceded immediately by it: `f(x)`, `(a + b)`, never
  `f( x )` or `( a + b )`. A space *follows* the `,` that separates items and
  never precedes it: `f(a, b)`.

### Understood, then rejected

Because the parse never depends on spacing, Tel can always *understand* a
mis-spaced line — and that is exactly what lets it react precisely instead of
failing to parse. It reports a clear, specific complaint ("missing space around
`+`") and the [formatter](../18-tooling/06-formatter.md) rewrites the line to
canonical form automatically. Mis-spacing is a fix-up, never a mystery
syntax error.

`TODO(open): error or warning?` Whether a spacing violation is a hard compile
error or a warning that the formatter silently repairs is unsettled. Lean: a
warning from the compiler plus an auto-fix in `fmt` / format-on-save, so
canonical spacing is restored without blocking a run, escalating to an error
only under a CI / `--strict` mode. Decide alongside the
[linter](../18-tooling/07-linter.md) severity model.

`TODO(open): mandatory trailing commas?` A thought worth recording: require a
trailing comma on *every* comma-separated list, so `[1, 2]` becomes `[1, 2,]`
and an argument list ends `f(a, b,)`. The win is diff-friendliness (adding an
entry touches one line, not two) and one uniform rule instead of "trailing comma
allowed multi-line, absent single-line". The cost is that the shortest literals
look slightly noisier. Today the
[style guide](../20-appendix/03-style-guide.md#layout) only *recommends*
trailing commas in multi-line lists; making them mandatory everywhere is a
stronger, formatter-enforceable rule. Lean: mandatory but auto-inserted by the
formatter, so the cost is purely visual. Interacts with the `...` continuation
marker above.

TODO: review
