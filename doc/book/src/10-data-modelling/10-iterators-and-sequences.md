# Iterators and Sequences

An **iterator** (or *sequence*) is a value that produces a series of elements
on demand. Iteration is the shared shape across every Tel container, finite
or infinite, lazy or eager. It is what lets `map` / `filter` / `take` /
`reduce` work uniformly on a `List`, a `Set`, a `Map`'s values, a generated
range, and a user-defined stream.

## What — one trait, many sources

Tel exposes iteration through a single trait (working name **`Iterable`**),
implemented by every collection in [`09-collection-types.md`](09-collection-types.md)
and by user-defined sources. A consumer that works on an `Iterable[T]` works
on all of them.

```tel
take(3, [1, 2, 3, 4, 5, 6, 7])              # [1, 2, 3]  from a list
take(3, range(1, infinity))                  # [1, 2, 3]  from a generator
take(3, generate(1, |x| x + 1))              # [1, 2, 3]  from a custom rule
take(3, words("the quick brown fox"))        # ["the", "quick", "brown"]
```

This is the "shared parent types for lists and generators"
goal: scripts should not have to know whether the source is a finite list or
an infinite stream to use it the same way. `take` itself is the canonical
example — it makes "the first N" meaningful uniformly.

## Why — uniform iteration is the data-transformation backbone

Tel's primary use cases (small to medium data transformation, schema work,
modding hooks) are largely about *running an operation over a series*.
Forcing the author to know the *kind* of series — list, set, generator —
defeats reuse:

- A function that processes "all the orders" should accept whatever the host
  gave it, whether that is a `List[Order]` already in memory or a generator
  that pulls one at a time from a host iterator.
- The standard library's `map` / `filter` / `take` / `reduce` should write
  once, work on all of them.
- Combining sources (a finite list followed by a generated tail) should be
  obvious — `concat(items, generate(...))` returns an `Iterable[T]` and
  consumers do not care what is inside.

The cost is a single trait the language commits to and every collection
implements. The benefit is non-overlapping, composable, consistent
operations — exactly the standard-library shape the
[maxims](../02-philosophy/02-maxims.md) ask for.

## On Swift's `Sequence` regret

Jordan Rose regrets that Swift's `Sequence` — the base of its collection
hierarchy and the thing `for`-in iterates — guarantees too little: you can't
tell whether it's finite or whether you may iterate it twice. Tel deliberately
takes the **Rust `Iterator` model** anyway: an `Iterable` is single-pass and not
necessarily finite. That is *usable and fast* in practice — Rust's `.iter()`
makes no repeatability or finiteness promise yet has excellent ergonomics and
codegen — so the critique is not treated as blocking. Where Tel does better than
Swift is finiteness: rather than leave it implicit, finiteness is a separate
**`FiniteIterable`** bound (below), so a consumer that needs a finite source
*says so in its type* instead of running off the end.

## Finite vs infinite

An `Iterable[T]` is *not* known to be finite. Operations that need a finite
input — `len`, `to_list`, `sum`, `sort` — would loop forever on an infinite
source. There are two reasonable responses:

1. **Two traits.** `Iterable[T]` for "produces elements on demand",
   `FiniteIterable[T]` for "and eventually stops". Operations like `to_list`
   require the finite bound.
2. **One trait, finite-by-default consumers, lazy adapters that may produce
   infinite outputs.** A `to_list` always assumes finite; if it is wrong, the
   script loops — the same hazard every iteration-heavy language carries.

This is not yet pinned down. Lean: **option 1**, two traits. The cost is
one extra bound to write; the benefit is the type system tells the author
"this consumer needs a finite source" instead of running off the end.

TODO(open): commit to one of the two models. Lean: two traits
(`Iterable` and `FiniteIterable`), with the bulk of collection types
implementing both and only generators picking. Pre-pivot check:
infinite streams are mostly a *standalone-script* idiom; for embedded use
the host usually supplies a finite cursor. Confirm worth the extra surface.

## Eager vs lazy operations

Pipeline operations split into:

- **Eager** — run immediately and produce a concrete value. `to_list`,
  `len`, `sum`, `find`, `any`, `reduce`.
- **Lazy** — produce another `Iterable[T]` without consuming the source.
  `map`, `filter`, `take`, `drop`, `chunk`, `zip`, `concat`.

Lazy by default for the *intermediate* steps and eager at the *terminal*
step is the standard Rust-`Iterator` / Java `Stream` / Kotlin `Sequence`
shape. Tel does not deviate from it; the consistent rule is:
**transformations are lazy, terminal operations realise**.

