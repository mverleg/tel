# Linear vs fused iterators — two protocols (to decide)

**Status:** open input, parked for a later pass.
**Context:** [TIP-0011](../book/src/tips/0011-resuming-borrows-and-linear-iterators.md)
committed linear iterators to the explicit consuming form and rejected the
resuming-borrow sugar. That leaves one thing undecided: Tel now has **two
iteration protocols**, and the `for` construct plus the combinator library
have to span both. This file collects the considerations; it does not decide.

## The two protocols

- **Fused** (the default, for `Alias` / plain-data sources):
  `fn next(a self: &!Iter[T]) -> Option[T]` — borrows, survives its own end,
  re-callable, yields `None` forever after exhaustion.
- **Linear** (opt-in, for resource-backed sources — channels, file cursors,
  poll-to-`Ready` futures): `fn next(self: Iter[T]) -> More(T, Self) | Done` —
  consumes `self`, threads the tail through the return, un-callable after `Done`.

They are genuinely different (the consume-through-borrow law in TIP-0011: a
borrow cannot consume, so the linear form *must* own and thread). This is
deliberate — "one-shot" is legible at the type — but it means shared machinery
must handle both.

## What has to span both

1. **`for`.** Must accept either protocol. Cheap: two desugarings, dispatched
   on which `next` the source's type provides.
   - fused → `loop { match it.next() { Some(x) => body, None => break } }`
   - linear → `loop { match it.next() { More(x, rest) => { it = rest; body },
     Done => break } }`
   - Decide: is dispatch by which `next` exists on the type, by trait bound, or
     by a marker? Lean: by the trait the type implements (see Q on traits).

2. **The combinator family** (`map`, `filter`, `take`, `drop`, `zip`,
   `concat`, `chunk`, …). This is the real cost. Options:

   ### Option A — one protocol-generic family
   Write each combinator once, generic over the protocol. A fused source
   satisfies the linear protocol trivially: its "tail" is itself (reconstruction
   is free because the source is `Alias`), so a linear-shaped `map` degrades to
   the fused one at zero cost. Combinators are authored against the **linear**
   shape; fused falls out as the degenerate case.
   - Pro: one implementation, one mental model, no duplication.
   - Pro: matches "one iterator design" (substructural).
   - Con: needs the type system to express "linear, but this instance's
     give-back is free / the source is `Alias`" so codegen stays fused-cheap.
     Verify the abstraction genuinely compiles to the fused code for `Alias`
     sources (identity-not-observable should make `More(x, self)` a no-op move,
     but confirm).
   - Con: forces every combinator author to write the threaded `More/Done`
     shape even when the vast majority of use is over plain lists.

   ### Option B — two families
   A fused combinator set and a linear combinator set (perhaps linear ones
   suffixed or in a submodule).
   - Pro: each is simple; the fused set reads exactly like Rust/Kotlin.
   - Con: duplication; two names for `map`; the linear set risks bit-rot as the
     less-used path.

   ### Option C — linear-only
   Make *all* iterators linear (invert the committed default). Rejected in
   TIP-0011's parent decision: fused `Option[T]` is the right default for
   plain-data walks and forcing `More/Done` on every list traversal is noise.
   Recorded here only for completeness.

   **Lean:** Option A (one linear-shaped family, fused as the free degenerate
   case) *if* the "give-back is free for `Alias`" story typechecks and codegens
   cleanly. Fall back to Option B if it does not.

## Related open questions to resolve together

- **Trait shape.** Does linear-ness live in a sub-bound (`LinearIterable` /
  `OnceIterable` vs `Iterable`), or is it purely a property of a given `next`'s
  signature with no new trait? TIP-0011 leans "a property, no new trait", but
  Option A above probably needs *some* bound for `for` and combinators to
  dispatch on. Decide the trait shape and the `for`/combinator dispatch
  together — they are the same question.
- **`FiniteIterable` interaction.** `10-data-modelling/10-iterators-and-
  sequences.md` already floats a `FiniteIterable` bound (finite vs infinite).
  Linear-vs-fused is a *third* axis (single-poll-safety), orthogonal to both
  finiteness and source affinity. Make sure the trait design does not conflate
  them: {fused, linear} × {finite, infinite} are independent.
- **Adapter tail-settling.** Early-terminating combinators (`take`,
  `take_while`, short-circuit `find`) over a *relevant* linear source must
  drain/settle the un-consumed tail (see TIP-0011 `Take` example). Confirm the
  combinator signatures make this obligation visible and that it composes
  through nesting (`take` of a `filter` of a channel).
- **`into_iter` / `iter` split.** Fused `iter()` borrows the source; linear
  `into_iter()` consumes it. Nail down which sources offer which, and whether a
  resource offers only the consuming form.

## Decision checklist (for the future pass)

- [ ] Pick A / B for the combinator family.
- [ ] Decide trait shape (new bound vs signature-property) and `for` dispatch.
- [ ] Confirm Option A (if chosen) codegens to fused-cost for `Alias` sources.
- [ ] Specify tail-settling obligation in early-terminating combinator sigs.
- [ ] Keep {fused|linear}, {finite|infinite}, {alias|affine source} as three
      independent axes in the trait design.
