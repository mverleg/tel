# Arithmetic and Numeric Operators

<!-- TODO: review -->

Tel's arithmetic operators are conventional: `+ - * /` plus a few familiar
extras. The interesting decisions are not which operators exist — they are how
the language **bans precedence games** and **requires whitespace** so an
expression cannot be misread.

## What

```tel
total + tax
qty * unit_price
distance / time
remaining - taken
```

The operands of a binary arithmetic operator must have the **same type**, and
the result has that type. There is no implicit widening, no silent integer
overflow, and no quiet NaN propagation — see
[antifeatures](../02-philosophy/04-antifeatures.md). If you want a `Float`,
convert explicitly (see [Conversion Expressions](09-conversions.md)).

Operators are overloadable for user types (see
[Overloading and Dispatch](../09-functions/09-overloading-and-dispatch.md))
but **their parsing never changes** — see
[Precedence and Associativity](../04-syntax/04-precedence-and-associativity.md).

## Forced parenthesisation and whitespace

Two rules, both load-bearing from the design notes:

### 1. Nested operations must be parenthesised

Tel has [no arithmetic precedence](../04-syntax/04-precedence-and-associativity.md).
A single binary operation is fine bare; the moment one operation feeds
another, the grouping must be written. The worst
offender:

```tel
# REJECTED — the meaning is not obvious to a reader
a * b / c * d

# ACCEPTED — explicit grouping, intent is unambiguous
(a * b) / (c * d)
(a * (b / c)) * d
```

Tel goes further: mixing `+` and `*` in one un-parenthesised expression is
also rejected, even though most languages would parse it under the usual
precedence rule. Tel does not ask the reader to remember a precedence table at
all.

A **flat chain of the same operator** is allowed because the answer does not
depend on grouping:

```tel
a + b + c          # fine — repeated `+`, left-to-right
total * 2 * 3      # fine — repeated `*`
```

`TODO(open): exactly which operators chain flat.` The lean is toward
`+ * && ||` and similar associative operators; everything else needs explicit
parens. Decide the list — see
[Precedence and Associativity](../04-syntax/04-precedence-and-associativity.md).

### 2. Whitespace around binary operators is required

A binary operator must be **surrounded by whitespace**: `a + b`, never `a+b`.
There are two reasons: readability, and reserving the no-space form for
possible future operator-like uses without ambiguity (postfix `-`, sections,
unary forms).

```tel
total + tax        # OK
total+tax          # rejected
- x                # unary minus
x - y              # binary minus
```

`TODO(open): unary minus.` Postfix minus is theoretically plausible but
"way too unfamiliar." Lean: keep prefix `-x` for negation as
mainstream languages do. Confirm and document.

## The operator set

Concretely, the design commits to:

| Op | Meaning | Notes |
|----|---------|-------|
| `+` | addition | overloadable; same type both sides |
| `-` | subtraction; prefix unary negation | overloadable |
| `*` | multiplication | overloadable |
| `/` | division | see "integer vs real division" below |
| `%` | remainder / modulo | `TODO(open): exact behaviour with negatives` |
| `**` or `^` | exponentiation | `TODO(open): which symbol, or none` |

### Integer division: `/` on two integers is rejected

**Decision.** `/` between two integer operands is a **compile error**. Both of
the usual answers are silent traps: truncating (`5 / 2 == 2`) quietly discards
the remainder, and promoting (`5 / 2 == 2.5`) is an implicit int→real
conversion — and Tel allows neither silent surprise (the Python-2-vs-3 `2 / 5`
split is the cautionary tale). The author must say which they meant:

```tel
5 / 2            # REJECTED — ambiguous: truncate? round? become a Real64?
Real64(5) / 2    # 2.5      — Real64 division, the Int64→Real64 conversion is explicit
div_floor(5, 2)  # 2        — floored integer division (stdlib)
div_trunc(5, 2)  # 2        — truncated-toward-zero integer division (stdlib)
div_exact(5, 2)  # error    — asserts the division is exact; 6/2 == 3 is fine
rem(5, 2)        # 1        — remainder, paired with the above
```

`/` *is* defined on two `Real64`s (`Real64 / Real64 -> Real64`), since there is no
hidden conversion there. The integer-division helpers live in
[Numerics and Math](../17-standard-library/07-numerics-and-math.md).

`TODO(open): final names and the exact set` (`div_floor` / `div_trunc` /
`div_exact` / `div_ceil`?), and whether `rem` vs `%` cover signed-operand
behaviour — see the `%` row below. Decided: the bare `/` on two integers is
*not* one of them.

### Bit-shifts are not operators

