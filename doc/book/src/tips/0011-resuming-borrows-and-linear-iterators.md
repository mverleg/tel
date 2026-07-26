# TIP-0011: Linear iterators (via explicit ownership threading)

**Status:** Accepted and **migrated** into the chapter docs (2026-07-02) — see
[iterators and sequences](../10-data-modelling/10-iterators-and-sequences.md#linear-single-poll-iterators),
[substructural types](../12-memory-and-runtime/08-substructural-types.md#the-iterator-value-as-a-linear-resource),
[references and aliasing](../12-memory-and-runtime/04-references-and-aliasing.md),
and [`for` loops](../08-control-flow/04-for-loops-and-iteration.md#iterating-a-linear-source).
The "resuming borrow" sugar is **rejected** and recorded as an
[antifeature](../02-philosophy/04-antifeatures.md) (see §"Rejected: resuming-
borrow sugar" below). Kept as the historical record. The remaining open items —
how `for` and the combinators span the fused and linear protocols, whether a
`LinearIterable` bound exists, and the terminal-settle naming — are relocated to
the [iterators chapter](../10-data-modelling/10-iterators-and-sequences.md#open-questions--linear-iterators)
and parked in
[`inputs/linear-iterator-two-protocols.md`](../../../inputs/linear-iterator-two-protocols.md).

**Created:** 2026-06-24 · **Decided:** 2026-07-01
**Touches:** `12-memory-and-runtime/08-substructural-types.md` (the affine/
relevant model, §"Iterating affine vs non-affine sources", §"Where `Copy`
fits"), `10-data-modelling/10-iterators-and-sequences.md` (the committed
`next() -> Option[T]` model and §"What Tel does *not* do"),
`12-memory-and-runtime/04-references-and-aliasing.md` (`&!T` suspends-then-
reinstates the owner),
`08-control-flow/04-for-loops-and-iteration.md` (`for` desugaring),
[TIP-0001](0001-mutability-and-borrowing.md) (move/borrow core),
[TIP-0002](0002-untagged-unions-and-sealed-traits.md) (the `More | Done`
result is a union).

## Summary

A **linear iterator** is one that *cannot be polled after it is exhausted*,
enforced by the type system rather than by the fused "`None` forever"
convention. It is the right contract when re-polling a finished source is a
bug worth catching: a one-shot generator, a channel receiver, a read-to-EOF
cursor, a poll-to-`Ready` future.

Tel expresses this with **no new syntax**. The iterator's `next` *consumes*
`self` and hands the continuation back through the return value:

```tel
type Step[T] = More(T, Iter[T]) | Done      # `Done` carries no Iter[T]

fn next(a self: Iter[T]) -> Step[T]          # CONSUMES self
```

Because the terminal variant (`Done`) carries no iterator, once iteration
ends there is nothing in scope to call again — "you cannot poll a dead
source" is a plain use-after-move error, not a convention. `for` hides the
re-threading for the common case, so end users never write it, and combinator
authors encapsulate it once per adapter. An earlier draft proposed sugar (a
`&!self` "resuming borrow" plus a `consume` keyword) to make hand-written
loops terser; it was **rejected** — see the last two sections.

## What Tel says today

- **iterators** commits to the Rust shape: `next() -> Option[T]`, *fused* —
  calling after `None` is legal and yields `None` again. There is no way to
  spell "calling after the end is a compile error". That fused form stays the
  **default** for plain-data walks; the linear form below is the opt-in for
  resources.
- **substructural** §"Iterating affine vs non-affine sources" says the
  borrow/lifetime machinery does not engage for `Alias` sources. That is about
  the *source*'s affinity. This TIP is about the **iterator value itself**
  being a linear resource — a different axis. A plain `List` is `Alias`, so
  `list.iter()` is fused and none of this engages.
- **references-and-aliasing**: a `&!T` borrow suspends the owner for the
  borrow's scope and reinstates it *unconditionally* afterward. That
  unconditional give-back is exactly why a borrowing `next` is fused and
  cannot express the dead end (see §"Why a borrow cannot do this").

So the model is settled for plain data; the only gap is the resource case,
where "fused, re-callable, `None` forever" is the wrong contract.

## The problem precisely

A fused iterator survives its own end:

```tel
fn next(a self: &!Iter[T]) -> Option[T]   # committed model — borrows, survives

while let Some(x) = it.next() { use(x) }
it.next()      # legal. returns None. forever. re-polling a dead source is silent.
```

For a resource-backed source — a channel `rx`, a file cursor, a future polled
to `Ready` — re-polling after the end is meaningless or a bug. We want it to
**not compile**. That requires the exhausted state to be *unrepresentable as a
callable value*: the only way to get "you cannot call `next` again" is for the
end to leave **no iterator in scope**.

## The design — thread ownership through the return

To make the end un-callable, `next` takes ownership and threads the
continuation back through the return. The terminal variant carries **no**
iterator, so after it there is nothing to call:

```tel
type Step[T] = More(T, Iter[T]) | Done      # `Done` carries no Iter[T]

fn next(a self: Iter[T]) -> Step[T]          # CONSUMES self
```

`Step[T]` is structurally `Option[(T, Iter[T])]` — the `unfold` step function
(`State -> Option[(T, State)]`) reified, with `self` moved in and the tail
handed back out. This shape is **forced**, not chosen: ownership entered the
call, and the only channel for it to leave is the return value (you cannot
consume through a borrow — see below).

### Caller side — `for` hides the re-thread

A hand-written loop re-binds the tail each step:

```tel
let uniq it = make_iter()
while let More(x, rest) = it.next() {   # it.next() MOVES it out
    it = rest                            # manual re-wire: rebind to the tail
    use(x)
}
# loop exits only via Done -> `it` was consumed, nothing rebound.
# `it.next()` here is a use-after-move COMPILE ERROR. That is the guarantee.
```

`for` is existing sugar that hides that re-wire; it desugars straight to the
move-threaded loop with no linear-specific machinery:

```tel
for x in src { body }
# ==>
let uniq it = src.into_iter()
loop {
    match it.next() {              # MOVES it
        More(x, rest) => { it = rest; body },
        Done          => break,    # it consumed, no tail -> no live `it` after loop
    }
}
```

So the `it = rest` tax is paid **only** in hand-written loops over a linear
source — rare. End users writing `for x in rx { ... }` see nothing.

### Combinator side — encapsulated once, per adapter

A lazy adapter owns its source and threads the tail through by reconstructing
itself. Users chaining `.map().filter().take()` never see it:

```tel
fn next(self: Map[I, F]) -> Step[U] {
    match self.src.next() {                     # moves self.src out
        More(x, rest) => More((self.f)(x), Map{ src: rest, f: self.f }),
        Done          => Done,
    }
}
```

The one adapter that must make a real decision is an **early-terminating** one
(`take`, `take_while`, short-circuit `find`): it stops while the source may
still hold items, so it must settle the linear tail rather than silently drop
it (the ordinary relevant-binding rule, [substructural]
(../12-memory-and-runtime/08-substructural-types.md); an affine-`Discard`
source is abandoned to GC instead):

```tel
fn next(self: Take[I]) -> Step[T] {
    if self.left == 0 { return { self.src.settle(); Done } }   # drain/settle the tail
    match self.src.next() {
        More(x, rest) => More(x, Take{ src: rest, left: self.left - 1 }),
        Done          => Done,
    }
}
```

## Zero runtime cost

The `Map{ src: rest, .. }` / `Take{ src: rest, .. }` reconstruction moves and
copies nothing at runtime. Because **identity is not observable on values**
(substructural §"Where `Copy` fits"), the backend reuses the same storage and
mutates in place. A consuming `next(self) -> Step[T]` and a fused
`next(&!self) -> Option[T]` compile to **identical machine code**
(mutate-in-place + branch). The only difference is type-level: one forbids the
call-after-end, the other does not. The stronger guarantee is free.

## Why a borrow cannot do this — the consume-through-borrow law

You cannot make `next` take `&!self` and consume on `Done`: a borrow does not
own, so it has nothing to destroy. The moment an operation *can* consume, it
needed ownership at the call site. Hence the receiver is always owning
(`self`); "give it back on the producing branch" can only ride the return
value; the `Option[(T, Self)]` thread is forced. This is why the linear form
is a *different protocol* from the fused borrowing one, not a variant of it.

## Properties that fall out

- **Early exit settles a linear iterator.** A `break`/`return`/`?` out of a
  loop body leaves the reinstated `it` (a *relevant* `Iter`) as a live
  must-use binding — a compile error unless drained or settled. No
  iterator-specific machinery: it is the ordinary relevant-binding rule.
- **Re-polling a dead source is unrepresentable.** After `Done` there is no
  iterator value in scope, so `it.next()` is a use-after-move error — not a
  discouraged pattern, an impossible one.

## Rejected: resuming-borrow sugar

An earlier draft proposed hiding the threading behind sugar: spell `next` as a
`&!self` **resuming borrow** whose reinstatement is *variant-conditional* (the
owner is handed back on producing branches and finalised on branches marked
with a new `consume` keyword), so authors write a borrow-shaped `next` and
hand-written loops drop the `it = rest`. **Rejected**, for three reasons:

1. **It overloads `&!`.** The resuming form *owns* (its desugaring is the
   `self`-threading above) but is spelled as a borrow, which never owns. Two
   opposite ownership contracts behind one sigil, distinguished only by a
   `consume` buried in the body — the reader cannot tell fused from linear at
   the signature. Tel does not let two ownership meanings hide behind one
   spelling.
2. **The tax it removes is already gone.** `for` hides the re-thread for
   users; combinators encapsulate it for libraries. Only a hand-written loop
   over a linear resource pays `it = rest`, which is rare. A keyword serving
   that thin slice does not earn its weight (*one good way over many clever
   ones*, *readability over writability*).
3. **It adds implementation surface.** The sugar requires flow-sensitive
   *revival* of the caller's binding — `it` dead after the call, revived on
   one match arm — a typestate the language otherwise never needs. The
   explicit form uses only ordinary move tracking (bind a fresh `rest`).

The explicit threaded form is kept as *the* linear-iterator design; the
visible `Step[T]`/`More | Done` shape is a feature, not a wart — it makes
"one-shot" legible at the type.

## Prior art

- **Rust** can already *type* this — `fn next(self) -> Option<(T, Self)>` does
  exactly the conditional consumption. It just makes the fused `&mut self`
  model the norm and leaves the linear form an unidiomatic outlier. Tel's only
  change is to bless the linear form as first-class for resources.
- **Session types / linear channels** thread the channel through every step in
  this `(value, continuation)` shape; the linear iterator is that discipline
  applied to iteration.
- **Vale / Austral** higher-RAII and true-linear types give the "must settle"
  half the relevant case leans on (substructural §"Prior art").

## Open questions

- **Two protocols.** Fused (`next(&!self) -> Option[T]`) and linear
  (`next(self) -> Step[T]`) are now visibly *different protocols*. How `for`
  and the combinator family (`map`/`filter`/`take`/…) span both — one
  protocol-generic family, or two — is the remaining decision. Parked in
  [`inputs/linear-iterator-two-protocols.md`](../../../inputs/linear-iterator-two-protocols.md).
- `TODO(open):` does a bound (`LinearIterable` / `OnceIterable`) exist, or is
  "linear" purely a property of a given `next`'s signature? Lean: a property,
  no new trait — mirrors substructural's "one iterator design". Ties into the
  two-protocol question above.
- `TODO(open):` naming of the terminal-settle operation on a relevant linear
  source (`settle` used above as a placeholder) — align with the linear-
  resources chapter.
