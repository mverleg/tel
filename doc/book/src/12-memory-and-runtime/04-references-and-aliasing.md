# References and Aliasing

<!-- TODO: review -->

Tel's surface language is **value-first**: most code passes values, and value
semantics (see [Value vs Reference Semantics](01-value-vs-reference-semantics.md))
mean a script cannot accidentally create two mutable names for the same object.
On top of that, Tel adds a **controlled borrow** form so a function can look at
or mutate a caller's value without taking ownership — and without ever exposing
raw pointers or addresses.

> The borrow design lives in [TIP-0001](../tips/0001-mutability-and-borrowing.md)
> (Accepted). This page is the migrated reference.

## Borrows, written `&` and `&!`

Tel adds a **controlled borrow** form using the sigils of the one mainstream
borrow language, Rust — under one hard rule that keeps them clear of C's pointer
baggage: **`&` only ever means "borrow", never "address-of".** A borrow is an
ordinary scoped value, not an address, and there is no pointer arithmetic (the
"no raw pointers" stance still stands; see
[antifeatures](../02-philosophy/04-antifeatures.md)). There are two borrow forms
plus a deref:

- **`&T`** — a **read-only borrow**. Multiple may coexist. It exposes only `T`'s
  non-mutating methods, and while one is live the owner's mutating methods are
  statically refused. A read borrow views its referent immutably whether the
  owner holds a shareable `T` or an affine `!T` (reading needs none of the
  affine capabilities, and views the fields in place — no freeze, no copy).
- **`&!T`** — an **exclusive mutable borrow**: a borrow of the *affine* type
  `!T`. At most one is outstanding, and no `&T` may coexist with it. A function
  taking `&!T` states in its signature exactly which arguments it will mutate,
  restoring local reasoning at the call site.
- **`*`** — a **deref**, for the rare case the compiler cannot elide it. Reading
  or calling through a borrow needs no `*` on the common path — auto-deref and
  auto-ref are inserted at method-call sites exactly as in Rust, so a `&self`
  method is callable on a borrow without an explicit `*`; the sigil exists only
  for the occasional explicit case.

### The borrow forms