Shifts are bit operations, not arithmetic, and Tel reserves **no `<<` / `>>`
shift operator** — those symbols are not part of the grammar. Bit-shifting is
done only through named standard-library functions (`bin_left_shift` /
`bin_right_shift`), alongside the other bitwise helpers, which ship in `std` in
1.0. There is no shift symbol to give a precedence or to reserve. See
[Bitwise and Binary Operations](../17-standard-library/21-bitwise-and-binary.md).

## Overflow

Integer arithmetic **must not silently wrap**. Tel commits to no quiet
overflow [(antifeatures)](../02-philosophy/04-antifeatures.md); the exact
behaviour is:

`TODO(open): overflow semantics.` Choices left open are *abort on
overflow*, *saturate*, or *return a Result*. Lean: a checked default that
aborts (a panic-style failure, not undefined behaviour), with explicit
wrapping/saturating ops in the standard library for the rare case that
genuinely wants them. Settle alongside [Primitive Types](../05-types/02-primitive-types.md).

## Division by zero and NaN

There are five candidate behaviours for `x / 0`:

1. Return a real value that isn't mathematically meaningful (e.g. `1 / 0 == 0`).
2. Return a non-real value still typed as `Real64` (an `inf`).
3. Return a non-real value of a wider type (so every division returns an `Option` or `Result`).
4. Refuse to compile unless the divisor is *statically* known non-zero (a `PartialDiv` style refined type — see [Refined Types](../05-types/12-refined-types.md)).
5. Abort at the point of the division.

Tel's working answer follows the same shape as integer overflow above:
**abort** for the default `Real64 / Real64`, and a `divide_or_none` /
`divide_or_inf` family in the standard library for the cases that genuinely
want to keep going. `0 / 0` (which gives NaN in IEEE arithmetic and is
*especially* nasty because it propagates silently) is treated the same as
divide-by-zero: abort by default. A divisor with a refined type that excludes
zero (`Real64 where self != 0`) makes the division total at the type level — no
runtime check needed.

`TODO(open): final spelling and exactly when an `inf` or `NaN` can ever
appear as a `Real64` value at all.` The strictest option — `real` never holds
NaN or infinity, period — is the most consistent with "no quiet number
weirdness" but rules out using `Real64` for code that *legitimately* uses IEEE
edge cases (numerical algorithms, FFT). Lean: the default `Real64` excludes
NaN and infinity; an opt-in `IeeeReal` or similar carries the full IEEE
arithmetic surface.

## Element-wise / vector arithmetic

There is a request for arithmetic on *iterators* — write `1 / 2 * range(100) ^ 2`
and get an iterator of values, without an explicit `map`:

```tel
let halves = nums / 2                 # element-wise
let energies = (1 / 2) * speeds ^ 2   # element-wise across an iterator
```

This is exploratory and conflicts with "one good way." Two designs are sketched:

1. A separate element-wise operator family (`|+`, `|*` etc.) — explicit but
   verbose.
2. An `@elementwise` annotation that lets a normal function act element-wise
   when applied to an iterator. This keeps the call-site looking like an
   ordinary call.

`TODO(open): element-wise arithmetic.` Lean: don't lift arithmetic operators
to iterators implicitly — too magical. Prefer `nums.map \ self / 2` for the
common case, and let library code use `@elementwise` (or the trailing-closure
form) to make pipelines read well. Re-justify against embedding before
committing.

## Numeric literals in arithmetic context

A numeric literal's type is inferred from context where unambiguous — see
[Literal Expressions](01-literal-expressions.md). Combined with no implicit
widening, this means `let x = (count + 1.0)` is a type error: `count` is `Int64`
and `1.0` is real. Write the conversion.

## Why

- **No precedence to remember** — *readability* and *if it looks correct, it
  is correct.* See
  [Precedence and Associativity](../04-syntax/04-precedence-and-associativity.md).
- **Mandatory whitespace** — a reader's eye sees `a + b` as three tokens at a
  glance; `a+b` does not, and `-x` versus `- x` blurs the same way. The cost
  is one keystroke per operator.
- **No silent overflow / coercion** — strict static typing earns its keep at
  the operator level; otherwise it pays for the typing without preventing
  arithmetic surprises.

## See also

- [Precedence and Associativity](../04-syntax/04-precedence-and-associativity.md)
- [Operators and Punctuation](../03-lexical-structure/06-operators-and-punctuation.md)
- [Comparison and Logical Operators](03-comparison-and-logical.md)
- [Bitwise and Binary Operations](../17-standard-library/21-bitwise-and-binary.md)
- [Conversion Expressions](09-conversions.md)
- [Overloading and Dispatch](../09-functions/09-overloading-and-dispatch.md)
