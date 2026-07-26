# TIP-0001: Mutability, Borrowing, and Lifetimes

**Status:** Accepted (migrated into the chapter docs 2026-06-13; kept as the
historical design record)
**Touches:** `02-philosophy/04-antifeatures.md`, `06-bindings-and-scope/02-mutability.md`, `10-data-modelling/01-records.md`, `12-memory-and-runtime/04-references-and-aliasing.md`, `12-memory-and-runtime/05-lifetimes.md`, `12-memory-and-runtime/08-substructural-types.md`, `14-concurrency-and-parallelism/07-memory-model-for-concurrency.md`

## Summary

This TIP proposes resolving three intertwined open questions at once:

1. **Mutability model** — `uniq` bindings, copy-update, type-level
   (`ListBuilder` vs `List`), or some combination.
2. **References** — whether Tel has script-visible references at all.
3. **Lifetimes** — whether any lifetime concept is exposed to the script.

The current docs lean *against* references and lifetimes (see
[`12-memory-and-runtime/04-references-and-aliasing.md`](../12-memory-and-runtime/04-references-and-aliasing.md)
and [`12-memory-and-runtime/05-lifetimes.md`](../12-memory-and-runtime/05-lifetimes.md))
and *for* `uniq` as a binding modifier on top of immutable-by-default. This
proposal pivots on one of those positions: **references re-enter the surface
language, in a controlled form**. Lifetimes follow, but in a *named-but-rarely-written*
shape modelled on how Send works in the IR — structural propagation, aggressive
elision, no annotation noise on the common path, with Rust-style `'a` lifetime
parameters surfacing only in the rare case elision cannot resolve.

## Recommended outcome (one-line summary)

- Mutability is **both** a type-level property (mutable types like
  `ListBuilder` vs immutable types like `List`) **and** a binding modifier
  (`uniq`). They cooperate, they do not duplicate.
- A bounded form of **borrowing** is added, written with Rust-style sigils:
  `&T` (read-only) and `!&T` (exclusive mutable lend), with a rare `*`
  deref. Borrows are **values you pass to functions**, not raw addresses; `&`
  means borrow and only borrow. See "Decided: borrow surface" below.
  **(Refined later in this TIP:** the mutable owned type and the exclusive lend
  are respelled with an `!` sigil — `!T` and `!&T` — see
  "Refinement: mutability is a sigil (`!`)".)
- **Lifetimes are named but almost never written.** They propagate
  structurally through types and signatures (Send-shaped). The 95% case has
  zero annotations.
- The 5% case that elision cannot resolve is written with an explicit
  **Rust-style `'a` lifetime parameter** (`&'a T`, declared in `['a]` like a
  type parameter): it ties inputs, returns, and stored fields to one scope.
  Tel deliberately follows Rust's spelling here — the same
  *follow-the-one-mainstream-borrow-language* rationale that settles the `&`
  sigils (below). The only difference from Rust is frequency: aggressive
  elision keeps `'a` off the common path, so the hard cases stay expressible
  and teachable without taxing the 95%.

## Decided: borrow surface is sigils `&` / `!&`, deref `*`

The borrow surface spelling is **settled on Rust-style sigils**, not named
types: `&T` for a read-only borrow, `!&T` for an exclusive mutable borrow,
and a rare, usually-elided `*` for deref. The mutability binding keyword is
`uniq`, not `mut`, and `!&` reuses it. (Earlier drafts of this TIP spelled
these `Readonly[T]` and `Uniq[T]`; the surface is now `&T` and `!&T`
throughout.)

Rationale — *follow the one mainstream borrow language*:

