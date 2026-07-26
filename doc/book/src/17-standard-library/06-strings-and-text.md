# Strings and Text

<!-- TODO: review -->

## What

`std` provides a text type and the operations a script needs to build,
inspect, and format text — including **string interpolation** and
**placeholder-based formatting**. Good, *lazy* string building is a
load-bearing feature: it is what makes logging, error messages, and DSL
output both cheap and readable.

## String formatting and interpolation

Tel supports two complementary ways to assemble text:

- **Interpolation** — embedding expressions directly in a string literal, so
  the common case reads naturally:

  ```tel
  let msg = "order ${order.id} total ${order.total}"
  ```

- **Placeholder formatting** — a template with positional or named holes,
  filled separately. This is the form logging uses (see
  [`logging` below](#lazy-logging-friendly-strings)):

  ```tel
  let msg = format("{} expiry {} strike {}", fund, expiry, strike)
  ```

`TODO(open): it is undecided whether interpolation is the default or
opt-in via a string prefix (e.g. an `f"..."`-style marker). Decide and
document; the readability priority leans toward making interpolation a
clearly-marked form rather than silently active in every literal.`

Formatting does **no** implicit type juggling: a value is formatted via its
display behaviour, never silently coerced number ↔ string. This follows *no
implicit conversions* in
[`../02-philosophy/04-antifeatures.md`](../02-philosophy/04-antifeatures.md).

## Lazy, logging-friendly strings

The argument to a log call, an assertion message, or an error should be cheap
to *not* build. Tel makes string-building arguments **lazy**: the expression
that constructs the message is evaluated only if the message is actually
needed (the log level is enabled, the assertion fails). This removes the
classic `if (log.isDebugEnabled())` guard.

Lazy string arguments are the foundation for several other features:

- The [logging facilities](12-concurrency-utilities.md) — lazy log arguments
  mean a disabled `debug` call costs almost nothing.
- The [editor's log folding](../18-tooling/09-editor-integration.md) — a
  formatting call with a known template can be rendered as an example
  message in the editor.

`TODO(open): lazy arguments are a general language mechanism (open question:
whether the same syntax could express Lazy[T] / lazy evaluation broadly, and
whether there is a postfix marker for it). The string-building case is the
clearest motivation; the general design belongs in a language chapter, with
this topic just relying on it.`

## Decimals and number formatting

Currency- and decimal-aware formatting is covered in
[`07-numerics-and-math.md`](07-numerics-and-math.md); text formatting just
calls into a value's display behaviour.

## No fixed-width character type

There is **no** single "character" type. The word "character" conflates two
genuinely different things, and lumping them into one
fixed-width type is the source of decades of mojibake bugs:

- **`Codepoint`** — a Unicode code point. Not always one byte, not always
  enough to be a *symbol*, and not stable across Unicode revisions for
  edge cases. Useful for low-level text processing.
- **`Grapheme`** — a user-perceived character: a base code point plus its
  combining marks. Variable length, effectively a tiny string. This is
  what users see and what cursor movement, length-for-display, and
  truncation should reason about.

A `Text` is logically a sequence of Unicode characters (the host picks the
storage encoding — UTF-8 is common; see
[Strings and Text](../05-types/03-strings-and-text.md)); iteration
exposes either `Codepoint` or `Grapheme` views explicitly — `.codepoints()`
and `.graphemes()` — so the programmer states which level they mean. There
is no implicit `Text[i]` indexing that pretends one of these is "the"
answer. `TODO(open): exact type names; whether `Grapheme` is its own type
or a `NonEmpty[List[Codepoint]]` newtype.`

## Escaping helpers

`std` ships escape and unescape functions for the formats a script keeps
running into: HTML/XML, shell arguments, SQL identifiers, file path
segments, URL components, JSON strings, regex literals. Each is a *named*
function — there is no overloaded `escape(string, "html")` form taking the
target as a string — so the editor can autocomplete the inventory and a
reader sees at a glance which escaping discipline applies.

Where it pays off, a **marker type** tracks that a
string has already been escaped for a particular sink, so accidental
double-escaping (or worse, accidental concatenation of raw and escaped
fragments) becomes a type error.

```tel
let raw   = "Bob & Carol"
let html  = escape_html(raw)              # Html (a refined Text)
let page  = "<p>${html}</p>"              # interpolation accepts Html as-is
let again = escape_html(html)             # compile error: already Html
```

`TODO(open): the full set of escape targets, and the choice between one
generic `Escaped[Kind]` wrapper or per-kind newtypes. Coordinate with the
refined-types story in
[`../02-philosophy/03-features.md`](../02-philosophy/03-features.md).`

## Text utilities

Beyond formatting, `std` ships the small helpers a script reaches for over
and over. Each is a free function (or method) with a single clear purpose:

- `trim`, `trim_leading`, `trim_trailing` — strip whitespace from one or
  both ends.
- `pad_left`, `pad_right`, `pad_center` — width-aligned padding for
  tabular text and CLI tools.
- `wrap(width)` — line-wrap respecting word boundaries.
- `split(on)` / `join(sep)` — the obvious pair, balanced so
  `split(on).join(on)` round-trips.
- `is_blank`, `non_blank` — emptiness checks that treat whitespace-only
  as blank, distinct from `is_empty`.
- `find`, `find_all`, `contains`, `starts_with`, `ends_with` — substring
  search, returning indices or matches.
- `replace`, `replace_first`, `replace_all` — with an explicit `_all`
  suffix to avoid the "did this only replace once?" ambiguity.
- `to_upper`, `to_lower`, `title_case` — locale-sensitive variants live
  in [`16-internationalisation.md`](16-internationalisation.md).
- `lines()` — iterate over lines without keeping the source allocated.

`TODO(open): exact names, especially where the prelude leans `keep`/`drop`
elsewhere; coordinate with the iteration chapter's naming notes.`

## URL parsing

`std` parses URLs into structured values (scheme, authority, path, query,
fragment) and renders them back. The reference is the WHATWG URL
specification — it is the post-RFC-3986 endpoint that has stopped moving,
and matches what browsers and modern HTTP stacks do. Pure data; no
capability needed (using a URL to *fetch* something is the network
capability, see [Networking](11-networking.md)).

```tel
let u = Url.parse("https://example.com/a/b?q=1#top")?
u.scheme == "https"
u.host == "example.com"
u.query.get("q") == Some("1")
let v = u.with_path("/a/c").to_string()
```

`TODO(open): query-parameter handling — exposed as an ordered list of
pairs, an ordered map, or both? Multi-valued keys are the awkward case.`

## File path parsing

Path *manipulation* — splitting into segments, joining, normalising,
extracting extensions — is pure data and belongs in `std`. Path
*manipulation* never touches the filesystem; reading or writing a path
needs a filesystem capability ([I/O and Filesystem](08-io-and-filesystem.md)).

```tel
let p = Path.parse("src/lib/util.tel")
p.parent() == Some(Path.parse("src/lib"))
p.extension() == Some("tel")
p.join("README.md")
```

`std` exposes both **logical** paths (slash-separated, host-independent,
the default for URLs and config) and **native** paths (host-shaped,
with Windows backslash and drive-letter quirks where applicable). A
script that builds a path to hand to a filesystem capability uses
native; one that records a path in data uses logical.

`TODO(open): name of the native vs logical types; whether Unicode
normalisation is applied to path comparisons (NFC by default, with an
escape hatch).`

## Regex

The library includes a regex engine — a *core
library* (written in Tel on top of primitives) rather than a per-target
implementation. The motivation is portability: every host gets the same
regex semantics without each runtime shipping its own engine. `TODO(open):
the regex flavour is not committed — PCRE-shaped vs RE2-shaped
(no backreferences, linear-time matching). Tel's safety priority leans
RE2-style; decide and document.`

## Text representation notes

Implementation ideas — short-string optimisation, storing a length-only
header when capacity is always the next power of two, keeping a
stack-resident prefix or hash to speed comparison in databases, interning
immutable strings or interning-on-second-encounter, separate "knot" /
mutable-string types like Rust's `ustr` — are implementation choices, not
user-visible behaviour. They live in `impl-notes/`, not here. The
user-facing rule is simply that `Text` is immutable; mutation goes
through its mutable form `!Text` (the `!T` sigil — see the settled
[mutability model](../06-bindings-and-scope/02-mutability.md) and
[TIP-0001](../tips/0001-mutability-and-borrowing.md)).

## See also

- [Numerics and Math](07-numerics-and-math.md)
- [Concurrency Utilities](12-concurrency-utilities.md) — logging
- [Internationalisation and Formatting](16-internationalisation.md)
- [Markup DSL](../19-use-cases/03-markup-dsl.md)
