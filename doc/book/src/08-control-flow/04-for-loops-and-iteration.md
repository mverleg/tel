# For Loops and Iteration

A `for` loop walks a sequence — a range, a collection, an iterator — and runs
its body once per element. It is the preferred loop when there *is* a sequence
to walk; use [`while`](03-while-loops.md) only when the exit is a changing
condition rather than a sequence.

```tel
for order in todays_orders {
    process(order)
}
```

`break` and `continue` work inside `for` and target the nearest enclosing loop
— see [loop and break](05-loop-and-break.md).

TODO(open): the iteration protocol is not pinned down — what makes a
value iterable, whether there is an `Iterator`/`Iterable` (or Swift-style
`Sequence`) trait, range syntax, and how iteration interacts with the
mutability model. This is a stub until those are designed; for now only the
generator side below is described.

## Iterating a linear source

Over an ordinary [fused iterator](../10-data-modelling/10-iterators-and-sequences.md)
`for` is the obvious `while let` over `next() -> Option[T]`. Over a **linear**
(single-poll) source — one that must not be polled after it ends, such as a
channel receiver or a read-to-EOF cursor (see
[substructural types](../12-memory-and-runtime/08-substructural-types.md#the-iterator-value-as-a-linear-resource))
— `next` instead *consumes* the iterator and hands the tail back through its
return, `More(T, Iter[T]) | Done`. `for` **hides** the ownership re-threading:
it rebinds the tail each step.

```tel
for x in rx { body }
# ==>
let uniq it = rx.into_iter()
loop {
    match it.next() {              # MOVES it
        More(x, rest) => { it = rest; body },   # re-wire to the tail
        Done          => break,    # it consumed — no live `it` after the loop
    }
}
```

Because the `Done` arm leaves **no** iterator in scope, an `it.next()` after
the loop is a use-after-move compile error: you cannot poll a dead source. End
users writing `for x in rx { ... }` never see the re-thread — only a
hand-written loop over a linear source pays the explicit `it = rest`, and a
`break`/`return` out of the body leaves the reinstated `it` as a live must-use
binding to settle (the ordinary relevant-binding rule). The two desugarings —
fused and linear — are dispatched on which `next` the source's type provides.

## Generators

A **generator** is a function that produces a sequence lazily by `yield`ing
values one at a time, instead of building a whole collection up front. It is the
natural producer for a `for` loop to consume.

```tel
fn fibonacci() -> Generator[Int64] {
    let uniq a = 0
    let uniq b = 1
    loop {
        yield a
        let next = a + b
        a = b
        b = next
    }
}
```

Generators are familiar from Python and fit Tel's data-transformation focus —
streaming a transformation without materialising every intermediate value. The
following design points are **open**.

### Eager or lazy start

A freshly-created generator should **not** run its body yet. It starts suspended
and runs up to the first `yield` only when first asked for a value. Starting
lazily is what makes a generator usable as a value you can pass around before
anyone consumes it.

This mirrors the broader pattern Tel wants for tasks and futures: the *thing
that will produce values* (an `IntoIterator`-like seed) is separate from the
*running* iterator. The seed is cheap and movable; the running generator, once
it has captured local state, is not freely movable. See the concurrency
chapter.

TODO(open): name and shape the seed-vs-running split. Candidate: a generator
*expression* yields an inert `Generator[T]` value; iterating it (via `for` or an
explicit `next`) starts it. Confirm.

### Returning a value

A generator may end early with `return`. Whether it may `return` a *value*
distinct from its yielded stream is open:

- If it can, the generator has two output channels — a stream of `yield`ed
  values and one final `return` value — and a plain `for` loop cannot express
  the second cleanly. Python 3 smuggles it through the
  stop-iteration exception and Rust exposes it via a yield/return enum.
- Lean: a generator *may* `return` a value, but `for`-loop sugar only applies
  when the return type is unit. A generator that returns a meaningful value is
  driven explicitly. This keeps the common case (`for x in gen()`) simple
  without forbidding the richer one.

TODO(open): confirm the above, and decide the explicit-drive API and whether
the yield type and return type are surfaced as one enum per step.

### Bidirectional `send`

Python generators are bidirectional — `yield` is an expression, and the
consumer can `send` a value back in that becomes the result of `yield`. This is
powerful but makes control flow hard to follow and overlaps heavily with the
results-vs-callbacks tradeoff (see
[`13-error-handling/01-philosophy.md`](../13-error-handling/01-philosophy.md)).

TODO(open): decide whether Tel generators are **one-way** (a pure producer,
`yield` is a statement) or **bidirectional** (`yield` is an expression a
consumer can `send` into). *One good way over many clever ones* and
*readability over writability* both lean one-way; bidirectional generators are a
coroutine in disguise and are hard for the IDE to follow. Recommend one-way
unless a concrete embedding use case demands `send`. Re-justify against
embedding either way — pre-pivot notes assumed standalone Python-style use.

TODO: review — new section; the whole generator design is a cluster of open
questions and only the lazy-start point is close to settled.
