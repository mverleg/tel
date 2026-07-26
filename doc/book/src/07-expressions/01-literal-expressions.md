# Literal Expressions

<!-- TODO: review -->

A *literal expression* is a value written directly in source — a number, a
string, a boolean, a unit. The lexical shape of each literal token is fixed in
[Literals](../03-lexical-structure/05-literals.md); this topic is about how
literals behave as **expressions** and what their type is.

## What

```tel
0          # an integer literal
3.14       # a Real64 literal
"hello"    # a string literal
true       # a boolean literal
```

Every literal is an expression. It has a type, it can sit anywhere an
expression of that type is expected, and it can be the body of a function or
the right-hand side of a binding:

```tel
let limit  = 100
let pi     = 3.14
let label  = "orders"
fn one() -> Int64 { 1 }
```

A literal carries **no implicit conversion**. `3` is not silently a `Float`
when the context wants one; `0` is not silently a `Bool`. The lack of implicit
coercion is one of Tel's [antifeatures](../02-philosophy/04-antifeatures.md) —
it serves "if it looks correct, it is correct."

## Number literals as expressions

A number literal's type is *inferred from context* where unambiguous, otherwise
it takes its default — see [Types](../05-types/02-primitive-types.md) for the
default-type rules. The widths are explicit and named: an un-annotated integer
literal defaults to `Int64` and a fractional one to `Real64`, and a script
reaches for a narrower width (or a domain newtype) by saying so.

```tel
let count = 42             # Int64 by default
let pi = 3.14              # Real64 by default
let byte: UInt8 = 42       # narrower width named explicitly
let total = EuroAmt(0)     # the literal `0` is the argument to a newtype constructor
```

Digit grouping with underscores is allowed: `1_000_000`, `0xFF_FF_FF`.

`TODO(open): the "10k"-style metric suffix.` A possible
shorthand like `10k` (= 10_000), `1M`, `1.5B` for large round numbers. This is
attractive for finance and game scripts but ambiguous with identifier-suffix
rules and trades clarity for terseness. Lean: not in 1.0; underscore grouping
covers the common case.

`TODO(open): numeric base prefixes and exponent notation.` Hex (`0xFF`), binary
(`0b1010`), octal (`0o17`), scientific (`1.2e3`) — settle which exist and
their lexical form alongside [Literals](../03-lexical-structure/05-literals.md).

## String literals as expressions

A string literal produces a value of Tel's string type (see
[Strings and Text](../05-types/03-strings-and-text.md)). The interesting
expression-level behaviour is **interpolation** — see
[String Operations](05-string-operations.md) for the operator side and
[Literals](../03-lexical-structure/05-literals.md) for the lexical form.

```tel
let n = 3
let label = "got ${n} items"
```

## Boolean and unit literals

```tel
let ok    = true
let nope  = false
let _     = ()        # the unit value
```

Booleans are values of `Bool`, not implicit numerics. The unit value (the
"there is no useful result" value) is produced when a block has no final
expression or a function has nothing useful to return.

`TODO(open): boolean keywords.` Whether Tel should ship
`Bool` at all, or push everyone toward purpose-named two-variant enums (a
`(Loaded | Unloaded)` instead of a `Bool` field called `is_loaded`). Lean: keep
`Bool` for genuinely boolean conditions, but make named two-variant enums easy
enough that "use a Bool" is rarely the cleanest option. Defer to
[Types](../05-types/02-primitive-types.md).

`TODO(open): unit literal spelling.` `()`, a named `Unit`, or `Nothing` — see
the open question in [Block Expressions](08-block-expressions.md). Settle in
[Types](../05-types/02-primitive-types.md).

## Tag / nullary-variant literals

A nullary union member can read as a literal in source — e.g.
`Color.Red`, or possibly a `:Red` short form. This is *data modelling* syntax,
not a separate literal kind; see
[Union Types](../10-data-modelling/02-union-types.md).

`TODO(open): the leading-"." rule for variants in patterns and expressions.`
Tel wants a way to always disambiguate "is this a fresh binding or an
existing variant?" — e.g. `match x { .None => ... }` makes the dot explicit so
`None` cannot be silently rebinding. Decide whether the dot is required in
patterns only, or in expression position too, alongside
[Match Expressions](../08-control-flow/02-match-expressions.md).

## Why

- **Literals are values** — uniform with the rest of the expression-oriented
  language. There is no "literal context" with special rules.
- **No implicit conversion** — a literal means exactly what it says, and a
  type mismatch surfaces at the source rather than as a later
  surprise.
- **Inferred-from-context typing** keeps small scripts uncluttered without
  hiding the actual type at the point of use.

## See also

- [Literals](../03-lexical-structure/05-literals.md) — the lexical shape.
- [Primitive Types](../05-types/02-primitive-types.md) — default types and
  the unit type.
- [Strings and Text](../05-types/03-strings-and-text.md)
- [String Operations](05-string-operations.md) — interpolation and concatenation.
