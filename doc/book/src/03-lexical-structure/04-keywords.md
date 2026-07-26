# Keywords

Keywords are a **fixed, reserved set of words** that the language defines and a
program may not use as identifiers. The set is frozen with the language: because
Tel is effectively frozen at 1.0 (see
[Priorities](../02-philosophy/01-priorities.md)), no later release adds a keyword
that could break an existing script.

This page records what the design notes commit to *about* keywords. The exact
spelling of individual keywords is settled in the chapters that introduce the
corresponding feature, not here.

## A medium-sized keyword set is acceptable

Tel does not chase a minimal keyword count. The design notes are explicit that
*"a medium amount of keywords is fine if it helps succinctness"* — a keyword that
makes common code read clearly earns its place. This trades a slightly larger
vocabulary for fewer punctuation-heavy idioms, which suits *readability over
writability*.

## Postfix keywords

Several of Tel's keywords are used **postfix** — they follow their operand
rather than preceding it — so that data-transformation code reads left-to-right
and chains cleanly. Among these are `return`, `print`, and `assert`,
alongside `then` (paired with `if`) and a `for`/`forEach` form.

```tel
# postfix forms read as "do X, then announce the outcome"
score(order, clock).print
total > EuroAmt(0)       assert
Ok(result)               return
```

This is a *how it looks* sketch, not pinned syntax. Postfix keywords interact
closely with method-call chaining `x.f(y)` and with lambda syntax — see
[Expressions vs Statements](../04-syntax/02-expressions-vs-statements.md) and
[Operators and Punctuation](06-operators-and-punctuation.md).

`TODO(open): the postfix keyword set and its spelling.` The candidate postfix
set is `return`, `print`, `assert`, `then`, `for`/`forEach`, but this is one of
the most exploratory areas of the design. Open sub-questions:

- Is `print` really a keyword? It implies an ambient output sink, which
  conflicts with Tel's capability model — there is no ambient `stdout`. More
  likely `print` is an ordinary function reached through a capability the host
  injects, and only *reads* postfix because of method-chaining, not because it
  is a keyword. `TODO(open): pre-pivot — re-justify `print` against embedding;`
  it probably is not a keyword.
- Mixing prefix and postfix keyword forms can hurt familiarity. Decide which
  keywords are postfix-only, which are prefix-only, and which (if any) allow
  both.

## Fully reserved, never contextual

Keywords in Tel are **fully reserved**: a keyword can never be used as an
identifier *anywhere*, even in a position where it would be unambiguous. Tel does
not have contextual keywords (words that are keywords only in certain positions
and ordinary identifiers elsewhere).

The rationale is consistency and tooling:

- *Simpler lexer.* A reserved word is classified by the lexer in isolation,
  with no parser feedback. This keeps lexing context-free and friendly to the
  fixed-lookahead approach Tel relies on.
- *No context-sensitivity.* "Is this word a keyword here?" has one answer
  everywhere, so a reader never has to reconstruct the grammatical position to
  know what a word means.
- *Better tooling and error messages.* Editors can colour and complete keywords
  without a full parse, and the compiler can give a direct "X is a reserved
  keyword" error instead of a confusing downstream parse failure.

The cost is a handful of attractive words removed from the identifier namespace;
given Tel's *readability over writability* stance and its [frozen-at-1.0](../02-philosophy/01-priorities.md)
keyword set, that trade is worth it. The reserved set itself is catalogued in the
appendix — see [Keywords Reference](../20-appendix/01-keywords.md).

## Words the design commits to

The chapters elsewhere already lean on a number of keywords. Rather than repeat
them here, the canonical roster lives in the appendix:
[Keywords Reference](../20-appendix/01-keywords.md). That table is the single
source of truth — it lists every reserved word, links each to the chapter that
settles its spelling and meaning, and tracks the words pre-reserved for future
use. This page stays focused on the *policy* about keywords (the points above);
the appendix carries the list.

`TODO(open): `self` is not declared.` `self` may be
used in methods without being declared as a parameter — i.e. there is no
`(self, ...)` in the parameter list, `self` is just always in scope inside an
`impl` block. This is convenient but differs from Rust (which makes `self`
explicit). Decide; pin down in
[Method Syntax](../09-functions/08-method-syntax.md).
