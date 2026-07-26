# Mutability

<!-- TODO: review -->

Tel values are **immutable by default**. Mutation exists, but it is explicit,
narrowly scoped, and visible in the source. The maxim is *mutability makes
things easier, but its scope should be minimized*
([maxims](../02-philosophy/02-maxims.md)).

## What

There are two ways to "change" data in Tel, and they are deliberately
different:

1. **Copy-with-update** — produce a new immutable value from an old one,
   changing some fields. This is the *default* way to model change.
2. **Explicit `uniq` bindings** — a binding marked `uniq` may be reassigned, and
   data reached through it may be mutated in place.

### Copy-with-update: `with`

Most "modification" in Tel does not mutate anything. It builds a fresh value:

```tel
let base   = Settings { volume = 5, muted = false }
let louder = base with { volume = 8 }
# `base` is unchanged; `louder` is a new Settings value.
```

`with` is covered in detail under
[Destructuring and Object Construction](06-destructuring.md); it also works
*between* types, copying same-named fields. For small scripts and for crossing
the host boundary this is the preferred model — immutable values are trivial to
share, cheap for a host runtime to manage, and easy to compile down to mutable
target languages (the reverse does not hold).

### Explicit `uniq`

When a genuinely mutable variable is the clearest tool — a running total, a
collection being built — the binding is marked `uniq`:

```tel
let uniq tally = EuroAmt(0)
for an_item in a_cart {
    tally = tally + an_item.price
}
```

A binding without `uniq` cannot be reassigned. Assigning to it is a compile
error, not a silent shadow.

### Assignment is a statement, not an expression — no `a = b = c`

A reassignment is a **statement** and yields no value, so **chained assignment
`a = b = c` is rejected**. One assignment per statement; assign separately if
you need to set several names. Two reasons:

- *Readability.* A chain hides which name receives which value, and invites the
  reader to wonder whether `a = b = c` means "set both to `c`" or something
  subtler.
- *Move/ownership surprise.* On move-only (`Copy`-less) types, `a = b = c` would
  silently move `c` into `b` and then `b` into `a`, leaving `c` unusable and `b`
  ambiguous about whether it was reassigned. Tel does not want that trap.

```tel
# REJECTED — chained assignment
a = b = c

# write it out
b = c
a = b
```

The keyword names **unique ownership**, not mutation. A `uniq` value is one no
other name aliases — which is *why* it is safe to mutate in place and to *move*
(not copy) across a
[task boundary](../14-concurrency-and-parallelism/07-memory-model-for-concurrency.md).
Mutability follows from uniqueness, so the keyword leads with the property that
licenses it rather than with the consequence. The deliberate result: a value
that mutates through *shared* access — a stdlib concurrent type — is **not**
`uniq`, because it is shared, not unique (see the next section).

## Transitive mutability

`uniq` is **transitive**, as in Rust. Mutability is a property of the *access
path*, not just the outermost name:

- Through a `uniq` binding you may reassign the binding *and* mutate the data it
  reaches.
- Through a non-`uniq` binding you can do neither — you cannot reach "around" an
  immutable binding to mutate something nested inside it.

There are **no interior-mutability escape hatches** in user code (no
`RefCell`-style cell that is mutable through an immutable reference). If a value
is reachable as immutable, it is immutable, full stop. The *only* shared-mutable
values are stdlib concurrency types (`Mutex`, atomics, a concurrent map; see
[locks and concurrency primitives](../14-concurrency-and-parallelism/10-locks-and-concurrency-primitives.md)),
which synchronise internally and are neither `uniq` nor immutable — a sealed
exception, not a capability user types can claim.

## Once gone, mutability cannot return

A value's mutability is **one-directional**: it can be given up but never
regained.

```tel
let uniq builder = !List[Int64]()
builder.push(1)
builder.push(2)
let frozen = builder.finish()   # frozen is an immutable List[Int64]
# There is no operation that turns `frozen` back into a mutable builder.
```

Once a value is frozen (or only ever reachable immutably), nothing can mutate
it again. This makes immutability a durable guarantee a reader and the compiler
can rely on:

