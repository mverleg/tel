# Style Guide

<!-- TODO: review -->

A short, opinionated set of style conventions Tel code is expected to follow.
The aim is the same as everywhere else in the language: *if it looks correct,
it probably is correct*, and *code is read more often than written*. The
[formatter](../18-tooling/06-formatter.md) enforces the mechanical parts;
this page collects the conventions a tool cannot derive from the AST alone.

## Names

### Identifier shape

- **Types** (records, unions, traits): `UpperCamelCase`.
- **Values, fields, functions, methods**: `snake_case`.
- **Constants**: `snake_case` like other values — *not* `SCREAMING_SNAKE`.
  A constant is a binding the compiler enforces as immutable; nothing about
  the call site needs reminding.
- **Modules and crates**: short, lowercase, `snake_case` if multi-word.

An *implicit* feature was considered here and rejected: giving every concrete
type a same-named interface that differs only in case — a struct `person`
automatically exposing an interface `Person` with its public signature, so a
concrete type could later be swapped for an abstraction without breaking
callers. It is rejected on two counts. It manufactures a trait per type "just
because"; when that abstraction is actually wanted, an *explicit* trait
expresses the intent better and only where it is needed. And the lowercase-type
spelling it leans on collides with ordinary value names (`let person = ...`),
forcing the type and its values to fight over one identifier.

So case never encodes concrete-vs-trait: a trait and a record are spelled the
same way. Every named type is `UpperCamelCase` — record, union, or trait alike
— and case is reserved for the one distinction that earns it, *type vs value*.

### Boolean names

A boolean — a `Bool` field, parameter, or function — reads as a yes/no
question. Prefer the prefixes mainstream readers expect:

```text
is_empty       has_children    can_retry       allows_anonymous
should_retry   needs_review    requires_auth
```

A bare verb (`enabled`) is acceptable for a field whose surrounding type
already supplies context; the prefix is mandatory for free-standing
functions and for parameters.

For a parameter that toggles a behaviour, prefer a *named-boolean enum* over
a bare `Bool` (see
[`../10-data-modelling/02-union-types.md`](../10-data-modelling/02-union-types.md)).
At the call site `cache(WriteThrough)` reads correctly; `cache(true)` does
not.

### Predicate functions

A function returning `Bool` is named for the question it answers, never for
the action it performs. `is_authorised(user, action)` not `check_auth(...)`.
A predicate that *also* mutates is a footgun, and
the name should make the predicate role unambiguous. If a function does
both, split it.

### Newtype and refined-type names

A refined or newtype wrapper takes the name of the *concept*, not of the
underlying primitive:

```tel
type EurAmt        = newtype Decimal      // not `Money` or `Decimal`
type UserId        = newtype Int64          // not `Int64` or `RowId`
type IsoCountry    = newtype Text
type StrategyVol   = newtype Real64
```