- Rust has trained a large audience that `&` means "borrow", not C's
  "address-of", so the [Swift `inout`-`&`
  confusion](https://belkadan.com/blog/2021/12/Swift-Regret-inout-Syntax/) is
  defused **provided `&` only ever means borrow and never yields a
  pointer/address** — which is Tel's hard rule. The Swift lesson only bites when
  a sigil means *two* things; here `&` means borrow and only borrow.
- Terseness matters at the call and return sites where borrows actually appear,
  and the `*` deref is mostly elided.
- The residual cost — a C reader misreading `&x` as "address of x" — is
  familiarity, not semantics, and is acceptable given the Rust-shaped audience
  and the no-raw-pointers guarantee that backs it
  ([antifeatures](../02-philosophy/04-antifeatures.md)).

## Refinement: mutability is a sigil (`!`), and the five forms

*This section refines two earlier decisions in this TIP. It gives the **mutable
owned type** a sigil `!T` in place of an `XBuilder` name (so `!List`, not
`ListBuilder`), and it replaces the `!&T` borrow spelling with an `!`-sigil
family. The `uniq` **binding** modifier is unchanged; only the **type** and
**borrow** surfaces move to the sigil.*

### Mutability is core enough to be a sigil

`&` already promotes borrowing to a one-character surface. Mutability is just as
core, so it gets the same treatment: a prefix sigil `!`. The immutable type
keeps the bare name — it is the default and by far the most-used — and the
mutable variant is marked:

```tel
Person        # immutable owned — the default, shortest, most common
!Person       # mutable owned ("builder")
```

`!` is chosen because Tel spells logical negation as the word `not`, leaving `!`
free in prefix position; and `!` already *means* "this mutates" across Scheme
(`set!`), Ruby (`sort!`), and Clojure (`swap!`). It is also the thinnest glyph,
so it stays legible jammed against `&` in the borrow clusters below. Candidates
`~`, `^`, `%` were weighed and rejected — `~` has a value-level bitwise-not
clash, `^` carries no meaning, `%` is visually heavy.

Why a sigil beats a word here: a word prefix (`mut Person`) reads ambiguously
before a path — `mut Person::name` looks like `mut (Person::name)`. A sigil
binds tight to its token exactly as `&` does, so `!Person::name` parses cleanly,
the same way `&Person::name` already does.

### The five forms

Two independent bits govern a value: is the **type** mutable (`!`), and — for a
borrow — is the **borrow** exclusive. Spelling both with one sigil works because
the position of `!` relative to `&` carries the distinction:

| form | meaning |
|---|---|
| `Person` | immutable owned |
| `!Person` | mutable owned (builder) |
| `&Person` | shared (aliasing) read borrow |
| `&!Person` | shared read borrow **of a builder** |
| `!&Person` | exclusive **mutable lend** |

Reading rule: **`!` left of `&` makes the *borrow* exclusive-mutable; `!` right
of `&` (on the type) makes the *referent* a builder.**

- `&!Person` — you hold a `!Person` and lend it aliasingly. It can do nothing a
  `&Person` could not, but it is the **only correct spelling**, because coercing
  `!Person → Person` may copy (a builder carries no promise of cheap freezing).
  So "aliasing-borrow a builder" must be written `&!Person`, never `&Person`.
- `!&Person` — the exclusive mutable lend. Semantically this is `!&!Person` (a
  unique borrow *of a builder*): a unique borrow only earns its exclusivity if
  the referent can actually be mutated. A unique borrow of an *immutable* type
  buys nothing, so `!&` of an immutable is **disallowed**, and the redundant
  second `!` is dropped — `!&Person` always means `!&!Person`.

### Why `!&`, not `&!`, for the mutable lend

The order is fixed by lifetimes. A lifetime is a property of the *borrow*, so
`'a` rides the `&`: `&'a` is the borrow-with-lifetime unit. Mutability of the
borrow therefore sits *outside* that unit, to the left:

```tel
!&'a Person     # exclusive mutable lend, lifetime 'a
&'a !Person     # aliasing view of a builder, lifetime 'a
```

This keeps `&'a` together in both forms and makes the left/right `!` rule
visually self-enforcing — you cannot move the `!` to the wrong side without
separating `'a` from its `&`. It mirrors Rust's `&'a mut T` with a better
reading order: `!&'a T` reads left-to-right as "mutable borrow of T."

So the exclusive mutable lend is `!&T`, the aliasing view of a builder is `&!T`,
and `&T` is unchanged. **Decided — `!&` is the single spelling for the mutable
lend; the earlier `&uniq` notation is retired** and has been swept from this TIP
and the memory/runtime chapters.

### Deriving `!T`, and opting out

For the common "same shape, frozen" case — a record whose mutable and immutable
forms differ *only* in mutability — `!T` is **auto-derived** from the `T`
declaration: the same fields, now reassignable, plus the mutating methods.
Nothing is hand-written; bare `T` is the immutable projection.

The freeze is auto-generated too:

```tel
let uniq b = !Person { name = "M", age = 1 }
b.age = 2
let p = b.finish()      # !Person -> Person : zero-copy, consuming, one-directional
```

`finish()` is the one auto-generated conversion (consistent with
[`06-bindings-and-scope/02-mutability.md`](../06-bindings-and-scope/02-mutability.md)).
`snapshot()` — a non-consuming clone-to-immutable that leaves the builder live —
is **opt-in via an explicit derive** (in the same family as `Eq` / `Hash`),
because it allocates. It is never auto-generated; a type that wants it asks for
it.

When the mutable form needs **extra state** the immutable one does not —
`capacity` on a list builder, a partial-construction scratch buffer —
auto-derivation is wrong, and the author **hand-declares `!T`** instead. This is
the old `ListBuilder`/`List` split, respelled: the hand-written mutable type is
`!List`, not `ListBuilder`. The win is that derived `!Person` and hand-written
`!List` are **identical at every use site**; only the declaration differs. So
`XBuilder` as a naming convention is retired in favour of `!X`.

The freeze verb stays `finish()` for both derived and hand-written mutable types
— a convention the stdlib follows (the language cannot enforce it beyond
stdlib).

### Unions: the same machinery, `mut` field → `mut` tag

A union needs no second hand-written declaration for the common case, and its
`Alias` rule is the exact mirror of the record rule. Recall the record rule (see
[`mut` fields and the `Alias` rule](#mut-fields-and-the-alias-rule)):

> a struct is `Alias` iff **all fields are `Alias`-typed** *and* **all fields
> are final** (no `mut`).

The union has the same two axes — payload interior, and re-tagging:

> a union is `Alias` iff **every variant payload is `Alias`-typed** *and* **the
> tag is final** (no in-place re-tagging).

In-place re-tagging — "swap the active variant" — is the union's structural root
of affinity, the mirror of a `mut` field. The argument is identical: swapping
the variant writes tag+payload into shared storage, so a concurrent alias that
already read the old tag mis-reads the new payload (variant/type-confusion).
That forces re-tag to require `!&` (exclusive) access, which requires the
uniqueness proof an `Alias` type has given up. So a re-taggable union cannot be
`Alias`, exactly as a `mut`-field struct cannot. `mut` remains the **single**
structural root of affinity: a re-taggable union is "a union with a `mut` tag."

Two clean cases, no middle:

- **Re-tag not allowed (common):** the union is `Alias` iff all payloads are
  `Alias`. Switching variant is done by copy-update / rebind, which is
  binding-level and needs no mutable union type — so no second declaration.
- **Re-tag allowed (opt-in):** that *is* `!Union` — the in-place-swappable form,
  paired with frozen `Union` and the same auto `finish()`. Unions thus reuse the
  whole `!`/freeze machinery, with "mutate a field" replaced by "swap the
  variant."

### Open questions from this refinement

- **Decided — sigil position is prefix for both `&` and `!`** (`&Person`,
  `!Person`, `!&Person`). The deciding factors: lifetimes are settled Rust-style
  `&'a T`, which prefix keeps intact and postfix would reopen; and the TIP's
  follow-Rust thesis spends its familiarity budget on prefix `&`. The postfix
  alternative (`Person&` / `Person!`) was considered for self-evident
  field-borrow scope (`person.age&`) and uniform trailing modifiers, but that is
  the universe where Rust-style `'a` is also abandoned — not chosen. The
  residual field-borrow ambiguity (`&person.age`) is handled the Rust way, by
  precedence (`.` binds tighter than `&`, so `&person.age` ≡ `&(person.age)`).
- **Decided — the glyph is `!`, and the never type takes a word name.** The
  mutability sigil claims `!`, which removes `!` from the never-type name
  candidates; the never type is **`Never`** (already preferred on the
  familiarity ground, see
  [`05-types/14-never-type.md`](../05-types/14-never-type.md)). Prefix `!` in
  *type* position does not clash with infix `!=` (inequality).
- **Decided — bindings are `let` and `let uniq`; type mutation is `!T`.** The
  division of labour: a binding's uniqueness is declared at the binding (`let b`
  immutable, `let uniq b` unique/mutable), while a value's *mutating methods*
  come from its type (`!Person`). The two compose without collision —
  `let uniq b = !Person { ... }` reads "a unique binding holding a mutable
  Person." (Tentative: `uniq` may later become implicit wherever a `!T` value is
  bound; for now it is written.)
- **Amendment (2026-06-20) — `let` is optional; it is the shadow/modifier
  marker.** A bare assignment `name = value` declares a fresh immutable local;
  `let` is required only to (a) **shadow** an enclosing name or (b) attach a
  **modifier** (`let uniq`). Shadowing is permitted *only* through an explicit
  binding site (`let`, a parameter, a pattern, a loop variable), never through a
  bare assignment — which instead reassigns a current-scope binding, declares a
  fresh local, or is an **error** if the name exists only in an outer scope
  (reassigning an enclosing binding stays explicit via `outer`). This supersedes
  both the earlier "mandatory `let`" lean and the separate `local`
  shadow-escape keyword (now dropped), and reverses the former no-shadowing rule.
  See [`06-bindings-and-scope/04-shadowing.md`](../06-bindings-and-scope/04-shadowing.md),
  [`01-let-bindings.md`](../06-bindings-and-scope/01-let-bindings.md), and
  [`05-scoping-rules.md`](../06-bindings-and-scope/05-scoping-rules.md). The
  `uniq`-implicit-where-`!T` tentative is unaffected.

## Why this is a pivot, and why it is justified

The current docs say: no references → no lifetimes → no aliasing concerns at
the surface. That position is consistent, but it forces every "I want to look
at a builder without taking it" pattern through either materialisation
(snapshot to immutable) or out-of-band conventions. Two consequences pushed
the rethink:

- **Local reasoning at function boundaries is lost.** A signature
  `fn extend(b: ListBuilder, xs: List[Int64])` cannot say whether `b` is
  appended to, replaced, or only read. A reader has to open the body. The
  current `uniq`-as-binding-modifier story does not extend to parameters
  cleanly — if `uniq` is purely local to a binding, callers cannot tell which
  args a callee will mutate.
- **Iteration over mutable structures becomes awkward.** Without any borrow
  concept, iterating a `ListBuilder` while it is being built requires either
  freezing it first (allocation) or trusting an unannotated convention.

Introducing a controlled borrow concept is the smallest change that fixes
both. The alternative — staying value-semantic everywhere and leaning on IR
aliasing analysis — is recorded as **Option A** below and remains a credible
fallback if this TIP's design proves too heavy.

## Design space

Four options, in increasing order of expressiveness and surface cost.

### Option A — Pure value semantics, no references at all

The status-quo direction in the current docs, formalised.

- Functions take values, return new values. No callee mutation of arguments,
  ever.
- Mutability is a **binding** property only (`uniq`). It does not cross
  function boundaries.
- A type can still be "builder-shaped" (operations that return a new value of
  the same type), but the script always writes
  `b = b.push(x)`; the IR collapses this to in-place mutation when aliasing
  analysis proves it safe.
- Local-reasoning question is answered by *the type* of the parameter: a
  `ListBuilder` parameter is a value; whatever the callee does to it is local
  to the callee's frame.

**Pros:** minimum surface area, no new concepts, no annotations to learn,
matches the "embedded scripts, host portability" philosophy most cleanly.

**Cons:** "lend a builder for read access" requires materialisation; advanced
APIs (long-lived iterators, structured views) cannot be expressed; the
local-reasoning win is weaker because the parameter type doesn't distinguish
"will mutate" from "will read."

### Option B — Scope-only borrows, no lifetime concept

Add the minimum-viable borrow: a function argument can be a **borrow** of a
caller's value, valid only for the duration of the call. No storage in
fields, no return of borrows, no lifetime annotations because the only
lifetime *is* the call.

- Two borrow flavours: `&T` (read-only view, multiple coexist) and
  `!&T` (exclusive mutable lend, one at a time).
- Borrows are not first-class values: they cannot be stored in a record or
  returned from a function. They exist only as parameters.
- The caller's binding is statically inaccessible during the call (the
  affine token is on loan).

