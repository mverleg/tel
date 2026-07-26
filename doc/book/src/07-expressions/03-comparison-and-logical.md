# Comparison and Logical Operators

<!-- TODO: review -->

Comparison operators ask "how do these two values relate?" and yield a
`Bool`. Logical operators combine `Bool` values with short-circuiting. A
comparable type needs only two primitives implemented, and the operator
surface stays deliberately conservative.

## Comparison operators

| Op | Meaning |
|----|---------|
| `==` | structural equality |
| `!=` | inequality (the negation of `==`) |
| `<`  | strictly less than |
| `<=` | less than or equal |
| `>`  | strictly greater than |
| `>=` | greater than or equal |

```tel
order.total > EuroAmt(0)
name == "alice"
count != 0
```

Both sides must have the **same comparable type**. There is no implicit
coercion: `42 == "42"` is a type error, not `false`. Equality across
unrelated types is not "always false" — it is "doesn't type-check."

`!=` is the inequality spelling (`?=` for equality was considered and rejected
as confusing); `==` and `!=` are the pair.

## Only two operators need implementing

A type that wants to be ordered only has to define **two** primitive
comparisons; the others are derived:

- `a == b` (equality)
- `a < b`  (strict less-than)

From those, Tel derives:

- `a != b` is `not (a == b)`
- `a <= b` is `(a < b) or (a == b)`
- `a >  b` is `b < a`
- `a >= b` is `not (a < b)`

And the implementation is expected to honour the usual identities:

- `a == b` implies `b == a` (symmetry)
- `a == b` implies `a <= b and b <= a`
- `a == b` implies not (`a < b` or `b < a`)

`TODO(open): partial vs total ordering.` Two comparisons suffice for **total**
ordering. Partial ordering (where some pairs are incomparable, e.g. NaN or
sets under subset-of) needs a third bit. Defer the
exact trait shape to [Traits or Interfaces](../10-data-modelling/03-traits-or-interfaces.md).

`TODO(open): self-checking comparison traits.` One option is for the ordering
trait to require sample instances against which the derived identities are
tested — a convenient sanity check, not a soundness guarantee. Whether this
lands in stdlib tooling or in the language is open.

## Comparison chaining reads mathematically

A chain of the **same kind** of comparison may be written without parentheses,
and it reads the way mathematics does: each *adjacent* pair is compared, and the
whole chain is true only if **every** adjacent comparison holds.

```tel
# all of: a < b, AND b < c
a < b < c

# all of: lo <= x, AND x <= hi  — the natural "x is in range" idiom
lo <= x <= hi

# all of: a == b, AND b == c
a == b == c
```

So `a < b < c` is exactly `(a < b) and (b < c)` — note `b` is mentioned once in
the source but evaluated once and compared on both sides. This is *not*
left-to-right folding of a `Bool` back into the next comparison (which would be
the nonsensical `(a < b) < c`); a comparison chain is a dedicated form that
expands to the conjunction of its adjacent pairs.

```tel
# x is strictly inside (0, 100)
if 0 < x < 100 {
    accept(x)
}
```

`TODO(open): are mixed-direction chains like `a < b > c` allowed or rejected?`
A same-direction chain (`a < b < c`, `a <= b <= c`) and an all-equality chain
(`a == b == c`) are clearly in. Whether a *mixed-direction* chain such as
`a < b > c` or `a <= b < c` is permitted — and if so whether it still expands to
the conjunction of adjacent pairs — is open. Lean: require a single direction
(all `<`/`<=`, or all `>`/`>=`, or all `==`) and reject mixed-direction chains;
write the awkward case out with explicit `and`.

## Mixing different operators needs parentheses

Outside same-kind comparison chains and boolean chains, mixing **different**
operators follows the general
[parenthesise-on-mix rule](../04-syntax/04-precedence-and-associativity.md):

```tel
# REJECTED — does `and` bind tighter than `or`?  (mixing and/or)
a > 0 and b < 10 or c == 0

# ACCEPTED
((a > 0) and (b < 10)) or (c == 0)

# ACCEPTED — a flat chain of the SAME boolean operator needs no parens
(a > 0) and (b < 10) and (c == 0)
```

A flat chain of one boolean operator (`and` only, or `or` only) is allowed, as
is a same-kind comparison chain; mixing `and` with `or`, or a comparison with an
arithmetic operator, requires parentheses. The reader should not have to recall
how `and`, `or`, `+`, `==`, and `<` rank against one another — forced parens
make the intended grouping visible.

## Logical operators

Tel uses **word** logical operators rather than the punctuation forms:

| Op | Meaning |
|----|---------|
| `and` | logical AND, short-circuit |
| `or`  | logical OR, short-circuit |
| `not` | logical NOT |

```tel
if (an_order.total > EuroAmt(0)) and (not an_order.is_void) {
    accept(an_order)
}
```

Tel uses the **word** forms `and` / `or` / `not`, never the punctuation
`&&` / `||` / `!`. Word operators pair well with the parenthesise-on-mix rule
(grouping reads better around words than around chained punctuation), and they
keep `&` / `|` / `!` free for other roles (bitwise, type-union, fallback
markers). This is settled — there is no punctuation spelling for the logical
operators.

Both `and` and `or` **short-circuit**: the right operand is evaluated only if
the left does not already decide the answer. This is the only place an
operator skips evaluation by default; everywhere else use a lazy parameter or
the [fallback operator](11-fallback-operator.md).

## No truthy / falsy coercion

The operand of `and`, `or`, `not`, or an `if` condition must be a `Bool`. Tel
has no truthy/falsy coercion — `0`, `""`, `[]`, and `none` are **not**
shorthand for `false`. This is one of Tel's
[antifeatures](../02-philosophy/04-antifeatures.md). If you want
"present-or-not," use the explicit [`??` fallback](11-fallback-operator.md) on
an `Option`; if you want "non-empty," ask `.is_empty` explicitly.

```tel
# REJECTED — `count` is an Int64, not a Bool
if count { ... }

# ACCEPTED — the condition is a Bool
if count > 0 { ... }
```

## Equality and `Option`

Equality on `Option`-shaped values compares both the presence and the
contained value: `Some(3) == Some(3)` is `true`, `Some(3) == None` is `false`.
Comparison operators that involve `Option`-shaped values have **no extra
rules — they compare like any other value**. The "missing" question is handled
by [`??`](11-fallback-operator.md) and by explicit `match`.

## Operator sections (Haskell-style)

Allowing operators to be used as functions — `map (+1) xs`, `fold (+) 0 xs` —
is terse, but conflicts with *familiarity* and with the "no precedence" rule
(sections lean on knowing which operator binds where).

`TODO(open): operator sections.` Lean: not in 1.0. A lambda `\ self + 1` is
two extra characters and reads the same in every mainstream language.
Re-justify against embedding before adopting.

## Why

- **Two primitives suffice** — fewer trait methods to implement and to keep
  consistent. Stability wins.
- **Word operators** match the no-precedence story: `and`/`or` chains read
  cleanly inside the required parens.
- **No truthy/falsy** stops a whole category of DWIM bugs at the source.

## See also

- [Bool primitive](../05-types/02-primitive-types.md)
- [Precedence and Associativity](../04-syntax/04-precedence-and-associativity.md)
- [Fallback Operator](11-fallback-operator.md) — `??` for missing values.
- [Traits or Interfaces](../10-data-modelling/03-traits-or-interfaces.md)
- [Match Expressions](../08-control-flow/02-match-expressions.md)
