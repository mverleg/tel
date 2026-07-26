# While Loops

A `while` loop repeats a body for as long as a boolean condition holds. It is
the construct for "keep going until something changes."

## What it is

```tel
let uniq remaining = budget
while remaining > EuroAmt(0) {
    remaining = remaining - step
    process(step)
}
```

The condition is checked before each iteration; the body runs only while it is
`true`. As with [`if`](01-if-expressions.md), the condition must be a real
`Bool` — there is no truthy/falsy coercion.

## A `while` loop needs a mutable condition

A `while` loop only terminates if something it tests can *change* between
iterations. If the condition is built entirely from immutable bindings, it
either never runs or never stops — both are bugs, and neither is what a reader
expects from a loop.

So a `while` loop in practice depends on **mutable state**: a `uniq` binding the
body updates, a mutable collection that drains, or a capability whose result
varies (a clock, a channel). A loop whose condition references nothing mutable
is suspicious, and Tel should flag it.

```tel
# Suspicious: `done` can never change, so this is either skipped or infinite.
let done = false
while !done { tick() }
```

TODO(open): "while loops need a mutable condition?" — decide
whether Tel *rejects* a `while` whose condition provably cannot change (a hard
error), *warns* about it, or leaves it alone. A hard rule aligns with *the
compiler tells you about your mistakes* and *invalid states unrepresentable*,
but proving "cannot change" in general is undecidable; a conservative
syntactic check (no mutable binding, no capability call, no mutable-collection
read in the condition) is probably what is meant. This also depends on the
unresolved mutability model — see
[antifeatures open questions](../02-philosophy/04-antifeatures.md).

## Why keep `while` at all

`while` overlaps with [`for`](04-for-loops-and-iteration.md) and the bare
[`loop`](05-loop-and-break.md). Each has a clear lane:

- `for` — iterate a known sequence or range. Preferred when it fits.
- `while` — repeat until a *condition* flips. Use when there is no sequence to
  walk, only a changing predicate.
- `loop` — repeat unconditionally; exit with `break`.

Keeping all three is a mild tension with *one good way over many clever ones*,
but each maps to a distinct, familiar shape from mainstream languages
(*familiarity over a novel surface*), so all three stay.

## As an expression

A `while` loop run for its body's effects produces nothing (the unit value). It
is normally used as a statement. To produce a value from a loop, use `loop`
with a value-carrying `break` — see [loop and break](05-loop-and-break.md).

`break` and `continue` work inside `while` and target the nearest enclosing
loop; see [loop and break](05-loop-and-break.md).

TODO: review — new section; mutable-condition enforcement is the open call.
