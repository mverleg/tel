# Lifetimes

Tel has a **lifetime concept**, but it is *named, structurally propagated, and
almost never written*. It exists to make a controlled form of borrowing safe —
see [References and Aliasing](04-references-and-aliasing.md) — not to put
`'a`-style annotations into everyday scripts. This page explains the shape of
that concept. The underlying **affine** property that makes borrows necessary
— and its sibling **relevant** property — are covered in
[Substructural Types](08-substructural-types.md).

> The full design lives in [TIP-0001](../tips/0001-mutability-and-borrowing.md)
> (Accepted). This chapter is the migrated reference.

## Why lifetimes exist

Lifetimes are not adopted for their own sake — they are *forced* by another
decision and earn their keep only because of it. The causal chain:

1. **Mutable values must be affine** (no aliasing). Allowing two names to reach
   the same mutable value is the Java pattern, which produces data races and
   "mutated from far away" surprise bugs. Tel rejects it — see
   [the concurrency memory model](../14-concurrency-and-parallelism/07-memory-model-for-concurrency.md).
2. **Affine ⇒ you cannot share by adding a second reference.** That is exactly
   what affine forbids; the only way to "use" an affine value is to *consume*
   the binding (move it).
3. **You often want to use a mutable value without consuming it** — let a
   function read or append and keep using the value after the call. Pure
   move-semantics turns this into return-tuple soup and rules out concurrent
   readers entirely. That ergonomic gap is what **borrows** fill: `&T`
   for non-consuming reads, `&!T` for non-consuming writes (see
   [References and Aliasing](04-references-and-aliasing.md)).
4. **A borrow is a pointer that is not the owner — so it is only valid for
   some scope.** That scope *is* the lifetime. The moment borrows exist,
   lifetimes exist.

Lifetimes are therefore the **price of borrows**, and borrows are the **price
of ergonomic affine mutability**. Strip either out and lifetimes go with them:
take borrows away and affine still works (verbose pass-and-return); take affine
away and you do not need borrows at all (just add another reference).

### Concrete: iterators

The cleanest place this shows up is external iterators.

- **Iterating an immutable collection** — say `List[Int64]`. It is not affine
  (immutable values are freely aliasable), so the iterator just holds another
  reference to the list. No borrow needed, no lifetime needed; refcount/GC
  keeps the list alive as long as the iterator is. Multiple concurrent
  iterators are fine, because nobody can mutate.
- **Iterating an affine mutable collection** — say `!List[Int64]`. You
  cannot "hold another reference"; that is what affine forbids. Without
  borrows you have only bad options: freeze to an immutable snapshot (copy,
  lose in-place building) or move the builder into the iterator (consume the
  original). With a `&!List` borrow held for the iterator's
  lifetime, you iterate without copying and the language statically prevents
  the owner from mutating while iteration is live.

So lifetimes earn their keep on the affine-mutable side. The
immutable-shared side never sees them.

## Lifetimes are named, and rarely spelled

Earlier drafts of Tel had *no* script-visible lifetimes at all, on the logic
that a value-only surface needs none. That has been **reversed**: a bounded
borrow form (`&T`, `&!T`) re-enters the surface language, and a borrow
necessarily has a *scope* — the span over which it stays valid. That scope is
the lifetime. Tel follows Rust's `'a` spelling for it; what Tel rejects is not
the syntax but any *requirement* to write it on the common path:

- **`'a` is spelled like Rust, but rarely written.** A script writes no
  lifetime on the common path — none is declared, threaded through a generic,
  or required to call a borrowing function. When one *is* written, it is the
  familiar `'a`, not a Tel-only invention.
- **Lifetimes propagate structurally**, the same way `Send` is computed from a
  type's fields. A type that stores a borrow inherits that borrow's scope; the
  compiler tracks the relationship without the user stating it.
- **Aggressive elision.** The common cases — a single borrowed input producing a
  single borrowed output, a borrow used only within the call it was passed to —
  are inferred with zero annotation.
- **When elision fails, you write `'a`.** If the compiler cannot decide a
  borrow's scope (e.g. a return that could borrow from either of two inputs),
  it is a compile error — and the fix is a Rust-style lifetime parameter,
  declared in `['a]` and used as `&'a T`, on the borrows that share a scope:

  ```tel
  # elision can't tell which input the result borrows from; name it
  fn merge['a](a: &'a List[Int64], b: &'a List[Int64]) -> &'a Iter[Int64]
  ```

  The same `'a` repeated across parameters, returns, and stored fields is what
  ties them to one scope; it ties two inputs together and relates a view type's
  stored borrow to a constructor input — the jobs `'a` does in Rust, spelled
  the same way. Errors still lead with the *borrow* and its *source* ("the
  returned view could borrow from `a` or `b`") before pointing at the `'a`
  fix. Restructuring or snapshotting remains a valid alternative when you
  prefer it. The full design is in
  [TIP-0001](../tips/0001-mutability-and-borrowing.md#writing-it-down-the-a-escape-hatch).

The practical effect: lifetimes are a **library-author** concern, surfacing when
someone builds an iterator or a view type. A 30-line modding hook *rarely*
mentions one, because the borrow forms it uses are elided end to end. (The
earlier doc said *never*; *rarely* is more honest — a script that stores a
borrow in a record can still hit a scope error.)

TODO(open): the model for borrows stored in records (structural propagation, no
user-written annotation) is settled, but the **"this record outlives its source"
diagnostic** still needs prototyping to confirm the wording reads well —
validation of the message, not of the design. (Migrated from TIP-0001.)

This serves *high abstraction over low-level control* and *readability over
writability*: the borrow story buys local reasoning at function boundaries
without importing Rust's annotation *burden* — the `'a` spelling is the same as
Rust's, but elision keeps it off the common path rather than spreading it
across ordinary signatures.

### Lifetime as an internal notion, too

Independently of the surface borrow scopes, the runtime and IR still reason
about *when* a value's logical life ends:

- **Eager drop** ends a value's life at its last use; see
  [Memory Management](03-memory-management.md).
- **Scope-based lifetime** is one of the per-variable IR metadata fields a
  backend uses to decide stack-vs-heap placement; see
  [Runtime Representation](06-runtime-representation.md).

None of this internal reasoning is what a borrow scope is — but the two share
the IR's lifetime metadata.

## Affine is why lifetimes exist

Lifetimes are the price of borrows, and borrows are the price of *affine*
mutability — a mutable value may not be reached by two names, so you borrow it
instead of aliasing it. The affine property itself, its sibling **relevant**
property, and the `Alias` / `Discard` capabilities that relax them, are
documented in [Substructural Types](08-substructural-types.md). The one fact
this page depends on: a borrow is not a second owner, so it does not break
affinity — it suspends the owner for the borrow's scope, which *is* the
lifetime.

## See also

- [Substructural Types](08-substructural-types.md) — affine, relevant, and the
  `Alias` / `Discard` capabilities.

- [References and Aliasing](04-references-and-aliasing.md) — the `&T` /
  `&!T` borrow forms whose scopes these lifetimes track.
- [Memory Management](03-memory-management.md) — eager drop, bulk heap drop.
- [Runtime Representation](06-runtime-representation.md).
- [TIP-0001](../tips/0001-mutability-and-borrowing.md) — the full mutability /
  borrowing / lifetime proposal.