A name like `Money` is reserved for cases that genuinely abstract over
currency; if the value is *always* in EUR, the type is `EurAmt`. The
catalogue of catalogued bugs (see
[`../05-types/12-refined-types.md`](../05-types/12-refined-types.md#bugs-this-prevents))
is full of cases where a too-general name (`Money`, `Vol`, `Date`) hid the
distinction that turned out to matter.

### Named-parameter names are public; positional names are not

A parameter is declared **positional or named, not both** (see
[`../09-functions/04-default-and-named-arguments.md`](../09-functions/04-default-and-named-arguments.md)).
That decides whether its name is part of the public surface:

- A **named** parameter's name *is* the signature — callers pass it by that
  name, so renaming it is a breaking change. Treat it like a public field
  name and rename only through a deprecation window.
- A **positional** parameter's name is internal documentation; callers cannot
  use it, so it can be renamed freely.

### Don't repeat the type in the name

A field of type `EurAmt` is `price`, not `price_amt` or `price_eur`. The
type carries the unit; the name carries the *role*. The exception is when
the type genuinely is just `Int64` or `Text` and the name is the only
disambiguator — but in that case, prefer making a refined type and dropping
the suffix.

## Files and modules

- One topic per file. A file that has accumulated two unrelated concerns
  should split.
- File name matches the module name (`snake_case`). Rust's `mod` re-export
  complexity is deliberately rejected: the file *is* the
  module.
- Tests live next to the code they exercise, marked with `test`; no
  separate `tests/` mirror tree. See
  [`../14-testing/01-testing.md`](../14-testing/01-testing.md).

## Imports

- Imports go at the top of the file, grouped by *source* (standard library,
  external crates, this project), separated by a blank line within each
  group.
- Prefer importing the **type** or **function**, not its containing module.
  `use std.text.Text` rather than `use std.text` followed by `text.Text`
  everywhere.
- Rename on import only to resolve a *conflict*; aliasing for cosmetic
  reasons (`as t` for `Text`) is rejected — a reader who follows the
  import to the source then has to translate.

## Layout

- **Indent with spaces.** Pick a width (the formatter defaults to four)
  and stay with it. Indentation is *not* semantically significant — see
  [`../03-lexical-structure/08-whitespace-and-newlines.md`](../03-lexical-structure/08-whitespace-and-newlines.md) —
  but a project-wide consistent depth keeps diffs small.
- **Line length is a guideline, not a rule.** The
  [formatter](../18-tooling/06-formatter.md) does *not* reflow or wrap lines;
  a line that genuinely reads better longer is fine and not flagged.
- **Trailing commas** in multi-line lists, records, and call sites: keep
  them. Adding a new entry then changes one line, not two.
- **Blank lines** separate logical groups within a function. One blank line
  between top-level declarations.
- **Canonical spacing** is required, not optional taste: spaces around binary
  operators, a tight `.` (`order.total`), and no space just inside parentheses
  (`f(x)`). It is enforced and auto-fixed rather than parsed differently — see
  [`../03-lexical-structure/06-operators-and-punctuation.md`](../03-lexical-structure/06-operators-and-punctuation.md#spacing).

## Comments

- A comment explains *why*, not *what*. If a reader needs the comment to
  understand *what* the code does, the code is unclear and should be
  refactored — through a clearer name, a refined type, or an extracted
  function. Comments are a last resort.
- **Document the non-obvious, not the obvious.** A public declaration whose
  name and types already say everything needs **no** doc comment — a comment
  that just restates the signature is noise. Reserve doc comments for what the
  signature cannot show: invariants, units, failure modes, and *why*. There is
  deliberately no "doc comment on every `pub`" rule.
- A `TODO:` comment carries an owner or an issue link, not a wish. A
  `TODO` without a name is a `TODO` no one will pick up.

## Expressions and statements

- One assignment per line — chained `a = b = c` is rejected by the language
  (see [Mutability](../06-bindings-and-scope/02-mutability.md)), so this is a
  rule the compiler already enforces, not just a style preference.
- Prefer pattern matching to a chain of `if`s when destructuring a value.
- Prefer the [fallback operator](../07-expressions/11-fallback-operator.md)
  to a `match` whose only purpose is "or this default."
- Don't `let _ = expr` to silence a `Result`. Either handle it, propagate
  it with `?`, or write `expr.expect("...")` to abort loudly. Silently
  discarded errors are an antifeature
  ([`../02-philosophy/04-antifeatures.md`](../02-philosophy/04-antifeatures.md)).

## Function design

- A function does one thing. The name says what it does.
- A long list of **positional** parameters (more than ~5) is a hint that they
  belong in a record. Make the record; pass it. This is weaker for **keyword
  parameters with defaults** — a wide configuration surface is fine when each
  option is named at the call site and has a sensible default. The strongest
  signal is when the *same* cluster of arguments is threaded through several
  functions: that cluster wants to be a record so it travels as one value.
- A boolean parameter is a hint to split the function (one function per
  branch) or to use a named-boolean enum.
- **Don't fork a function into `_v2`.** A name like `process_v2` is the
  "I changed the API but couldn't break the old callers, so I copied it"
  pattern from churning ecosystems. Tel does not need it: a frozen language
  with named and defaulted arguments
  ([`../09-functions/04-default-and-named-arguments.md`](../09-functions/04-default-and-named-arguments.md))
  lets a function *evolve in place* — add a defaulted parameter, or rename via
  a deprecation window — instead of leaving two near-identical functions
  behind. A `_v2` suffix is a smell the linter flags.

## Errors

- Return `Result`-shaped values from anything a caller could reasonably
  expect to fail.
- Use `abort`/`panic` for *programmer mistakes* (an unreachable branch
  reached, a contract violation, a `todo` left in place) **and** for
  situations the program genuinely cannot recover from or chooses not to
  bother handling. Never for ordinary bad user input that a caller could
  reasonably want to handle.
- The **binary/library** split decides which way to lean. A *binary* may
  panic where it judges a failure unrecoverable and not worth the
  stability/recovery cost — that is the application author's call, informed by
  the external systems it talks to. A *library* should prefer returning
  `Result`, because it cannot know how much stability the end application
  needs.
- Name an error variant for the *condition* that caused it, not the action
  that detected it. `RegionUnknown`, not `LookupFailed`.

## When in doubt

- Prefer the shape mainstream-language readers (Python, Java, Kotlin, Rust,
  C#, JS, TypeScript) will recognise. *Familiarity over a "better" but
  novel surface* — see
  [`../02-philosophy/01-priorities.md`](../02-philosophy/01-priorities.md).
- Prefer the form that looks correct. *If it looks correct, it probably
  is correct.*
- If two forms are equally valid, pick the one the
  [formatter](../18-tooling/06-formatter.md) emits. There is no value in
  litigating taste differences that the formatter has already resolved.

## See also

- [Priorities](../02-philosophy/01-priorities.md) — the trade-offs this
  style serves.
- [Antifeatures](../02-philosophy/04-antifeatures.md) — the patterns this
  style avoids.
- [Formatter](../18-tooling/06-formatter.md) — the mechanical enforcement.
- [Linter](../18-tooling/07-linter.md) — style and bug-pattern checks.

TODO: review — first cut of this page. Specific items still open: exact
keyword for `uniq`/visibility, exact spelling of refined-type syntax —
update once those are settled.