```tel
let result = items
    .map(|x| x * 2)
    .filter(|x| x > 10)
    .take(5)
    .to_list()          # terminal — actually runs the pipeline
```

A lazy pipeline that is never consumed simply does nothing. There is no
hidden eager step.

TODO(open): whether a *side-effecting* lazy step (`for_each`, `tap`) exists,
and how it interacts with the effect system in
[`../05-types/05-function-types.md`](../05-types/05-function-types.md). Lean:
side-effecting iteration uses a `for` statement, not a lazy adapter; a
`tap`-style debug helper is a standard-library nicety.

## Generators

A **generator** is a user-defined `Iterable` that produces elements by
running a function. The obvious forms:

```tel
range(1, 100)                    # 1, 2, ..., 99
generate(seed, |x| next(x))      # custom recurrence
unfold(state, |s| ...)           # produce-and-update
```

These are *not* a language feature in themselves — each is a standard-library
helper that produces an `Iterable[T]`. Open whether Tel needs a
coroutine-style `yield` keyword that turns a function body into a generator
(Python style). That is a real ergonomic win for complex generators, but it
implies an effect (suspending and resuming a function) the rest of the
language has to model.

TODO(open): commit to or reject a `yield`-based generator syntax. Lean:
*defer*. Combinator-style generators (`generate`, `unfold`, `concat`, `map`)
cover most cases; a single-resume-point `yield` would compose with the
[effects discussion in `05-function-types.md`](../05-types/05-function-types.md),
but it is a meaningful new construct and Tel can ship without it. Revisit
once the effect model is settled.

## Combining sources

Sources combine into longer pipelines without caring whether each operand is
finite or infinite — provided the combined operation makes sense:

```tel
concat(items, generate(1, |x| x + 1))      # finite then infinite — still iterable
zip([1, 2, 3], ["a", "b", "c", "d"])       # stops at the shorter source
chain(seasons, festivals)
```

`zip` of a finite and an infinite source is finite (it stops with the
shorter); `concat` of two infinite sources is infinite (the second is
unreachable). The type signatures convey which.

TODO(open): how `FiniteIterable` is preserved or lost through combinators —
`concat(Finite, Finite)` is `Finite`; `concat(Finite, Iterable)` is just
`Iterable`. Standard structural propagation, but spell it out in the
combinator signatures.

## Iteration order

For an ordered source (`List`, `Array`, `OrderedMap.values()`,
`SortedSet`), iteration order is **defined and stable** — same input, same
order, across hosts and runs. This is the reproducibility commitment from
[the maxims](../02-philosophy/02-maxims.md).

For an unordered source (`Set`, `Map` without ordering), iteration order is
*not* guaranteed and **carries the `random` effect** when observed — see
[`07-equality-and-hashing.md`](07-equality-and-hashing.md). A script that
needs reproducible iteration over an unordered source either fixes the host
seed or converts to a sorted view (`set.sorted()`).

## Head/tail destructuring

Tel wants first-class **head/tail pattern matching on lists**, the way
ML-family languages do — see
[`06-pattern-matching-in-depth.md`](06-pattern-matching-in-depth.md):

```tel
match items {
    Empty           => 0,
    [first, ..rest] => 1 + count(rest),
}
```

This is `match` syntax — not an iteration-trait feature — but it works
seamlessly with lists because a `List` is a recursive type (see
[`05-recursive-types.md`](05-recursive-types.md)). It does not generalise to
arbitrary `Iterable`s: a one-shot generator cannot be "destructured" without
running it, so head/tail is for lists specifically.

## Iterators as values vs as control flow

An iterator is a *value* of an `Iterable[T]` type that can be passed around,
stored, and consumed lazily. A `for` loop is the control-flow shape over an
iterator — sugar for the obvious `while let` over `next()`. Iteration and
`for` are two faces of the same trait, not separate
mechanisms.

```tel
for x in items.filter(|x| x > 0) { print(x) }
```

TODO(open): whether `for` is the *only* control-flow form (as in Python) or
sits alongside a `while` and a `loop` (as in Rust). Defer to the syntax /
control-flow chapter — this page only assumes the existence of `for`.

## Linear (single-poll) iterators

The `next() -> Option[T]` model above is **fused**: calling after `None` is
legal and yields `None` again. That is the committed **default**, and the right
contract for plain-data walks — a `List` is `Alias`, so `list.iter()` is fused
and re-callable at no risk.