- An immutable value can be shared freely, including across tasks, with no risk
  of a race — see [concurrency](../14-concurrency-and-parallelism/).
- A mutable value generally cannot be shared between tasks; it must be frozen
  or deep-copied first.
- Only an immutable value is **hashable**: a `uniq` value is not a valid `Map`
  key or `Set` member, because a key whose hash could shift after insertion
  corrupts the container. Freezing it first (as above) yields a key with the
  usual derived hash — see
  [Equality and Hashing](../10-data-modelling/07-equality-and-hashing.md#hashing-needs-immutability).

## Two axes: ownership and reassignability

Two *orthogonal* concerns are easy to conflate, and Tel keeps them apart
(settled in [TIP-0001](../tips/0001-mutability-and-borrowing.md)):

- **Ownership — the safety axis.** *May this value be mutated at all?* Governed
  by **affineness** and spelled with the prefix `!` sigil: `!T` is the
  **affine** form (reachable by one owner, moved not copied), bare `T` is the
  **shareable** (`Alias`) form (the default — shortest, most common; immutable
  for user types, the sealed synchronised-stdlib exception aside). You may
  mutate a `!T` *in place* precisely because you own it uniquely, so no other
  name can observe a torn update. At the binding level the same property is
  spelled `uniq` ([above](#explicit-uniq)); `!` is its type-level statement.
- **Reassignability — the data-model axis.** *Is this slot meant to change?*
  Per-field, marked `mut`, **final by default** — Java's `final` with the
  polarity flipped. It is a statement of design intent, not a safety property:
  owning a `!T` already makes mutation *safe*; `mut` records whether it is
  *intended*.

The two compose — reassigning a field needs **both** an owned `!T` value **and**
the field marked `mut`. Neither subsumes the other.

```tel
record !Person { mut age: Nat, name: Text }

let p = Person { name = "M", age = 1 }        # frozen projection: shareable, all final
let uniq q = !Person { name = "M", age = 1 }  # affine value, owned through a uniq binding
q.age  = 2     # ok    — q is an owned !Person and `age` is `mut`
q.name = "N"   # ERROR — `name` is final by default, even though you own q
```

### Why `!` means *owned*, not *mutates*

It is tempting to read `!` as "mutates" — it connotes that across Scheme
(`set!`), Ruby (`sort!`), Clojure (`swap!`). In Tel it names the property that
*licenses* mutation instead: **affineness (unique ownership)**, the same choice
that named the binding keyword `uniq` rather than `mut`. Leading with ownership
is what makes the sigil correct at the edges:

- A **shared interior-mutable** type — `Mutex`, `RwLock`, an atomic — *mutates*
  but is **not** affine: many names reach it and it synchronises internally. It
  is `Alias`, so it wears **no `!`** (the sealed stdlib exception from
  [above](#transitive-mutability)). Spelling it `!` would be a lie.
- An **immutable but owned** resource — a handle you must `close` but never
  reassign — *is* affine, so it **is** `!Handle`, even though no field is `mut`.
  `!` marks that you own it and it moves; the absence of `mut` fields just says
  there is nothing to reassign.

`!` still binds tighter than a word would — `!Person::name` parses cleanly,
where `mut Person::name` would read as `mut (Person::name)`.

`!T` values are **affine** — reachable by at most one binding, moved not copied
— which is mechanism (b) of the data-race-safety question in
[antifeatures](../02-philosophy/04-antifeatures.md) and the subject of
[substructural types](../12-memory-and-runtime/08-substructural-types.md).

### Declare `!T`; the freeze `T` is derived

The affine type is the one you **declare**; its immutable, shareable projection
`T` is **auto-derived** as the *freeze*. Declaring `record !Person { … }` gives
you `!Person` and, for free, a `Person` that is the same fields frozen — all
slots final, freely shareable. The derivation runs `!T → T`: give up unique
ownership, gain shareability.

This is the reverse of the builder you might expect to hand-write: you do not
write the immutable record and bolt a mutable twin on; you write the owned form
and the frozen view falls out. When the owned form needs **extra state** the
frozen one does not — a `capacity`, a scratch buffer — it lives on the `!T`
declaration and the derived `T` drops it. That is the `List`/`!List` case:
`!List` carries growth state, `List` is its freeze.

The `mut` markers ride along: in the owned `!T` a `mut` field is reassignable
(when you hold it uniquely), a non-`mut` field is final; the derived `T` freezes
everything. A type with **no** `mut` field is still a perfectly good `!T` — an
owned, move-only value you cannot reassign but can consume (the resource-handle
case above).

The freeze is a **library convention with compiler help**: the derivation
auto-generates a consuming `finish(): !T -> T` (zero-copy, one-directional). A
non-consuming `snapshot()` — clone-to-immutable that leaves the owned value live
— allocates, so it is **opt-in via an explicit derive** (the `Eq`/`Hash`
family), never automatic.

### Three declaration shapes

Which projections exist is read off *what you declare*. In all three the sigil
is constant — `!` always means owned/affine, bare always means shareable and
immutable — so a reader never meets an ambiguous name:

| You declare | `T` (frozen) | `!T` (owned) | Use |
| --- | --- | --- | --- |
| `record T { … }` | yes | no | pure data with no owned form — the default |
| `record !T { … }` | yes (derived freeze) | yes | the common owned-then-frozen pair — `!List`/`List` |
| `record !T { … }`, freeze suppressed | **no** | yes | a value that is *never* frozen — a live resource |

The first needs no ceremony: a plain record is shareable and immutable. The
second is the workhorse — declare the owned form, get the freeze for free. The
third is the **freeze-suppressed** (mutable-only) case: a live socket, an OS
handle, a cursor, where an immutable snapshot is meaningless. Suppressing the
derived `T` makes bare `T` a name error, not a dead snapshot:

```tel
record !Connection { mut the_fd: Int32 }   # freeze suppressed; bare `Connection` is undefined
```

Two consequences follow from there being no `T`:

- **No `finish()`.** With no `T` to land in, the freeze derivation does not run;
  the type supplies a `finish()` targeting some *other* type, or none at all.
  "This never becomes shareable" is exactly the intended message.
- **Affine, often linear.** Being `!`, it is affine; a live resource also wants
  to be *relevant* (no `Discard`, must be used/closed), making it
  [linear](../12-memory-and-runtime/08-substructural-types.md) — one owner, must
  be released. That must-use half is a *separate* marker; `!` carries only the
  ownership half (see below).

Never invert the sigil to make a bare name mean owned — that would cost the
local readability the `!` exists to provide (and that the
[IDE renders](../18-tooling/12-language-cues-for-the-ide.md)).

`TODO(open):` the **spelling of freeze-suppression**. Declaring `record !T`
auto-derives the freeze `T` by default; the resource case needs an explicit
opt-out (an attribute, or implied by the type being `relevant`/linear). Not yet
chosen.

### What `!` does *not* carry

`!` is one axis — ownership (affineness, `¬Alias`). Two neighbours are kept
*separate*:

- **Reassignability** is the per-field `mut` axis above. Owning a `!T` makes
  mutation *safe*; `mut` is what makes a given slot *intended* to change. A `!T`
  whose fields are all non-`mut` is owned and move-only but reassigns nothing.
- **Must-use** is the `Discard` axis: a value that must be consumed before it
  leaves scope is *relevant* (no `Discard`), spelled with its own marker, not
  `!`. A linear resource is *both* affine (`!`) and relevant; `!` carries only
  the first half. See
  [substructural types](../12-memory-and-runtime/08-substructural-types.md).

### `uniq` bindings and `&!T` parameters

A `uniq` binding licenses mutation *locally*. To let a **function** mutate a
value without consuming it, the value is lent as an exclusive mutable borrow,
written `&!T` (read-only lends are `&T`). The borrow forms and their scopes are
covered in
[References and Aliasing](../12-memory-and-runtime/04-references-and-aliasing.md);
the interaction with bindings is:

- Calling a `&!T` parameter requires a `uniq` binding at the call site — you
  cannot lend exclusive write access you do not hold.
- While the borrow is outstanding the caller's binding is statically
  inaccessible; it is reinstated when the call returns.

```tel
fn extend(a_builder: &!List[Int64], a_items: List[Int64]) {
    for an_item in a_items { a_builder.push(an_item) }
}

let uniq my_builder = !List[Int64]()
extend(my_builder, List(1, 2, 3))   # auto-lent as &!List; my_builder usable again after
my_builder.push(4)
```

The signature now states exactly which arguments a callee mutates, restoring
local reasoning at the call site.

TODO(open): **building immutable graphs with cycles.** A cycle needs a node
reachable by two paths — i.e. aliasing — but a `uniq` (affine) value is
uniquely owned, so a `uniq`-build-then-`finish()` is always a *tree* and can
never close a loop. The difficulty is at *construction*, not at the freeze
(freezing preserves whatever structure exists). A genuinely cyclic immutable
structure (a state machine, a mutually-recursive grammar) therefore needs
either a `rec`/letrec knot-tying form, or construction through shared-mutable
`Sync` cells. Whether Tel offers `rec` is undecided; if it offers neither,
immutable cycles cannot be built and the only cycles in a program come from
stdlib `Sync` types — which simplifies
[memory reclamation](../12-memory-and-runtime/03-memory-management.md). The
portable workaround that needs no cycles at all is ID-indirection: nodes hold
`Id`s and a `Map[Id, Node]` resolves them.

## Why

- **Immutability compiles down, mutability does not.** A key selling point: an
  immutable Tel program maps cleanly onto a mutable host language; the reverse
  is hard. Defaulting to immutable keeps Tel portable across very different
  host runtimes.
- **Reproducibility and safety.** Less shared mutable state means fewer of the
  bugs strict typing cannot otherwise catch, and it is the foundation of
  data-race safety.
- **Readable diffs.** `base with { volume = 8 }` says exactly what changed;
  scattered in-place assignments do not.
- Mutable bindings still earn their place for building collections and
  accumulating results — but their scope stays small and visibly marked.

## In-place mutation, by example

The recurring production-bug shapes this design is meant to prevent (see
[`../12-memory-and-runtime/01-value-vs-reference-semantics.md`](../12-memory-and-runtime/01-value-vs-reference-semantics.md)
for the value-semantics half of the story):

- **Mutating a value still held by an upstream consumer.** An interpolation or
  "move" step rewrites its input directly. A later step persists what it
  *thinks* is the original input. The persisted record no longer matches what
  the downstream computation actually saw. In Tel, `with`-style copy-update is
  the default and an in-place mutation requires an explicit `uniq` binding
  visible at the source — both sides see this.
- **Optional-step unit conversion that sometimes mutates the input.** An
  optional copy-and-update step normally returns a fresh value; when disabled,
  it returns its input by reference, and the next step's "in-place unit
  conversion" silently rewrites the caller's data. Sound boundary code uses
  copy-update unconditionally; mutation-when-disabled is exactly the surprise
  Tel's immutable-by-default rule removes.
- **Mutation of a shallow-copied API input.** A "shallow copy" passes a
  collection but not the elements inside it, leaving the receiver looking at
  the same mutable lists the sender keeps editing. Because `uniq` is
  [transitive](#transitive-mutability), there is no way to expose a "shallow
  copy" that is immutable at the top but mutable underneath: the whole reach
  is either mutable or it isn't.
- **A field that "got cleared" because the diff encoding meant something
  different.** A configuration that toggles between "send diffs" and "send
  absolute values" lets `0` mean "no change" in one mode and "the value zero"
  in the other; `null` switches similarly. The fix is a [tagged
  union](../10-data-modelling/02-union-types.md) per kind of update, never an
  overload of the same field. Mutability discipline does not solve the
  encoding-choice bug by itself, but the immutable-by-default + `with` shape
  makes "the field changed because someone applied a diff in the wrong mode"
  a visible, deliberate transformation rather than an in-place write.

## See also

- [Let Bindings](01-let-bindings.md) — `let` versus `uniq`.
- [No Global Mutable State](07-no-global-mutable-state.md) — why mutable state
  never lives at the top level.
- [Destructuring and Object Construction](06-destructuring.md) — the `with` form.
- [Antifeatures](../02-philosophy/04-antifeatures.md) — no `null`, no setters.