**Pros:** local-reasoning solved at call sites; no lifetime concept exposed;
syntactically very small.

**Cons:** iterators that outlive their producing call cannot exist; library
authors cannot build view types stored in records; the line between "borrow"
and "value" is sharp and surprising.

### Option C — Borrows with structural propagation, no named lifetimes

Borrows can be stored, returned, and composed. Lifetimes exist as a named
type-system property and propagate structurally (like Send), but there is **no
written lifetime syntax** — when elision cannot decide, the user must
restructure or snapshot.

- `&T` and `!&T` are real types, not parameter-only forms.
- A type that contains a borrow inherits its borrow's scope. The compiler
  tracks this structurally — the user does not write the relationship.
- Function signatures elide: a single-borrow input → single-borrow output is
  inferred. Multi-borrow inputs with potentially-different scopes are a
  compile error whose only fix is to restructure or snapshot — there is no
  annotation to disambiguate. Most signatures never trigger this.
- Iterators, slice-like views, and read-only handles can be built as library
  types, *as long as* their borrow topology is simple enough for elision.

**Pros:** library authors can build the common iterators and view
abstractions; local-reasoning solved; the surface stays completely clean —
there is never a lifetime to write or read.

**Cons:** the elision-failure cases have *no* escape hatch — a legitimate API
that relates two borrows (or ties a stored field to one of several inputs)
cannot be written at all and must be restructured around; and because the
lifetime concept has no written form, users only ever meet it through a
compiler error, which hurts teachability.

