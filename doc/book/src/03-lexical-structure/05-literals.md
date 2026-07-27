# Literals

A **literal** is a constant value written directly in source. This page covers
the *lexical* literals — numbers and strings — whose shape the design notes
touch. Compound literals (arrays, records) are syntax, not tokens, and are
covered in [Expressions](../07-expressions/) and
[Data Modelling](../10-data-modelling/).

## Number literals

Numbers are written in the conservative, familiar way:

```tel
0        42        3.14        2.72
1_000_000             # underscores group digits, ignored by the value
```

### Digit-group underscores

Digit-group underscores are **optional**, but **if used they must follow
3-digit grouping** — `1_000_000`, never `10_00` or `1_0000`. The compiler
enforces this: an underscore that does not sit on a thousands boundary is a
compile error, not a quietly-accepted style choice.

```tel
1_000          # ok
1_000_000      # ok
12_345_678     # ok
10_00          # error — not 3-digit grouped
1_0000         # error — not 3-digit grouped
1000_000       # error — first group is not 3 digits
```

Underscores make magnitudes readable at a glance — a *readability over
writability* win — and forcing a single grouping convention keeps that benefit
from turning into noise. This deliberately **ignores the non-3-digit grouping
conventions of some cultures** (e.g. the South Asian `10_00_000` lakh/crore
grouping) in favour of one consistent, machine-checkable rule across all Tel
source.

An un-suffixed integer literal defaults to `Int64` and a fractional one to
`Real64`; a narrower or wider width is named explicitly (`let b: UInt8 = 42`).
See [Types](../05-types/02-primitive-types.md) for the default-type rules.

`TODO(open): numeric literal detail.` Still open: exponent notation, and
whether literals may carry a type suffix at all (`42u8`-style) given that the
type is normally inferred from context or named on the binding. Settle in
[Types](../05-types/). Note also: there is **no implicit numeric widening or
quiet overflow** (see [antifeatures](../02-philosophy/04-antifeatures.md)), so a
literal's type must be unambiguous, not silently coerced.

`TODO(open): metric-suffix shorthand (`10k`, `1M`).` One idea is to let
`10k` mean `10_000`, `1M` mean `1_000_000`, etc., for game and finance
scripts. Tempting for readability of round numbers; conflicts with the rule
that an identifier may follow a digit and complicates the lexer. Lean: not in
1.0 — underscore grouping (`10_000`) covers most cases and stays unambiguous.

### No base prefixes — non-decimal literals go through a const function

Tel has **no base prefixes or lexical shortcuts** for non-decimal literals:
there is no `0x`, no `0b`, no `0o`. The only number tokens the lexer recognises
are plain decimal integers and decimals.

