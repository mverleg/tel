# String Operations

<!-- TODO: review -->

A Tel script lives by moving text around — labels, log lines, query bodies,
templated outputs. This topic covers the *expression-level* operations on
strings: concatenation, interpolation, raw / DSL-style literals, and the
small handful of operators that act on `Text` values. The lexical shape of
string literals is in [Literals](../03-lexical-structure/05-literals.md); the
type and its methods are in [Strings and Text](../05-types/03-strings-and-text.md).

## Concatenation

Joining two strings is a binary operator:

```tel
let greeting = "hello, " & name
```

`TODO(open): the concatenation operator.` Both `&` and `+` are in play.
`+` collides with numeric addition (a long-standing source
of "concatenated 1 + '2' to get '12'" jokes); `&` is unambiguous. Lean: `&`,
because operator overloading already exists for `+` on numerics and reusing
it for text muddles the *one good way* principle. Confirm.

Like arithmetic, repeated `&` is allowed as a flat chain:

```tel
let line = prefix & " — " & body & " (" & status & ")"
```

Mixing `&` with arithmetic or comparison requires parens — same
[no-precedence rule](../04-syntax/04-precedence-and-associativity.md) as
everywhere else.

## Interpolation

Most concatenation should be **interpolation** — placing expressions inside a
literal:

```tel
let line = "order ${an_order.id}: ${total} EUR"
print("doubling number ${arg}")
```

Interpolation is the preferred form because it keeps the literal's shape
visible at a glance — *if it looks correct, it is correct*. A long `&`-chain
of fragments is almost always less readable than the interpolated equivalent.

`TODO(open): interpolation spelling.` Open points already tracked in
[Literals](../03-lexical-structure/05-literals.md):

- Delimiter (`${expr}`, `{expr}`, `$name` shorthand).
- Whether every string interpolates, or only ones with a prefix marker — the
  input floats an `f"..."` prefix (Python/Scala-style) so the *literal* opts
  in and ordinary strings are guaranteed to contain `$` and `{` as-is.
- How `{` and `}` are escaped inside an interpolated string.

### Format specifiers

An interpolation may carry a **format specifier** after the value, e.g.
`"amt: ${value:.2f}"`. The intended design is a **two-step** process that keeps
formatting cheap and almost-never-failing at runtime:

1. **Compile time.** A `const` function parses the specifier (`.2f`) against the
   *type* of the value and turns it into a concrete formatter — or raises a
   **compile error** if the specifier is invalid for that type (`.2f` on a
   `Text`, say). The format string is checked once, where it is written.
2. **Run time.** The pre-built formatter renders the actual value. Because the
   type and the specifier were already reconciled at compile time, this step is
   unlikely to fail — there is no re-parsing of the format and no type surprise.

This mirrors the language's general preference for moving validation to compile
time, and it means a malformed format is a build error rather than a runtime
one. `TODO(open): the exact specifier grammar and which types ship `const`
format parsers.`

### Lazy interpolation

String interpolation is **lazy where used as a function
argument**, so this:

```tel
logger.debug(f"scores: ${expensive_dump(state)}")
```

does **not** pay `expensive_dump`'s cost when `debug` is disabled. Two
mechanisms cooperate:

- The function's parameter is declared *lazy*
  ([Function Application](06-function-application.md#lazy-arguments)).
- An interpolated string is the natural thing to put in a lazy parameter,
  because the contained expressions are not evaluated until the lazy value is
  read.

`TODO(open): lazy interpolation mechanics.` Whether laziness lives on the
*parameter*, on the *literal* (so a special form like `f""` is always lazy),
or both is unsettled. Lean: laziness is a parameter property; interpolated
strings cooperate cleanly with it.

## Raw / DSL-style literals

There is a want for a way to embed snippets of *other languages* — SQL, JSON,
shell, regex, GLSL — with little or no escape noise. Two shapes are on the table:

```tel
# a raw / multi-line literal: no escape processing, no interpolation
let sample = r"""
{ "a": 1, "b": "two" }
"""

# a backtick-bounded "tagged" literal — a keyword names the embedded language
let query = sql `SELECT name FROM users WHERE id = ${id}`
let body  = json `{ "ok": true }`
```

The tagged form (the leading keyword names the embedded language so an IDE can
highlight and lint it, while the keyword is a function that does parameter
binding on the placeholders) is **deferred** — raw / multi-line strings carry
their weight for 1.0, but tagged DSL literals overlap metaprogramming and can be
added later. The sketch and open questions live in
[Deferred Features → Tagged DSL literals](../20-appendix/06-deferred-features.md#tagged-dsl-literals).

## Operator overview

| Op | Meaning |
|----|---------|
| `&` | concatenation (string + string) |
| `==`, `!=` | structural equality / inequality |
| `<`, `<=`, `>`, `>=` | lexicographic ordering |

Strings are values, immutable, and compare by content. There is no
identity-vs-content distinction to worry about. Comparison is lexicographic
by code unit — but see [Strings and Text](../05-types/03-strings-and-text.md)
for the locale / Unicode-collation subtleties; "human-correct" ordering needs
a collation-aware library call, not the bare `<`.

## Indexing and slicing

`TODO(open): direct indexing.` It is not yet decided whether `text[0]` returns
a code point, a byte, or is banned entirely. UTF-8 means there is no cheap
"character at offset N" operation — indexing by byte is fast but gives
nonsense between characters, and indexing by grapheme is slow. Lean: no
direct numeric indexing; slicing, splitting, and iteration go through methods
that name the unit (`.chars()`, `.bytes()`, `.split(...)`). Defer to
[Strings and Text](../05-types/03-strings-and-text.md).

## Why

- **Interpolation by default** beats concatenation for readability — that is
  literally what the priority list says about "looks correct = is correct."
- **Lazy interpolation** is the only way logging stays both cheap and terse.
  Without it, every log line either pays string-building cost or grows a
  lambda.
- **Tagged DSL literals** earn their place by removing escape-noise from
  glue code — Tel scripts are often glue between systems, so this is the
  common case, not the niche one. They must compose with interpolation and
  not undermine the static-typing story.
- **No `+` for concatenation** removes a famous source of mistakes and keeps
  one operator one meaning.

## See also

- [Literals](../03-lexical-structure/05-literals.md) — the lexical shape of
  string literals, including interpolation delimiters.
- [Literal Expressions](01-literal-expressions.md)
- [Strings and Text](../05-types/03-strings-and-text.md) — the type itself.
- [Function Application](06-function-application.md#lazy-arguments) — what
  makes lazy interpolation possible.
- [Precedence and Associativity](../04-syntax/04-precedence-and-associativity.md)
