# Tokens

A Tel source file is first turned into a flat stream of **tokens** by the lexer,
before any grammar is applied. This page lists the token kinds; the detail of
each lives in its own page.

## Token kinds

- **Identifiers** — names of bindings, functions, types, fields, parameters.
  See [Identifiers](03-identifiers.md).
- **Keywords** — a fixed, reserved set of words. See [Keywords](04-keywords.md).
- **Literals** — numbers, strings, and other inline constant values. See
  [Literals](05-literals.md).
- **Operators and punctuation** — the fixed symbol set: arithmetic and logical
  operators, the three bracket pairs, the statement separator, and so on. See
  [Operators and Punctuation](06-operators-and-punctuation.md).
- **Comments** — discarded before parsing. See [Comments](07-comments.md).
- **Whitespace and newlines** — mostly insignificant, but a newline can act as a
  statement terminator. See [Whitespace and Newlines](08-whitespace-and-newlines.md).

## Why a clean token stream matters

Tel commits to **fast, predictable compilation** with clear errors, and to being
re-implementable across many host languages. Both push the lexer toward being
simple and unsurprising:

- The lexer needs only **fixed, bounded lookahead** — no unbounded backtracking.
- Tokenisation does not depend on type information or on what has been parsed so
  far. The one notable exception is the interaction between a newline and a
  following `.` in a method chain — see
  [Whitespace and Newlines](08-whitespace-and-newlines.md) — which the lexer
  resolves with a small, fixed rule rather than by consulting the parser.

A simple lexer is part of the deal that lets the same Tel script be compiled by
many independently-maintained host implementations and still behave identically.

## Open questions

`TODO(open): newline-sensitive tokenisation.` The most-recent design notes hit a
real lexing problem: a newline usually ends a statement, but a line that ends
with a trailing lambda or is continued by a leading `.` must *not* end there.
One resolution combines `\n` and a following `.` into a single
token so fixed lookahead still works. Whether that specific trick survives
depends on the final lambda and chaining syntax — see
[Whitespace and Newlines](08-whitespace-and-newlines.md).

TODO: review