The borrow operator is `&`; the `!` belongs to the **type**, marking it affine
(`!T`; see [mutability](02-mutability.md#two-axes-ownership-and-reassignability)).
The two compose left-to-right, so `&!T` reads as "a borrow of `!T`":

| form | meaning |
|---|---|
| `Person` | owned, shareable (`Alias`) |
| `!Person` | owned, affine (mutable in place) |
| `&Person` | read borrow (shared) |
| `&!Person` | exclusive **mutable** borrow — a borrow of the affine `!Person` |

Reading rule: **exclusivity comes from the referent being affine, not from a
separate borrow marker.** `&!T` is exclusive because `!T` is affine; `&T` is
shared because `T` is `Alias`. There is **no `!&` form**: an exclusive borrow of
a non-affine type would have nothing to mutate, so it is meaningless — which is
exactly why the `!` sits on the type and never to the left of the `&`.
Equivalently, `&!Person` is the contraction of "exclusive borrow of `!Person`";
the redundant doubled marker older drafts wrote (`!&!Person`) collapses to one.

A `&!T` borrow **suspends** the owner for the borrow's scope and **reinstates
it unconditionally** at the end — the owner is always handed back, whatever
happened during the borrow, because a borrow never owns and so has nothing to
destroy. That unconditional give-back has a direct consequence for iteration: a
`next` written as `next(&!self) -> Option[T]` is inherently **fused** — it
always leaves a live, re-callable iterator behind, so it *cannot* express "this
source is exhausted, do not call again." Reaching that dead end requires `next`
to own `self` and thread the continuation out through its return value (the
consume-through-borrow law); see
[the linear iterator](08-substructural-types.md#the-iterator-value-as-a-linear-resource)
and the [iterators chapter](../10-data-modelling/10-iterators-and-sequences.md).

(Earlier drafts spelled these `Readonly[T]` / `Uniq[T]`, and the exclusive lend
`!&T` or `&uniq`; all are retired in favour of the `&` / `&!` sigils.)

Coercion from `T` to `&T` (and lending as `&!T`) is implicit at call sites;
the borrow's scope is inferred and almost never written (see
[Lifetimes](05-lifetimes.md)). Borrows are **fiber-local** — they never cross a
task boundary, and the deep-copy hand-off between tasks refuses to copy one,
preserving heap isolation (see
[the concurrency memory model](../14-concurrency-and-parallelism/07-memory-model-for-concurrency.md)).

Where a Rust program would pass `&T` or `&mut T`, a Tel program passes a value,
an `&T`, or an `&!T`; the compiler still decides whether that compiles down
to a copy, a shared pointer, or a native borrow in the target language — so one
script behaves identically across an interpreter, an AOT Rust backend, and a JS
backend.

### Why borrows are `not Send`

A borrow is a scoped pointer into the owner's heap, and tasks' heaps are
fiber-local (see
[the concurrency memory model](../14-concurrency-and-parallelism/07-memory-model-for-concurrency.md)).
Sending a borrow to another task would produce a cross-heap pointer the
receiver cannot validly dereference. Even on a host that happened to give every
task a shared GC heap (so the pointer *did* dereference cleanly), letting a
borrow cross would re-open concurrent shared access to mutable state — the very
thing isolation exists to prevent. So borrows are **structurally `not Send`**, on
both the platform-portability and the data-race-safety grounds.

This is *the* main source of `not Send` in normal Tel code: **owned values stay
Send** — shareable values share or copy across tasks, affine values *move*
(consuming the original) — and only **borrows and host-affine resources**
are `not Send`. Users never opt into `not Send` on plain data; it falls out
structurally from "this type contains a borrow" or "this type wraps a
fiber-pinned host handle."

## Aliasing as IR metadata

A backend, especially one targeting a low-level language like Rust or wasm,
still benefits enormously from knowing whether a given value is aliased. The
[Xolir IR](06-runtime-representation.md) therefore tags each variable with an
aliasing classification. The three relevant levels:

- **Possibly shared between fibers** — the value may be reachable from more
  than one task. (In practice rare: Tel's per-fiber heaps mean cross-fiber
  sharing happens by deep copy, so a "shared" value is usually a deliberate
  hand-off.)
- **Possibly aliased within one fiber** — more than one name in the same
  fiber may reach this value, but no other fiber can. A Rust backend can use a
  shared smart pointer (`Rc`) without atomics.
- **Non-aliasing within one fiber** — exactly one name reaches this value.
  A Rust backend can use a plain owned value or `Box`, no reference counting.

The classification is **conservative**: the compiler starts every value at the
most-shared level and only *downgrades* when analysis proves a tighter one.
Getting it wrong toward "more shared" is merely slower; getting it wrong toward
"less shared" would be unsound, so the analysis never does.

This metadata is part of the IR a backend consumes, not part of Tel syntax. A
script author neither writes it nor sees it. It is recorded here, rather than
left purely to `impl-notes/`, because it is a guarantee the IR makes to every
backend: see [Runtime Representation](06-runtime-representation.md).

## Ownership and aliasing together

Aliasing classification pairs with the **ownership** axis. An affine `!T` value
is "non-aliasing within one fiber" by construction — it has one owner — and
freezing it (`finish()`, one-way) yields a shareable `T` that may then be
aliased freely at no cost, because no name can change it. Ownership is
transitive: an affine value's parts are reached affinely.

This is the *ownership* axis, **not mutability** — the two do not coincide. A
shareable (`Alias`) value need not be immutable: the stdlib synchronised types
(`ConcHashMap`, `Mutex`, atomics) are `Alias` *and* interior-mutable, and are
deliberately not `!` types (they coordinate internally; see
[substructural types](08-substructural-types.md)). What the aliasing analysis
and the per-fiber collector actually rely on is that nothing is *shared between
fibers* implicitly — all aliasing is fiber-local or an explicit hand-off — which
is what keeps the per-fiber single-threaded collector (see
[Memory Management](03-memory-management.md)) sound.

The model is settled in
[TIP-0001](../tips/0001-mutability-and-borrowing.md) (Accepted): the `!` axis is
type-level **ownership** (`!T` affine vs `T` shareable) with `uniq` as its
binding-level form; mutability is the separate, correlated data-model axis (see
[mutability](02-mutability.md#two-axes-ownership-and-reassignability)).

## See also

- [Value vs Reference Semantics](01-value-vs-reference-semantics.md) — the
  value model that removes accidental aliasing.
- [Runtime Representation](06-runtime-representation.md) — the full set of
  per-variable IR metadata, of which aliasing is one field.
- [Lifetimes](05-lifetimes.md) — the scopes that `&T` / `&!T` borrows
  carry, named but rarely written.
- [TIP-0001](../tips/0001-mutability-and-borrowing.md) — the borrowing proposal.