For a **resource-backed** source — a channel receiver, a read-to-EOF cursor, a
future polled to `Ready` — re-polling after the end is meaningless or a bug. A
source of that kind opts into a **linear** iterator, one the type system
forbids from being polled after it ends. It needs **no new syntax**: `next`
*consumes* `self` and threads the continuation back through its return value,
and the terminal variant carries no iterator:

```tel
type Step[T] = More(T, Iter[T]) | Done      # `Done` carries no Iter[T]

fn next(a self: Iter[T]) -> Step[T]          # CONSUMES self
```

Because `Done` carries no iterator, once iteration ends there is nothing in
scope to call again — re-polling a dead source is a plain **use-after-move**
compile error, not a discouraged convention. This "one-shot" contract is
legible right at the type. The [`for`](../08-control-flow/04-for-loops-and-iteration.md)
loop hides the ownership re-threading, so end users writing `for x in rx { ... }`
see nothing; combinator authors (`map` / `filter` / `take`) encapsulate it once
per adapter. An early-terminating adapter such as `take` must **settle** the
un-consumed tail of a *relevant* source rather than silently drop it (the
ordinary relevant-binding rule; an affine `Discard` source is abandoned to GC).

The linear-vs-fused distinction is a different axis from *source* affinity: it
is a property of the **iterator value itself**, and the consuming shape compiles
to the same machine code as the fused one (identity is not observable). Both
points are covered in
[substructural types](../12-memory-and-runtime/08-substructural-types.md#the-iterator-value-as-a-linear-resource);
the reason a borrowing `next(&!self)` *cannot* express the dead end is the
unconditional give-back of a `&!` borrow, in
[references and aliasing](../12-memory-and-runtime/04-references-and-aliasing.md).
This design was decided in
[TIP-0011](../tips/0011-resuming-borrows-and-linear-iterators.md), which also
records why the proposed give-back *sugar* was rejected.

## What Tel does *not* do

- **No external iterator protocol cobbled together from a `hasNext` /
  `next` pair.** A single `next() -> Option[T]` is enough — the absence value
  is the end signal. (This is the Rust shape; the Java `Iterator` shape with
  `hasNext` is rejected as twice the API for one purpose.) This fused
  `next() -> Option[T]` is the **default**; a resource that must not be
  re-polled opts into the [linear form](#linear-single-poll-iterators) instead,
  where the end leaves no iterator to call.
- **No multiple iteration semantics on the same trait.** A finite list,
  iterated twice, gives the same elements twice; a one-shot generator does
  not — that distinction is in the *source*'s type (`List` vs a generator
  built from a closure), not in two flavours of `Iterable`.
- **No language-level streams / observables / reactive plumbing.** Lazy
  iteration plus capability-based event sources from the host (a `Channel[T]`
  the host supplies) cover the practical cases without dragging a reactive
  framework into the language.

## Open questions — linear iterators

These are parked from [TIP-0011](../tips/0011-resuming-borrows-and-linear-iterators.md);
they stay open and do not block the settled model above.

- TODO(open): **Two protocols.** Fused (`next(&!self) -> Option[T]`) and linear
  (`next(self) -> Step[T]`) are now visibly *different* protocols. How `for`
  and the combinator family (`map` / `filter` / `take` / …) span both — one
  protocol-generic family with fused as the free degenerate case, or two
  separate families — is the remaining decision. The considerations are
  collected in [`inputs/linear-iterator-two-protocols.md`](../../../inputs/linear-iterator-two-protocols.md).
  Keep {fused | linear}, {finite | infinite}, and {alias | affine source} as
  three independent axes.
- TODO(open): **Is there a `LinearIterable` / `OnceIterable` bound**, or is
  "linear" purely a property of a given `next`'s signature with no new trait?
  Lean: a property, no new trait — mirrors the "one iterator design" of
  [substructural types](../12-memory-and-runtime/08-substructural-types.md#iterating-affine-vs-non-affine-sources).
  Ties into the two-protocol dispatch question above; decide them together.
- TODO(open): **Name the terminal-settle operation** a `break`/`return` uses to
  drain a relevant linear source's un-consumed tail (`settle` is a placeholder
  in the TIP examples). Align with the linear-resources / [substructural]
  (../12-memory-and-runtime/08-substructural-types.md) chapter.

## See also

- [Collection Types](09-collection-types.md) — the concrete `Iterable`
  implementations.
- [Pattern Matching In Depth](06-pattern-matching-in-depth.md) — head/tail on
  lists.
- [Function Types](../05-types/05-function-types.md) — closures and the
  effect story that touches lazy iteration.
- [Equality and Hashing](07-equality-and-hashing.md) — iteration order and
  the `random` effect.

TODO: review
