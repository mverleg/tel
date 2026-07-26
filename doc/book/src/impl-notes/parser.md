# Parser (notes)

Implementation scratchpad for the Tel parser. Not user-facing; the grammar
*rules* this strategy relies on are specified in
[`../03-lexical-structure/`](../03-lexical-structure/08-whitespace-and-newlines.md)
and [`../04-syntax/`](../04-syntax/01-grammar-notation.md).

## Recommended structure

A hand-written recursive-descent core with a Pratt (precedence-climbing) sub-parser
for expressions:

- `parse_module()` → **loop** over top-level items until EOF.
- `parse_block()` → **loop** over statements until the closing `}`.
- `parse_statement()` → **dispatch** on the leading token (`let`, `if`, `for`,
  `while`, a declaration keyword, else fall through to an expression statement).
- `parse_expression()` → **Pratt**.

Keep genuine recursion confined to the constructs that are actually nested:

- plain text / atoms,
- `( parenthesized expr )`,
- blocks `{ … }`,
- `if` / `else`,
- `match` arms,
- lambdas.

Everything else is a loop or a flat dispatch, not recursion. Add a **nesting-depth
check** (a counter incremented on each recursive descent, capped at a generous
limit) so pathological input cannot blow the native stack — return a clean
"too deeply nested" diagnostic instead.

## Why this is easy in Tel specifically

The language was shaped so a fixed-lookahead parser suffices; several documented
decisions pay off directly here:

- **The Pratt table is nearly trivial.** Tel has *no cross-operator precedence*
  (see [`../04-syntax/04-precedence-and-associativity.md`](../04-syntax/04-precedence-and-associativity.md)):
  mixing different binary operators requires explicit parens. So the only binding
  powers the expression parser needs are for same-operator chains and the
  additive (`+`/`-`) group; any other operator meeting a different one is a parse
  error, not a precedence decision. There is no long precedence ladder to encode.
- **Statement boundaries are mechanical.** A newline terminates a statement
  unless a bracket is still open; the lexer tracks bracket depth and nothing else
  (see [`../03-lexical-structure/08-whitespace-and-newlines.md`](../03-lexical-structure/08-whitespace-and-newlines.md)).
  `parse_statement()` does not need layout/indentation logic.
- **Blocks vs lambdas need no lookahead.** A lambda always wears an opening
  marker (`|x|`, `\`, `fn`); a bare `{` is therefore unambiguously a block body
  (see [`../09-functions/06-closures-and-lambdas.md`](../09-functions/06-closures-and-lambdas.md)
  and [`../04-syntax/02-expressions-vs-statements.md`](../04-syntax/02-expressions-vs-statements.md)).
  The dispatch in `parse_expression()` decides lambda-vs-block at the first token.
- **Control-flow heads stop the condition at the first `{`.** `if`/`while`/`for`
  read the head as an expression up to the opening brace of the body (see
  [`../08-control-flow/01-if-expressions.md`](../08-control-flow/01-if-expressions.md)),
  so no `then`/`do` keyword and no backtracking.
- **`[ … ]` decides list-vs-map one token past the first element** (`=>` ⇒ map,
  `,`/`]` ⇒ list) — see
  [`../03-lexical-structure/05-literals.md`](../03-lexical-structure/05-literals.md).

## Open / to-confirm

- Whether the lexer also enforces the canonical-spacing rule (spaces around
  binary operators, tight `.`, no space inside `()`) or whether that is a
  separate fmt/lint pass over the token stream — see
  [`../03-lexical-structure/06-operators-and-punctuation.md`](../03-lexical-structure/06-operators-and-punctuation.md#spacing).
- Exact nesting-depth cap and whether it is configurable per host.
- Error-recovery strategy (panic-mode resync to the next statement/`}` vs more
  precise recovery) for IDE use.
