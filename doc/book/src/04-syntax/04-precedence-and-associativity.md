# Precedence and Associativity

## Mixing different operators requires parentheses

Tel has **no global precedence table**. The reader never has to recall whether
`+` binds tighter than `/`, because the moment two *different* binary operators
meet, Tel demands explicit parentheses instead of resolving the grouping for
you. The compiler refuses `3 + 3 / 2 % 2` outright rather than quietly reading
it off a precedence ladder.

{{#spec MIXED_OPERATORS_REQUIRE_PARENS}}

```tel
# REJECTED — mixes +, /, and %
3 + 3 / 2 % 2

# ACCEPTED — each step between different operators is parenthesised
3 + ((3 / 2) % 2)

# a single, non-nested operation needs no parentheses
total + tax
order.total > EuroAmt(0)
```

### The exceptions

A strict "parens for everything" rule is punishingly verbose where there is no
real ambiguity. Three cases may be written flat, and each **associates
left-to-right**:

1. **Repeated same operator** — `a + b + c`, `a * b * c` read as `((a+b)+c)`.
2. **The additive group** — `+` and `-` form one familiar left-to-right family,
   so they mix freely: `balance + credit - debit` is `((balance+credit)-debit)`.
3. **Boolean chains** — a flat chain of `and`, or of `or`, short-circuits
   left-to-right. Mixing `and` with `or` is a different-operator mix and needs
   parentheses (see
   [Comparison and Logical Operators](../07-expressions/03-comparison-and-logical.md)).

```tel
# ACCEPTED
a + b + c                     # repeated same operator
balance + credit - debit      # additive group
ready and primed and (not aborted)   # boolean chain

# REJECTED — + mixed with * ; write it out
a + b * c
a + (b * c)
```

Everything else is parenthesised the instant two different operators meet, so
outside these three cases there is nothing left to associate — that is the whole
story, with no ladder for the grammar to climb across operator kinds.

### Why

- *Readability over writability; "if it looks correct, it is correct."*
  Precedence bugs — `a + b * c` read one way by the writer and another by the
  table — are a common silent error class. Requiring parentheses where operators
  mix makes the intended grouping visible at the point of use. The three
  exceptions carry no such ambiguity, so they pay no parenthesis tax.
- *This kills the classic C "hundred-year mistake" by construction.* In C and
  its descendants `a & b == c` parses as `a & (b == c)` because `==` binds
  tighter than `&` — almost never what the writer meant, and a gotcha copied
  forward into language after language (Eric Lippert, "Hundred Year Mistakes").
  In Tel that line is simply **rejected**: `&` and `==` are different operators,
  so you must write `(a & b) == c` or `a & (b == c)`. There is no precedence to
  inherit and therefore no trap to inherit. (Bitwise `&` is in fact a stdlib
  function, not an operator, in Tel — see
  [Arithmetic and Numeric](../07-expressions/02-arithmetic-and-numeric.md) — but
  the rule would catch the mix regardless.)
- *Fast, predictable compilation.* With no cross-operator precedence to resolve,
  the expression grammar parses with fixed lookahead — see
  [Grammar Notation](01-grammar-notation.md).
- *Familiarity is outranked here.* Most languages rank `*` above `+`; this is a
  rare place Tel chooses against familiarity, permitted because the rule is
  learned once, not a trap that bites silently.

`TODO(open):` the operator inventory itself (see
[Operators and Punctuation](../03-lexical-structure/06-operators-and-punctuation.md)),
and whether any operators beyond `+`/`-` earn a mixed group. Lean: no — keep the
additive group the sole multi-operator exception. Comparison/equality use
separate *chaining* (see
[Comparison and Logical Operators](../07-expressions/03-comparison-and-logical.md)).

## Operator overloading

Tel's operators are a fixed set: you cannot define new operators or change any
operator's precedence or associativity. You *can* **overload** an existing one
for your own types, Rust/Python-style — each operator binds a trait, and a type
implementing it can use the operator. This is kept (where custom operators are
not) because `a * b` reads as clearly as `a.mul(b)`, and only fixed precedence
lets a reader avoid learning a project's private notation.

- An overload changes what an operator *does*, never how it parses.
- It must satisfy its trait's contract — equality stays reflexive/symmetric,
  ordering stays a total order. No surprise semantics: what reads like addition
  adds.
- **An overload cannot do I/O.** The operator traits take no capability
  parameters, and I/O, time, and randomness are reachable only through a
  capability the caller passes in (see
  [Function Types](../05-types/05-function-types.md#effects-belong-on-the-function-type)),
  so `a * b` cannot touch the network, clock, or filesystem, nor lazily load —
  it computes from its operands. (It may still allocate, and panic where the
  trait's contract allows.) This keeps every effect a line can have visible in
  its named calls, never hidden in a symbol.

TODO(open): the exact set of overloadable operators and their traits (see
[`../09-functions/09-overloading-and-dispatch.md`](../09-functions/09-overloading-and-dispatch.md)),
and whether the no-capability rule is operator-specific or a general "syntax-used
trait methods are capability-free" rule. Lean: the general rule.
