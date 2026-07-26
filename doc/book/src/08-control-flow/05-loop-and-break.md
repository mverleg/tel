# Loop and Break

`loop` is the unconditional loop: it repeats its body forever until something
inside it stops it. `break` and `continue` are the two ways to steer any loop.

## `loop`

```tel
loop {
    let line = input.next()
    match line {
        none      -> break,
        Line(txt) -> process(txt),
    }
}
```

`loop` is the right choice when there is no sequence to walk and no single
condition to test up front — the exit lives in the middle of the body. Using
`loop` plus `break` instead of `while true` makes the intent explicit: a reader
sees immediately that this loop ends from inside, not from a condition.

## `break`

`break` exits the loop immediately. It targets the **nearest enclosing loop**.
A `match` or `if` between the `break` and the loop is transparent — `break`
inside a `match` arm still breaks the loop, not the `match`. See
[match expressions](02-match-expressions.md).

### Value-carrying `break`

Because Tel is expression-oriented, `loop` can produce a value: `break` may
carry one, and that value becomes the value of the whole `loop` expression.

```tel
let first_match = loop {
    let candidate = pool.next()
    if candidate.fits(criteria) {
        break candidate
    }
}
```

This is how a loop returns a result without a mutable temporary. A plain
`break` (no value) is for loops used as statements; their value is unit.

`while` and `for` can also `break`, but they have a path where the loop ends
*without* a `break` (the condition fails, the sequence is exhausted). A
value-carrying `break` therefore only makes clean sense for `loop`, or for
`while`/`for` paired with an `else`-style fallback for the no-`break` path.

### The `else` clause (open proposal)

The design notes float a Python-style **`else` clause** on loops. In Python,
`for`/`while` take an `else` block that runs only when the loop finished
*without* a `break`. For Tel the interesting use is value-carrying: a loop
`else` could supply the loop's value on exactly the no-`break` path that makes
value-carrying `break` awkward for `while`/`for`.

TODO(open): decide whether Tel adopts a loop `else` clause, and what to call it —
the name `else` is a poor fit, since it reads as the `if`/`else` pairing, which
this is not. There is no `try` in Tel, so Python's `try`/`else` use does not
apply. Lean: keep value-carrying `break` as `loop`-only for now and treat the
loop `else` as a separate, lower-priority proposal — it risks being a second
clever way to do what an `if` after the loop already does (*one good way over
many clever ones*).

## `continue`

`continue` skips the rest of the current iteration and moves to the next one.
Like `break`, it targets the nearest enclosing loop.

```tel
for order in orders {
    if order.cancelled {
        continue
    }
    process(order)
}
```

## Nearest-loop targeting and nested loops

Both `break` and `continue` act on the **innermost** loop containing them. The
input is explicit: *for break/continue just use the nearest loop*. There is no
labelled-break syntax described.

To break out of an outer loop from inside an inner one, the straightforward
options are an early [`return`](06-early-return.md) from the enclosing function,
or a `uniq` flag tested by the outer loop.

TODO(open): decide whether Tel needs labelled loops / labelled `break` for the
nested-loop case. *One good way over many clever ones* and *familiarity* pull
both ways — Rust and Kotlin have labels, Python does not. Lean: omit labels;
revisit only if real scripts show the workaround is painful.

TODO: review — new section; value-carrying `break` scope and labels are open.
