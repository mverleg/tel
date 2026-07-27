# Whitespace and Newlines

Whitespace in Tel is **significant in one way only**: a newline can terminate a
statement. A space is otherwise just a token separator.

1. **Newlines can terminate statements.** A newline normally ends a statement
   (see below), so a line break is a token-significant event, not mere spacing.
2. **Spaces only separate tokens.** A space carries no other meaning *for the
   parser*. Space (juxtaposition) application — writing `min a b c` for
   `min(a, b, c)` — was considered and **rejected** (see
   [Function Application](../07-expressions/06-function-application.md) and
   [Antifeatures](../02-philosophy/04-antifeatures.md)), so spacing between
   tokens never changes how an expression parses. It is *not*, however, free
   taste: Tel enforces one canonical spacing (spaces around binary operators,
   tight `.`, no space inside `()`), reported and auto-fixed rather than parsed
   differently — see
   [Operators and Punctuation §Spacing](06-operators-and-punctuation.md#spacing).

What is *not* significant is **block structure**: indentation and the amount of
leading whitespace never determine nesting. Only `{}` delimit blocks — see
[Layout Rules](../04-syntax/05-layout-rules.md) for the full rationale. Code
whose indentation is mangled by a host config UI still means exactly the same
thing.

## Newline as statement terminator

Tel statements end at a newline. There is no mandatory `;` at the end of every
line — the newline does that job — which keeps everyday code free of line noise
while still giving the parser a clear, fixed point where a statement ends.

{{#spec NEWLINE_TERMINATES_STATEMENT}}

```tel
let total = order.total
let age   = order.age_days
Score.from(total, age)
```

When two statements share a line, an explicit separator divides them. The exact
separator token is settled in
[Layout Rules](../04-syntax/05-layout-rules.md); the lexical point here is that
*a newline is a token-significant event* even though *indentation is not*.

This pairs with the bracket choices: blocks are delimited by `{}`, not by
layout, so the only thing a newline controls is where one statement stops and
the next begins — never nesting depth.

## Continuation: when a newline does *not* terminate

A newline terminates a statement **except in two cases**:

1. **An opening bracket is still unclosed.** While a `(`, `[`, or `{` is open,
   the newlines inside it are insignificant, so an expression spans as many
   lines as the brackets enclose.
2. **The next line begins with a leading `.`.** A line whose first non-space
   token is `.` continues a method chain off the previous line — so a multi-line
   chain needs **no** wrapping parentheses.

{{#spec LEADING_DOT_CONTINUATION}}

```tel
render(
    title,
    body,
    footer,
)

# A multi-line chain: each leading `.` continues the previous line.
let report = orders
    .keep(|o| o.total > zero)
    .map(|o| score(o))
    .sorted()
```

Apart from those two cases Tel has **no other continuation rules**: a *trailing*
operator does not reach onto the next line, and there is no `...`
line-continuation marker.

```tel
let area = width *          # ERROR: the newline ends it; `*` has no right operand
    height

let area = (width *         # OK — the open `(` holds the line open
    height)

let area = width * height   # OK — one line needs no grouping
```

### Brace-less closures stay on one line

The leading-`.` rule is unambiguous only because an **expression-bodied
closure** — one written without `{}`, such as `\ total > zero` or
`|x| x + 1` — **may not contain a newline.** Its body ends at the line's
newline like any other expression. That is exactly what lets the *next* line's
leading `.` attach to the surrounding chain rather than to the closure body:

```tel
orders
    .keep \ total > EuroAmt(0)      # the closure body ends at this newline
    .map \ score(self)              # leading `.` continues the chain, not the closure
    .sorted()
    .take(10)
```

A closure body that genuinely needs more than one line uses **braces**
(`\{ ... }`, `|x| { ... }`), which hold themselves open with `{` — the
unclosed-bracket case again. So the two lambda shapes split cleanly: an
expression-bodied closure is single-line and ends at its newline; a brace-bodied
closure spans lines because its `{` is open.

`TODO(open): `;` separator on one line.` Two statements on one line are
separated by `;`. A trailing `;` before `}` is permitted but redundant. Confirm
in [Layout Rules](../04-syntax/05-layout-rules.md).

## Still a cheap lexer rule

The decision stays mechanical for a fixed-lookahead lexer. A newline at bracket
depth greater than zero is plain whitespace. A newline at depth zero ends the
statement **unless** the next non-space token is `.`, which one-token lookahead
settles. The only supporting rule is that a brace-less closure body cannot cross
a newline — without that, a following `.method` would be ambiguous between
"extend the closure body" and "continue the chain"; with it, the closure body
is already closed, so the `.` can only continue the chain. See
[Closures and Lambdas](../09-functions/06-closures-and-lambdas.md#how-far-the-body-reaches).

## Why this design

A short, mechanical newline-terminates-a-statement rule keeps everyday Tel free
of trailing `;` while still giving a fixed-lookahead lexer a definite end point
— supporting *fast, predictable compilation* and easy re-implementation across
hosts. Crucially it does **not** make *indentation* significant: code pasted
into a host config UI or an in-browser box, where leading whitespace is mangled,
still means exactly the same thing. Only line *breaks* matter, and those survive
copy-paste.

TODO: review