### Option D — Option C plus Rust-style `'a` lifetimes (recommended)

Everything in Option C, plus a written escape hatch for the elision-failure
case: a **Rust-style lifetime parameter `'a`**, declared in `['a]` and used as
`&'a T`. It is the *only* addition over C, and it appears only where elision
cannot decide.

- Identical to C on the 95% path: structural propagation, full elision, no
  annotations.
- When elision fails, the user writes `'a` to tie inputs, the return, and
  stored fields to one scope — instead of being forced to restructure.
- The escape hatch makes the APIs C cannot express writeable: returning a
  borrow from one of several inputs, view types whose field is tied to a
  specific constructor argument, functions relating two borrows.

**Pros:** everything C offers, *and* the hard cases stay expressible; the
lifetime concept gains a written form, so it is teachable rather than
encounterable only through errors; matching Rust's `'a` reuses existing
audience familiarity (the same rationale that settles the `&` sigils).

**Cons:** adds *one* piece of surface syntax (`'a`) that a reader may
eventually meet — though aggressive elision keeps it off the common path. This
is the cost C avoids entirely, and the only real difference between C and D.

## Recommendation: Option D, with these constraints

- **Rust-style `'a`, elided on the common path.** If elision fails, the user
  writes an explicit lifetime parameter (`&'a T`, declared in `['a]`) rather
  than being forced to restructure. It is written only when elision cannot
  decide, and is absent from the 95% path. What Tel rejects is not the `'a`
  spelling but any *requirement* to write it where elision succeeds.
- **Lifetimes surface in errors, docs, and (rarely) user-written signatures.**
  Errors lead with the *borrow* and its source — *"borrow of `my_builder` does
  not live long enough to be stored in `the_cache`"* — and only mention `'a`
  when pointing at the written fix, so a reader meets the concept (`borrow`,
  `source`) before the sigil.
- **Structural propagation matches Send.** `T: Borrowed` is computed from
  field types; the user never writes it. Same machinery the IR already needs
  for [`12-memory-and-runtime/04-references-and-aliasing.md`](../12-memory-and-runtime/04-references-and-aliasing.md).
- **Mutability stays one-directional.** Once a value is exposed as immutable,
  no part of the language can re-acquire write access. `!&T` is a
  *temporary* upgrade of an affine token, not a permanent capability.

### Why Options C and D over Option B

Option B is tempting — it sidesteps the lifetime concept entirely — but
the "no iterators" cost is real. Tel already commits to first-class
iterators (see
[`10-data-modelling/10-iterators-and-sequences.md`](../10-data-modelling/10-iterators-and-sequences.md)).
An iterator that borrows from its source collection is the natural shape;
forcing every iterator through a snapshot is either a performance cliff
(materialising large collections) or a semantic cliff (iteration sees a
frozen snapshot, not the live state). Both C and D allow stored, returned, and
composed borrows; B does not.

### Why Option D over Option C

The two options differ on exactly one thing: whether the lifetime concept has a
*written* form. Option C says no, and pays for it twice.

- **Some legitimate APIs become unwriteable.** A library author who builds an
  iterator, a view type, or a function relating two borrows needs to *say* how
  the scopes connect. Under C, "always restructure" is the only answer, and
  some shapes simply cannot be restructured into elision's reach.
- **The concept becomes unteachable.** A lifetime that can only be *inferred*,
  never *written*, is one a user can only ever meet through a compiler error —
  there is no way to show it in a signature, a tutorial, or a doc example.

Option D adds the one missing piece — a written escape hatch — and nothing
else. The common path is byte-for-byte identical to C.

### Why Rust-style `'a` for the escape hatch

Given that a written form is worth having, *how* should it be spelled? Tel
already settles the borrow sigils `&` / `!&` on the
*follow-the-one-mainstream-borrow-language* rationale (below): Rust is the only
mainstream language with lifetimes, and matching it lets a large audience reuse
what they know. Inventing a second, Tel-only spelling for the same concept (an
earlier draft floated a `scope s` keyword) would split that familiarity for no
gain. So the lifetime is written `'a` — declared in `['a]`, used as `&'a T` —
exactly as Rust spells it.

So the difference from Rust is *not* a different spelling and *not* "no
lifetimes." It is **aggressive elision**: named, structural, `'a`-spelled, and
elided by default — surfacing only in the rare case the compiler cannot infer
the connection. Where Rust makes you write `'a` across many ordinary
signatures, Tel's elision removes it from the 95% path while keeping the same
escape hatch when you need it.