{{#spec NUMBER_LITERALS_DECIMAL_ONLY}}

Instead, a non-decimal value is written by calling a **const function** that
parses a string at compile time:

```tel
hex('12ab')        # hexadecimal
bin('1010_0110')   # binary
oct('755')         # octal
```

The rationale is *fewer lexical special cases*: the number grammar stays tiny
(decimal only), the lexer needs no per-base digit rules, and a base is named by
an ordinary identifier rather than a cryptic sigil. Because these are const
functions evaluated at compile time, there is no runtime parsing cost, and the
set is open — a host or library can offer additional radix parsers the same way.
(The argument is a string literal, so the same digit-grouping freedom of strings
applies; underscores inside it are the parser's concern, not the number-literal
rule above.)

This also dissolves a lexer hazard: `12bA` could otherwise
read as *the hex-like literal 11 in base 12* or *the product of `12` and
identifier `bA`*. With no base prefixes, `12bA` is never a number; **a numeric
literal is not allowed to abut an identifier**, so the multiplication is written
`12 * bA` (the bare two-token form `12 bA` isn't valid either — see
[rejected `A B` juxtaposition](../20-appendix/05-design-history-and-changelog.md)).
This keeps the *no implicit juxtaposition* parser rule clean.

`TODO(open): bin/oct const-function names.` `hex(...)` is the named example; the
exact spellings of the binary and octal parsers (`bin`/`oct` vs `binary`/`octal`
vs a single `radix(16, '...')`) are not pinned down.

## String literals

Strings are double-quoted. Two features stand out.

### Interpolation

Tel strings support **interpolation** — embedding an expression inside a string
literal:

```tel
print("doubling number ${arg}")
print("${order.total} exceeds limit ${limit}")
```

Interpolation is justified twice over: it is a core ergonomic for
data-transformation code, and good *lazy* interpolation is what lets logging and
telemetry stay terse without paying string-building cost on the happy path.

`TODO(open): interpolation spelling and laziness.` Open points:

- Exact delimiter — `${expr}`, `{expr}`, or a `$name` short form for the bare
  identifier case. A string *prefix* (an `f"..."`-style marker) to opt a literal
  into interpolation, versus making every string interpolating, is also on the
  table. Decide whether interpolation is always on or prefix-gated.
- Whether interpolation arguments are evaluated **lazily**, so an interpolated
  string handed to a logger costs nothing if the log level discards it. The
  notes want this; it depends on the lambda / lazy-argument design — see
  [Functions](../09-functions/).
- How `{` and `}` are escaped inside an interpolating string, given `{}` is also
  the block / lambda delimiter.

### Multi-line strings

A string literal may span multiple lines. The delimiter is **triple quotes**,
`"""..."""`:

```tel
let doc = """
    line one
    line two
    """
```

Multi-line strings are the natural home for the longer text a glue-style Tel
script carries — a SQL query, a JSON body, a template — without escaping every
embedded newline or quote.

Inside a `"""..."""` literal, **layout is significant**: the line breaks are
part of the value. This is the one place Tel's "indentation carries no meaning"
rule does not apply (see [Layout Rules](../04-syntax/05-layout-rules.md)).

`TODO(open): multi-line string details. (1) Indentation/margin rule — how is the
script's own leading indentation stripped so it does not leak into the value
(closing-delimiter column, an explicit margin marker, or similar)? (2) Is there
a raw variant with no escape processing and no interpolation, and how is it
spelled? (3) Interpolation inside `"""..."""` follows the same rules as a
single-line string.`

### Tagged / DSL string literals (backtick form)

A **tagged** literal — `sql`/`json`/`regex` naming an embedded language, with
backticks as the delimiter and the tag a function the lexer hands the contents
to — is **deferred**. It overlaps the [metaprogramming](../02-philosophy/04-antifeatures.md)
Tel is wary of and ranks below raw and multi-line literals, which cover most of
the need. The sketch and design points live in
[Deferred Features → Tagged DSL literals](../20-appendix/06-deferred-features.md#tagged-dsl-literals).

## Collection literals — `[ … ]` for lists and maps

Compound literals are *syntax*, not tokens; the construction surface lives in
[Data Modelling](../10-data-modelling/09-collection-types.md). The lexical
shape, though, belongs here, because it settles what `[ … ]` means and rules a
competing shape out.

Tel has **one bracket literal**, `[ … ]`, for its two everyday collections,
distinguished only by what is inside:

```tel
let xs = [1, 2, 3]                          # List[Int64]
let tr = ["en" => "hi", "fr" => "salut"]    # Map[Text, Text]
let by_id = [u.id => u, v.id => v]          # keys are expressions, not just literals
```

- A bare sequence of expressions is a **list**.
- Entries of the form `key => value` make it a **map**.
- Both are **immutable** — the default for every collection (see
  [Collection Types](../10-data-modelling/09-collection-types.md)). Mutable
  construction goes through the mutable form (`!List`, `!Map`).
- Keys and values are **arbitrary expressions**. `[compute() => f(x)]` is fine;
  keys are not required to be literals.

### Why this needs no lookahead

The map-vs-block problem that sank JSON literals (below) does **not** arise for
`[ … ]`, because `[` has only one interpretation: it opens a bracketed sequence
of expressions. The parser does one uniform thing — see `[`, parse the first
element as an expression (correct for *both* a list and a map), then read the
single token after it:

- `=>` → this is a map, and that expression was a key; parse the value.
- `,` or `]` → this is a list.

No backtracking, no re-lexing — the decision is one token past the first
element. This is the same shape as Rust telling `[a; n]` from `[a, b]`. The
`=>` separator is reused from match arms (`pattern => result`); inside `[ … ]`
there is no `match`, so the context is unambiguous, and "key `=>` value" reads
consistently with "pattern `=>` result".

### The empty case

`[]` is the **empty list** — always, with no type-context guessing. An empty
*map* cannot be spelled with brackets (nothing inside distinguishes the two), so
it is written `map_of()`; see
[Collection Types](../10-data-modelling/09-collection-types.md). This falls out
of the pair-based `map_of` form rather than needing a dedicated `empty_map()`.

### Only lists and maps get a literal

Brackets are *not* extended to other containers. Sets and the rest are built
with ordinary functions — **one way to do things**, no compiler-magic
constructor:

```tel
let s  = set_of(1, 2, 3)               # Set[Int64] from elements
let s2 = set([1, 2, 3])                # Set[Int64] from a list
let m  = map_of([k1, v1], [k2, v2])    # Map from [key, value] pairs
let e  = map_of()                      # empty Map
```

### Not JSON-literal-compatible

A JSON-style `{ "k": v, ... }` is still not valid Tel: `{...}` delimits blocks,
records, and lambda bodies, and telling a block-opening brace from a map-opening
one would need exactly the multi-token lookahead Tel refuses. Maps live in
`[ … ]` instead, so bare braces never have to mean "map". Carrying JSON as
*data* is fine — a snippet lives in a string literal (e.g. a `json` tag) and is
parsed by a function, not by the Tel grammar.

### Conditional elements (maybe-idea)

`TODO(open)/maybe:` Inside `[ … ]` — list or map — an element may carry an
`if c` suffix; the element is included only when `c` holds:

```tel
let xs = [1 if is_monday(), 2, 3]          # [2, 3] except on Mondays
let m  = [k1 => v1 if cond, k2 => v2]      # k1 entry present only when cond
```

A small comprehension-like sugar, available **only** inside `[ … ]`. It is not
a ternary: to *choose a value* use an ordinary `if`/`else`, which works because
it is just an expression:

```tel
[if is_monday() { 1 } else { 4 }]          # always one element; value depends on the day
```

Note the suffix and the expression-`if` read in opposite orders
(`value if cond` vs `if cond { value }`) — accepted as the price of keeping the
ternary non-magic. Unproven; revisit before committing.

TODO: review