## Design sketch (Option D)

### Mutability is both type-level and binding-level

```tel
let my_list = List[Int64](1, 2, 3)              # immutable type, immutable binding
let uniq my_list2 = List[Int64](1, 2, 3)             # immutable type, uniq binding — rebind only

let uniq my_builder = ListBuilder[Int64]()           # mutable type, uniq binding — full mutation
my_builder.push(4)
my_builder.push(5)
let my_frozen = my_builder.finish()           # one-way to immutable List[Int64]
```

- **Mutable types** (`ListBuilder`, `MapBuilder`, channel ends, …) have
  mutating methods.
- **Immutable types** (`List`, `Map`, plain records, …) do not.
- A `uniq` binding is required to call mutating methods or to reassign — a
  non-`uniq` binding to a mutable type is read-only at the *binding* level
  (you cannot reassign), but the type's mutability still matters for whether
  mutating methods exist at all.

### `&T` as a view, not a copy

```tel
fn total(a_builder: &ListBuilder[Int64]) -> Int64 {
    let uniq my_sum = 0
    for an_item in a_builder { my_sum = my_sum + an_item }
    my_sum
}

let uniq my_builder = ListBuilder[Int64]()
my_builder.push(1); my_builder.push(2)
let my_total = total(my_builder)              # auto-borrowed to &ListBuilder[Int64]
my_builder.push(3)                            # ok again — view's scope ended at return
```

- `&T` exposes only the non-mutating subset of `T`'s methods.
- The borrow `T -> &T` is implicit at call sites (covariant in the obvious
  direction).
- While a `&T` view is live, the owner's mutating methods are statically
  refused.

### `!&T` as exclusive mutable lend

```tel
fn extend(a_builder: !&ListBuilder[Int64], a_items: List[Int64]) {
    for an_item in a_items { a_builder.push(an_item) }
}

let uniq my_builder = ListBuilder[Int64]()
extend(my_builder, List(1, 2, 3))             # auto-lend
my_builder.push(4)                            # ok after return
```

- `!&T` is an exclusive lend — only one outstanding at a time, no
  concurrent `&T` either.
- The signature now says exactly which arguments will be mutated. Local
  reasoning at the call site is fully restored.

### Lifetimes propagate structurally, are named in errors

```tel
fn first_word(a_text: &Text) -> &Text {
    # returned slice borrows from a_text; single-input → single-output, elision applies
    a_text.split_once(' ').first
}
```

```tel
type CachedView = record {
    the_source: &ListBuilder[Int64],   # field stores a borrow
    the_filter: Fn(Int64) -> Bool,
}
# `CachedView` is borrow-bound to whatever `the_source` borrows from.
# Compiler tracks this; user writes no lifetime annotation.
# Constructing a CachedView that outlives its source is a compile error
# with a message that names `the_source`.
```

```tel
fn merge_views(a: &List[Int64], b: &List[Int64]) -> &Iter[Int64]
# compile error: ambiguous borrow source for return — add a lifetime, restructure, or snapshot.
```

The error in the third example does not just say *"missing lifetime
parameter"*; it leads with the source — *"the returned view could borrow from
`a` or `b` — these may have different scopes. Tie them with a lifetime `'a`,
snapshot one of them, or return an owned iterator."*

### Writing it down: the `'a` escape hatch

When elision genuinely cannot decide — and only then — the connection is made
explicit with a **Rust-style lifetime parameter `'a`**. It is declared in the
generic-parameter brackets `['a]` (alongside any type parameters) and used on a
borrow as `&'a T` (or `&'a uniq T`); repeating the same `'a` across parameters,
the return, and stored fields is what ties them to one scope. This is exactly
Rust's spelling, and it never appears where elision already succeeds.

```tel
# Ambiguous return, made explicit: the result borrows from `a` and `b`
# together, so all three share lifetime `'a`.
fn merge_views['a](a: &'a List[Int64], b: &'a List[Int64]) -> &'a Iter[Int64]

# Relating two inputs without a borrowed return — both must outlive `'a`.
fn longest['a](a: &'a Text, b: &'a Text) -> &'a Text {
    if a.len >= b.len { a } else { b }
}
```

```tel
# A view type whose stored borrow must be related to a constructor input.
# Structural propagation handles the single-field case with no annotation;
# `'a` is only needed when a field's scope must be tied to something the
# compiler cannot infer (e.g. one of several constructor arguments).
type Window['a] = record {
    the_source: &'a ListBuilder[Int64],
    the_start:  Int64,
    the_len:    Int64,
}
```

This subsumes the narrower `-> &T from a_text` "named source" idea considered
earlier (a single input sharing a scope with the return) without adding a
second syntax: that case is just one input and the return carrying the same
`'a`. The bound on the feature is deliberate — `'a` relates borrow scopes that
are *already in play*; it does not reintroduce raw pointers, pointer
arithmetic, or address-taking. The only departure from Rust is how *often* it
appears, not how it is written.

### Iterators: borrow-free or borrow-bound, one type

A consequence worth stating explicitly, because it drives iterator ergonomics:
**there is one `Iter[T]`, and whether it carries a borrow falls out of structural
propagation — there are not two kinds of iterator.**

An iterator *captures its source*. What that capture is depends only on the
source's substructural classification:

- **`Alias` + immutable source** (e.g. `List`) — the iterator captures an
  owned *alias* of the source, kept alive by refcount/GC. It holds **no
  borrow**, so `Borrowed` computes to false and no lifetime appears. The
  iterator is freely storable in records, returnable, and `Send`-able.
- **affine / mutable source** (e.g. `ListBuilder`) — the iterator cannot take
  a second owner, so it captures a `&T` **borrow** and structurally
  inherits that borrow's scope. While that `&T` view is live, no `!&`
  may coexist, so the source provably cannot mutate mid-iteration — which is
  exactly the iterator-invalidation guarantee.

```tel
fn iter(a self: List[T])             -> Iter[T]   # self is a cheap alias; result borrow-free
fn iter(a self: &ListBuilder[T])     -> Iter[T]   # result borrow-bound to the lend
```

The return type is spelled `Iter[T]` in both cases; the borrow scope rides
along invisibly via the `T: Borrowed` machinery and is simply absent in the
`Alias` case. The borrow-free iterator is the *simpler special case* of the
general borrowing one — not a separate flavour. The full treatment lives in
[`12-memory-and-runtime/08-substructural-types.md`](../12-memory-and-runtime/08-substructural-types.md#iterating-affine-vs-non-affine-sources).

The composition payoff is concrete: borrow-free iterators never trigger the
"ambiguous borrow source" return error (see `merge_views` above), so `zip` /
`chain` over immutable collections compose without restriction; the same
operations over builders engage the borrow rules.

### Function types

Affinity (captures consumed on call) forces *one* split in function types.
Mutation does *not* force a separate one, because it is already handled by
the borrow system. The result is two function-type forms, not Rust's three.

- `Fn(...) -> T` — callable any number of times. The default. No constraint
  on how often the value is invoked.
- `FnOnce(...) -> T` — callable at most once. Inferred for any closure that
  consumes an affine capture (or whose captures recursively contain an
  affine value).

There is no separate `FnMut`. A closure that captures `!&T` is still
`Fn(...)`; the constraint that mutation requires exclusive access is enforced
at the call site by the existing borrow rules. The closure can be invoked
repeatedly as long as the captured borrow is still live and unaliased.

```tel
fn each(a_seq: List[Int64], a_fn: Fn(Int64) -> ()) { ... }     # called many times
fn run(a_callback: FnOnce() -> ())          { ... }       # called at most once

let uniq my_builder = ListBuilder[Int64]()
each(my_list, |x| my_builder.push(x))         # captures !&ListBuilder;
                                              # still `Fn(...)`, not `FnOnce`
```

Inference: a closure literal's function type is `Fn(...)` unless analysis of
the body finds that calling it twice would re-use an affine value, in which
case it is `FnOnce(...)`. Users rarely write `FnOnce` themselves; they write
it in signatures that *require* one-shot callbacks, and the inference fills
in the rest.

A `Fn(...)` is a subtype of `FnOnce(...)` (every multi-callable is also a
once-callable), so passing a `Fn(...)` where `FnOnce(...)` is expected is
fine.

**Why this is not "function colouring."** The split is not an effect that
infects every container of a function value; it is a property of the
function value itself, computed from its captures. A `List[Fn(Int64) -> Int64]`
is one type; a `List[FnOnce(Int64) -> Int64]` is another, but neither is a new
*colour* of the language — the `FnOnce` marker behaves like the existing
borrow-source propagation, not like `async`.

### Interaction with concurrency

Borrows are fiber-local. A borrow never crosses a task boundary — sending a
value to another task is the existing deep-copy hand-off
([`14-concurrency-and-parallelism/07-memory-model-for-concurrency.md`](../14-concurrency-and-parallelism/07-memory-model-for-concurrency.md)),
and the deep-copy machinery refuses to copy a `&T` or `!&T`. This
preserves the per-fiber heap isolation property unchanged.

A function type may pick up a `Send` bound (`Fn(...) -> T + Send`) when it is
required to cross a task boundary — same shape as Rust, structurally
inferred where possible. Send is independent of `FnOnce`; the two compose.

### Interaction with `with` (copy-update)

`with` continues to work as today for immutable types. For mutable types, the
preferred mutation path is the type's own mutating methods through a `uniq`
binding (or a `!&T` parameter); `with` on a mutable type is allowed but
returns a fresh value (it does not mutate in place).

## Substructural model: affine *and* relevant

An earlier draft of this TIP adopted only the **affine** half and rejected
**relevant** (must-be-used) typing on the "type colours" cost argument. That
is **reversed**: Tel adopts both, with every type **linear (affine + relevant)
by default** and two structurally-derived opt-out capabilities — `Alias`
(relax affine) and `Discard` (relax relevant). The full treatment, including
the private-destructor "use" mechanism and the `AutoUse` ergonomic, lives in
[`12-memory-and-runtime/08-substructural-types.md`](../12-memory-and-runtime/08-substructural-types.md).

Two points matter for *this* TIP specifically:

- **Affinity still drives the borrow system** exactly as described above: a
  `!&T` lend is an affine token, mutable values may not alias, and the
  `Fn` / `FnOnce` split falls out of affine captures. None of that changes.
- **Relevance replaces the must-handle/structured-only story for handles.**
  A task handle and a `Result` are simply **linear** (they do not derive
  `Discard`), so dropping one unused is a compile error — the same guarantee
  the old "must-handle discipline" gave, now uniform with every other
  must-use resource. Structured concurrency's auto-join is the *mechanism*
  that uses (joins) a child handle at scope exit, i.e. its `AutoUse`.

The cost the earlier draft feared — relevance forcing a *recovering* unwind with
destructors on every failure path — does not arise, because Tel **aborts without
recovering**: relevance is a compile-time obligation on the normal path, and on
abort a NoPanic cleanup unwind settles live linear resources (running
`AutoUse`/`finally`) before the task's heap is dropped — no catch, no resume. See
the substructural topic for the full reconciliation.

### `mut` fields and the `Alias` rule

`Alias` is **derived**, not declared, but the derivation has *two* conditions,
not one. A field contributes mutability along two independent axes:

- **Interior mutation** — calling the held value's mutating methods — is
  decided by the **field's type** (`ListBuilder` yes, `List` no). No annotation
  carries it; the type already does.
- **Reassignment** — repointing the slot at a different value — is **not** a
  property of the field's type. It is marked with **`mut` on the field**:
  `mut` means the slot is reassignable, the exact opposite of Java's `final`.
  It is **shallow**: `mut the_xs: List` makes the *slot* repointable, it does
  **not** make `List`'s contents mutable. The default (no `mut`) is final.

Reassigning a `mut` field, like calling any mutating method, requires `!&`
access to the containing value; with a plain `&` (read-only) borrow neither is
possible.

This gives the corrected derivation rule — **state it precisely**:

> A type is **`Alias`** iff **all of its fields are `Alias`-typed *and* all of
> its fields are final (no `mut`)**.

The finality half is essential and was easy to miss: a reassignable field needs
`!&` to be written, granting `!&` needs the compiler to *prove*
exclusivity, and that proof is exactly the uniqueness tracking an `Alias` type
has given up. So a `mut` field can never actually be reassigned on an `Alias`
value — meaning a type with any `mut` field cannot soundly be `Alias`. Having
only `Alias`-*typed* fields is not enough; the fields must also be final.

Consequently **`mut` is the single structural root of affinity**: a type is
affine (¬`Alias`) iff it transitively contains a `mut` field. "Interior-mutable
type" (`ListBuilder`) just means "type that transitively reaches a `mut` field"
— the primitive mutable cell is literally `record { mut the_value: … }`. The
**one** explicit exception is a synchronised stdlib primitive that carries a
`mut` field yet still implements `Alias`, because its internal lock makes
concurrent reach safe; only stdlib can construct it.

```tel
# All fields final and Alias-typed → Point derives `Alias`, freely shared.
type Point = record { the_x: Int64, the_y: Int64 }

# A `mut` field → Counter is affine (¬`Alias`); the slot is reassignable.
type Counter = record { mut the_n: Int64 }

fn bump(a self: !&Counter) { self.the_n = self.the_n + 1 }   # reassign needs !&

let uniq my_counter = Counter(the_n: 0)
bump(my_counter)              # ok — exclusive access

# `mut` is shallow: the slot may be repointed, the List stays immutable.
type Log = record { mut the_entries: List[Text] }   # affine; List contents not mutable
```

The borrow-storing view types shown earlier (`CachedView`, `Window['a]`) are
consistent with this rule: their fields are **final** (no `mut`), so the only
reason they are not `Alias` is the borrow they store, tracked by lifetime
propagation — not a reassignable slot.

`mut` is spelled `mut`, not `uniq`, on purpose. `uniq` on a *binding* means
"this local is rebindable *and* I hold exclusive access"; `mut` on a *field*
means only "this slot is reassignable" — a structural layout fact, Java's
non-`final`. They answer different questions, so they get different keywords;
this is why `mut` is fine on a field even though TIP-0001 rejects it as a
binding modifier.

## What this TIP does *not* do

- **No interior mutability.** No `RefCell`-style escape. The
  one-directional mutability rule from
  [`06-bindings-and-scope/02-mutability.md`](../06-bindings-and-scope/02-mutability.md)
  stands.
- **No raw addresses or pointer arithmetic.** Borrows are values, not bytes.
  The "no low-level machine access" antifeature stands.
- **No async coloring.** `&T` and `!&T` do not infect generic containers
  the way `async fn` would — they are ordinary borrow forms, not effect markers.
- **No mandatory lifetimes, and no raw pointers.** Lifetimes are written
  Rust-style `'a` only when elision fails (see "Recommendation" and the design
  sketch); they are never required where elision succeeds, so they stay off the
  95% path. The `'a` is a scope relator, not a pointer — it adds no raw
  addresses, no pointer arithmetic, and no address-taking.
- **No `FnMut` distinction.** Mutation through captures is handled by the
  borrow system on the captures themselves. See "Function types" above.

## Open questions

- **Accepted direction — borrows in records use structural propagation, with no
  user-written annotation.** The design is settled; the remaining work is
  prototyping the "this record outlives its source" diagnostic to confirm the
  error messages are readable. Only the wording needs validation, not the model.
  *Migrated to [`12-memory-and-runtime/05-lifetimes.md`](../12-memory-and-runtime/05-lifetimes.md).*
- **Decided — `&T` and `!&T` are built-in borrow forms.** No `#[readonly]`
  method attribute is needed: the read-only-vs-mutating split is already carried
  by the `!T`/`T` type distinction. A method declared on the immutable type `T`
  is non-mutating by construction and is exactly what a `&T` borrow exposes;
  mutating methods live on `!T`. There is nothing left for a per-method marker to
  decide, so the earlier `#[readonly]` idea is dropped.
- ~~TODO(open): escape hatch when elision fails on returns.~~ **Decided —
  Rust-style `'a` lifetime parameters.** The earlier candidates were (a) force
  restructure/snapshot with no syntax, and (b) a single-source
  `-> &T from a_text` form. Resolved in favour of full **Rust-style `'a`**: a
  lifetime parameter declared in `['a]` and used as `&'a T`, which ties two
  inputs to one scope, relates a stored field's scope to a constructor's
  inputs, and subsumes the `from a` case. Rationale: a written form is needed
  to express legitimate library APIs and to make the concept *teachable*; and
  matching Rust's spelling (the one mainstream lifetime language) reuses
  audience familiarity, the same rationale that settles the `&` sigils. Still
  elided on the 95% path — the difference from Rust is frequency, not
  spelling.
- **Decided — auto-deref for read-only method calls on `&T`.** The user writes
  `b.size`, not `b.value.size`; there is no ambiguity, because mutating methods
  are not reachable through `&T`. This matches Rust, which inserts auto-ref /
  auto-deref (and `Deref` coercions) at method-call sites so a `&self` method is
  callable on a borrow without an explicit `*`.
- **Decided — freeze is a library convention with compiler help.** Deriving `!T`
  from `T` auto-generates a consuming `finish(): !T -> T`. A hand-written `!T`
  (the opt-out, for builders that need extra state) supplies its own freeze and
  follows the `finish()` naming convention by discipline — the stdlib does, but
  the language does not enforce the name beyond stdlib. See
  [`06-bindings-and-scope/02-mutability.md`](../06-bindings-and-scope/02-mutability.md).
- **Decided / confirmed — this TIP enables option (b)** of the data-race-safety
  question (separate mutable/immutable types, mutable values affine), leaving
  option (a) (Rust-style ownership of arbitrary values) off the table. Confirmed
  against [`02-philosophy/04-antifeatures.md`](../02-philosophy/04-antifeatures.md),
  which now states the same.
- **Decided — call-arity follows capture consumption; capture mode is inferred
  with an optional `move`/`borrow` override.** A closure is `FnOnce` iff a call
  **consumes** (moves out) one of its captures; otherwise it is `Fn`. Capture
  mode is inferred (immutable → value snapshot; `uniq` → reusable `!&` borrow,
  hence `Fn`), and an optional, overrides-only **capture clause** lets the author
  write `move <name>` / `borrow <name>` — keywords, not sigils — both to document
  intent and to force a move (which, if the body then hands the value away,
  yields `FnOnce`). This is the same inferred-but-writeable pattern as `'a` and the
  `Send` bound. See
  [`09-functions/06-closures-and-lambdas.md`](../09-functions/06-closures-and-lambdas.md).
  *Migrated to [`05-types/05-function-types.md`](../05-types/05-function-types.md).*
- **Decided — require a capability with `: Alias` on the type declaration**, the
  same capability-floor spelling that [TIP-0002](0002-untagged-unions-and-sealed-traits.md)
  gives unions (`(A | B) : Trait`). An author who intends a type to stay freely
  shareable writes `type Person : Alias = record { ... }`; the compiler checks
  that the derivation still yields `Alias` and **errors if a later edit breaks
  it** (e.g. adding a `mut` field or a non-`Alias` field). It is a pure
  compile-time assertion on a derived property — no runtime cost, no unit-test
  scaffolding. The rejected alternative was a standalone static check
  (`fn check(const _: Alias); check(Person)`), which would need a
  `const`-parameter / compile-time-value mechanism Tel does not have. The same
  `: Capability` spelling generalises to asserting `Discard` / `Send` / `Sync`.
  *Migrated to [`12-memory-and-runtime/08-substructural-types.md`](../12-memory-and-runtime/08-substructural-types.md).*
- **Decided — the `Send` bound on function types is writeable.** `Fn(...) -> T`
  still carries an inferred `Send`/`not Send` from its captures on the common path,
  but the user can *write* `Fn(...) + Send`. This is required, not cosmetic: a
  signature that hands a callback to another task has no other way to state
  "this function must be `Send`" — inference cannot demand a bound the caller
  must satisfy. Inference fills it in where omitted; the written bound is how an
  API *requires* it.
- ~~TODO(open): Borrow surface spelling — sigils vs. named types.~~ **Decided —
  see "Decided: borrow surface" above.** Sigils win: `&T`, `!&T`, rare `*`.

## Migration plan — completed 2026-06-13

All steps below are **done**; the chapter docs are now the authoritative
reference and this TIP is the historical record.

1. ✅ `02-philosophy/04-antifeatures.md` — the "no references" stance is gone and
   the lifetime line reads "elided on the common path, Rust-style `'a` only when
   elision fails"; the mutable-type spelling is `!List`.
2. ✅ `12-memory-and-runtime/04-references-and-aliasing.md` — describes `&T` /
   `!&T` and the five `!`-sigil forms as the surface borrow forms; the IR
   aliasing-classification section is kept.
3. ✅ `12-memory-and-runtime/05-lifetimes.md` — scope-based structural
   propagation, aggressive elision, and the written Rust-style `'a` escape hatch.
4. ✅ `06-bindings-and-scope/02-mutability.md` — the type-level `!T` half and how
   `uniq` bindings lend as `!&T` parameters.
5. ✅ Instead of a redundant new data-modelling "borrows" topic (12/04 already
   covers `&T` / `!&T`), the borrow-as-type material landed where it belongs:
   the `!T` mutable record form and `mut` fields in
   `10-data-modelling/01-records.md`, the `!Union` re-tag mirror in
   `10-data-modelling/02-union-types.md`, and the two-part `Alias` rule in
   `12-memory-and-runtime/08-substructural-types.md`.
6. ✅ The affected chapters cross-link to this TIP for historical context;
   `14-concurrency-and-parallelism/07` and `…/09` had their Draft/mutability
   TODOs resolved, and the `XBuilder` naming was swept to `!X` repo-wide.

The three questions that remained genuinely open were migrated to their owning
chapters (see the *Migrated to …* notes in "Open questions" above): the
record-borrow diagnostic wording → lifetimes; `FnOnce` inference for `!&T`
captures → function types; requiring `Alias` on a type → substructural types.
