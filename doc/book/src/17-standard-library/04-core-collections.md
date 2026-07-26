# Core Collections

<!-- TODO: review -->

## What

`std` ships a small, curated set of collection types: lists, maps, sets, and
*tables*. Each comes in a **mutable builder** form and an **immutable** form,
and the library also offers **vectorized / transposed** collections for
data-parallel work. The reader works with high-level types — *a list*, *a
map* — and does not pick between hash and tree representations by hand.

## Why

### Mutable builder vs immutable result

Tel leans immutable: immutable values are easy to reason about, cheap for the
garbage collector, and — the load-bearing reason for Tel — **easy to compile
to a mutable target language, while the reverse is not true**. A host language
can always implement an immutable Tel `List` with whatever it likes; an
immutable Tel value never forces the host into a particular sharing model.

But building a collection element-by-element wants mutation. So each
collection has two faces:

- A **mutable form** (`!List`, `!Map`, …) — used in a small, local scope to
  assemble a value. Its mutability is confined; for the garbage collector it
  behaves like short-lived young-generation data.
- An **immutable** collection (`List`, `Map`, `Set`) — the finished value,
  freely shared, freely returned across the host boundary.

```tel
let primes = {
    let uniq builder = !List[Int64]()
    for n in 2..100 {
        if is_prime(n) { builder.add(n) }
    }
    builder.finish()        # yields an immutable List[Int64]
}
```

Crucially, **ownership is a property of the type, not of the binding**. A
`List` is shareable because it *is* a `List`; a `!List` is owned — and so
mutable in place — because it *is* a `!List` (the `!T` sigil). This makes
ownership visible in signatures and — the goal — lets the compiler treat
shareable collections as safe to pass between tasks while an affine form is
confined to one owner. A shareable collection may only contain shareable
(`Alias`) element types. This is the settled model
([TIP-0001](../tips/0001-mutability-and-borrowing.md)):
type-level `!T` *and* `uniq` bindings, which compose
(`let uniq builder = !List[Int64]()`).

### Mutable and immutable can have different shapes

The ideal representation can differ between the two forms. A sorted collection
is naturally a balanced tree while it is being mutated, but a plain sorted
array once frozen. The builder/immutable split lets each side pick what fits.
`TODO(open): could the mutable/immutable pair be expressed as one type
parametrised by a const "is mutable" flag — possibly a no-data enum used as a
const generic — so a single declaration yields both. Interesting but
unresolved; depends on the const-generics design.`

### Hiding the representation

At a high level the programmer should not have to think *hash set vs tree set
vs list*. The library's safeguard is a performance contract on the operations,
not the type name: an operation like `.contains(..)` is only offered where the
chosen type can do it in at most logarithmic time. If you can call
`.contains`, it is cheap — so the underlying structure stops mattering. This
is also a linting concern; see
[`../18-tooling/07-linter.md`](../18-tooling/07-linter.md) for catching
patterns like `.contains` on a plain list.

### Equality, ordering, hashing — key or collection?

Whether ordering/hashing/equality is tied to the *key type* or supplied to the
*collection* is an open call. The design leans toward a **key-wrapper**
approach: wrap the key in a newtype that documents the alternative hash or
order, rather than passing a loose comparator. This keeps the unusual
behaviour visible at every use site. `TODO(open): decide key-wrapper vs
collection-supplied comparator (or allow both); coordinate with the traits
chapter.`

## Tables

A *table* (`Table[R]`) is a rectangular collection: many rows of one common,
column-typed row shape `R`. It is a full standard-library feature with its own
chapter — **[Dataframes](../10a-dataframes/01-overview.md)** — covering the
record-shape calculus, the schema-changing operations (`select`, `extend`, join,
`group_by`/`agg`, `pivot`), columnar storage, and the `!Table` builder. This
section only situates it among the other collections.

The blessed storage is **column-major** (a struct of equal-length arrays) — the
same idea as the vectorized collections below: store common structure once per
column instead of once per element. An **array-of-structs row view** is offered as
a plain `List[R]` over that storage and needs no special support. Tables also carry
**type-level structure** the other collections do not:

- **Uniqueness** of a column (it behaves like a set; *sortedness* may be
  expressible the same way). Filtering on a unique column yields an `Option[R]`
  rather than a list.
- **Schema transforms as type functions** — joins, projections, aggregations. A
  join is a function *on types*: two row types and a key produce a third, with a
  left join making the right-hand columns optional and key-uniqueness propagating
  so the type system knows no duplicates were introduced.

```tel
# Schematic — syntax not pinned down.
# Names  { id!, first, last }          ! = unique
# Births { id!, bsn!, birthday }
# left join on id yields:
# People { id!, first, last, bsn!?, birthday? }   ? = nullable
```

These are the
[record-shape calculus](../05-types/15-record-shape-calculus.md) primitives;
the dataframe chapters are the spec. A **capability-backed table** — backed by a
file-shaped capability and pulling rows on demand, or pushing operations down to a
host database — is the *query carrier* of the same calculus (see
[the dataframe evaluation chapter](../10a-dataframes/03-storage-mutability-evaluation.md#the-query-carrier-direction)).
It composes this Table with the [I/O capabilities](08-io-and-filesystem.md) and the
[stream model](05-iteration-and-streams.md) and presents the same value-level
surface either way.

## Vectorized / transposed collections

For data-parallel work `std` offers collections that store a `List[Point]` as
columns — conceptually `Point(List[x], List[y])` — *transparently*, so the
code reading it does not change. There are three vector flavours:

- A plain collection of objects (effectively the immutable `List`).
- A **transposed** collection storing objects column-wise, supporting
  efficient per-field vectorized operations.
- A transposed collection allocated for GPU-style execution, convertible
  to/from the CPU form (the conversion moves data across memory).

The transposed forms generate per-platform code; a host without the relevant
hardware still runs them, just without the acceleration. Static-sized variants
(like fixed arrays) matter too, so small vectors do not pay for the widest
representation.

**The language makes no memory-layout promise for these vectors**, on purpose.
An implementation must be free to store a vector as a whole number of
SIMD/GPU blocks for its element type — e.g. if the target multiplies four
`f64` at once, a 10-element vector is three 4-wide blocks with the last two
lanes **masked** — so the spec never fixes element stride, forbids trailing
padding, or guarantees a contiguous element-for-element layout. This block-and-
mask padding is an invisible implementation detail; the only observable thing
is the vector's logical length and its values. (Code that needs an exact byte
layout for FFI uses an explicit fixed array with a stated representation, not a
vector.)

This is the *high-abstraction* answer to SIMD/GPU: the programmer writes
ordinary collection code, and the implementation vectorizes. Tel does **not**
expose SIMD intrinsics or a GPU sub-language — see
[`../02-philosophy/04-antifeatures.md`](../02-philosophy/04-antifeatures.md).
`TODO(open): it is unsettled how much of this pays off versus plain
autovectorisation; hand-written numerical kernels may still
beat a generic vector calculation graph. Keep the transposed-collection idea;
treat the GPU tier as tentative. Re-justify against embedding — heavy
data-parallel kernels may belong in the host behind a capability.`

## Arrays and lists

Beyond the conceptual `List`, `std` makes several array shapes visible — the
choice is driven by what the *size* story is, not by manual memory layout:

- **Compile-time fixed size** — the length is part of the type. Useful for
  small dense vectors (a 3-component point), and for catching size mismatches
  at compile time.
- **Run-time constant size** — the length is set on construction and never
  changes. No silent re-growth, no allocator surprises during a loop.
- **Freely growable** — the everyday list (Rust `Vec`, Java `ArrayList`).
- **Capacity that only grows on explicit request** — fixed-shape behaviour
  with a manual `.reserve(n)` escape hatch. It is unsettled whether this fourth
  flavour pays its weight; the value is preventing accidental re-allocations
  in hot loops. `TODO(open): keep or drop the explicit-grow flavour.`

Arrays support **per-element initialiser functions** — `Array.init(n, |i| ...)`
— so the common pattern of "allocate, then loop to fill" becomes one
expression. This both reads cleaner, and gives the implementation a fused
producer it can parallelise; see
[`05-iteration-and-streams.md`](05-iteration-and-streams.md).

```tel
let squares = Array.init(50, |i| i * i)
```

Implementation notes (stack vs heap, short-string-style optimisations,
bucketed allocators for stable element addresses) are left to `impl-notes/`.
The user-facing rule is that the implementation may be different for each
flavour, but the surface API is uniform.

## Sets, maps and the "what to do when full" question

The library leans on a few rules across all keyed structures:

- **Hash, equality and ordering are tied to the key type, not the
  collection.** A key wrapped in a newtype with a specific hash or order is
  visibly different at every use site. The library does not accept loose
  comparators on every operation. See the open question above.
- **Sets, maps and queues respect the underlying type's mutability rules.**
  An immutable `Map` may only contain immutable keys and values; a
  `!Map` may hold mutable values, but cannot then be frozen until they
  are.
- **Linked-hash variants exist** to prevent maps from leaking iteration order
  through their hash salt. A Rust bug fixed in 2016 leaked salt through
  iteration order; using linked-hash by default for "iterable
  map" prevents the class of issue.
- **Iteration order is either fully defined or aggressively randomised — never in between.** The
  [determinism feature](../02-philosophy/03-features.md) requires that a
  Tel program's behaviour does not silently depend on a hash salt or a host's
  hashmap implementation. The library offers two map shapes: an
  *ordered* map (insertion-order or key-ordered, fully deterministic) and a
  *plain* hash map whose iteration order is **deliberately permuted per
  run**, so any script that accidentally depends on order fails loud and
  early instead of in production on a different host. There is no third
  "happens to be stable today" map. `TODO(open): exact spelling, whether
  the permutation is per-task or per-process, and how this composes with
  reproducibility — a fixed seed via the RNG capability gives a stable
  order when wanted, randomised by default.`

### Bitsets and enum sets

A set of enum-without-data values is, structurally, a bitfield. `std` exposes
this as `EnumSet[E]` (or similar) so the user reads it as a set of values
while the implementation packs it into a machine word. Java's
`EnumSet` is the precedent. `TODO(open): naming and exact API.`

## Queues and channels (data-structure side)

Every queue in `std` is built around a firm set of rules —
the *concurrency* side of channels lives in
[`12-concurrency-utilities.md`](12-concurrency-utilities.md); this is the
**data structure** contract:

- **Bounded by default.** A queue has a fixed capacity unless explicitly
  asked to grow. Unbounded queues are the cause of most "filled up the heap"
  outages; the library does not make that the default.
- **Explicit fullness policy.** Every queue is constructed with a stated
  policy for what happens when it is full — **block**, **drop head**, **drop
  tail**, or **fail**. There is no library default.
- **Closeable.** A queue has three observable states — non-empty, empty,
  *closed* — not two. Closure is how a producer signals "no more values"
  without consumers guessing.
- **Observable.** Queues count size, drops, and queue-time, and expose
  `on_add` / `on_pop` / `on_full` callbacks for metrics. *Almost every queue
  should* count these by default; metrics are part
  of the contract, not an opt-in extra.
- **Timeouts on blocking operations.** A `send` or `receive` may take a
  timeout measured against the injected [`Clock`](09-time.md); there is no
  ambient timer.

```tel
let q = Queue[Job](
    capacity = 1024,
    on_full  = OnFull.DropOldest,
    metrics  = MetricsSink(...)
)
```

`TODO(open): the exact shape of the metrics hook — callbacks vs an aggregator
object — overlaps with the observability story; coordinate with
[`14-observability-and-logging.md`](14-observability-and-logging.md).`

## Many-to-many and relational shapes

Beyond list, map, set and table, `std` offers a small set of
relational-flavoured structures:

- **Many-to-many** — a structure that maps each `A` to a set of `B`s and each
  `B` to a set of `A`s, with both views always consistent. The classic
  example is tag systems.
- **Lookup-by-property map** — a map whose key is *derived from* a field of
  the value, rather than supplied separately. Insertion needs only the value;
  the key is always in sync with the stored object, and the "key wrong way
  round" mistake disappears.
- **Two-map pairing / merge** — given two maps with possibly overlapping
  keys, produce a map keyed by the union, with values paired (and made
  optional where one side is missing). Java's `Map.merge` only handles the
  per-key case; the wishlist is for whole-map pairing too.
- **Sorted-unique list-as-type** — a list whose declared type encodes
  "sorted, no duplicates". The compiler / linter rejects an out-of-order
  literal, so a hard-coded table stays sorted as code evolves. `@sorted` /
  `@aligned` annotations are an alternative, but a refined collection
  type is preferred.

`TODO(open): the relational extension set risks overlap with **tables** above.
Decide whether many-to-many is a separate type or a special-case table; decide
whether lookup-by-property is a map flavour or a tiny table.`

## Mutable → immutable conversion

A mutable collection should be able to *become* the matching immutable form
in one call, reusing the allocation rather than copying. The tricky part:
code that does this only knows the abstract mutable form, not the
concrete implementation, so the produced type is "the right `List` for this
`!List`" — chosen by the implementation, exposing the traits the caller
needs (e.g. `Sync`-ness is preserved if available).

The simple shape:

```tel
let uniq names = !List[Text]()
names.add("...")
let frozen: List[Text] = names.finish()    # consumes the mutable form
```

`TODO(open): a builder that implements *several* mutable traits would need to
strip all the `Mut` bits at once when frozen — this is
fiddly. Defer until the trait system is firmer.`

## Heterogeneous-but-inline collections

Can `std` offer a *collection of differently sized
elements stored without indirection* — the same shape as UTF-8 storing
variable-width code points inline? The motivation is cache locality; the
constraint is that the element type's *size* must be part of the encoding.
`TODO(open): unclear that this earns its place in an embedded scripting
library — it leaks the layout the language otherwise hides. Treat as a
direction, not a feature.`

## Bidirectional trees and graphs

A bidirectional tree (parent points to children, children point back to
parent) is awkward in any GC-free model: the cyclic references force
either reference counting, weak parent links, or arena allocation. The
input proposes an **arena-allocated tree** type in `std` so a user does
not roll their own. Properties to make user-visible:

- **Number of links per node** — one-way (downstream only), two-way
  (parent + children), or N-way (DAG, graph).
- **Cycle policy** — strict tree, acyclic graph, general graph. A
  cycle-attempting insertion is a runtime error for stricter types.
- **Ownership shape** — the arena owns the nodes; a "node handle" is a
  cheap index into the arena. Handles are valid only while the arena
  is in scope.

The arena's borrow lifetime constrains where the tree can travel —
typically a lexical scope. Returning a tree across that boundary needs an
explicit *freeze* into a node-owned (or immutable arena-backed) form.

`TODO(open): the bi-tree / graph design touches the unresolved mutability
and ownership model
([`../02-philosophy/04-antifeatures.md`](../02-philosophy/04-antifeatures.md)).
It is also an open question whether `std` should expose a *generic* arena
allocator (with safe pointer types) as a building block; this leans
toward "no" — generic arenas are exactly the low-level control Tel
avoids — but the tree/graph cases that need them are real.`

## Stack-allocated fixed-size collections

For the cases where allocator pressure shows up (tight loops, small
buffers passed across the host boundary), the library exposes a small
family of *stack-resident* fixed-size collections — `StackList[T, N]`,
`StackMap[K, V, N]`, etc. The size is part of the type; overflow is a
compile-time error where it can be proven and a runtime error
otherwise. These mirror the compile-time-fixed array above; the
distinction is that they preserve the *collection* surface (push, pop,
contains) on top of the fixed storage.

`TODO(open): "stack-allocated" leaks an implementation detail Tel
otherwise hides — the host runtime may not have a stack the way the
description implies. Re-justify against embedding. A cleaner framing
might be "small-collection optimisation": the *type* commits to
not-allocating-on-the-heap-for-N-or-fewer elements, and the
implementation picks the representation.`

## A note on `add_all(optional)`

A small ergonomics point: `list.add_all(maybe_other)`
should accept an `Option[Iter[T]]` and add either every element or
nothing, without a guard. The library treats `Option` as a single-element
or zero-element iterable for this purpose: any operation that consumes
an iterable also accepts an optional one. Same for `Result[Iter[T]]` —
either the elements, or short-circuit on the error. See the fallible-
iteration notes in [`05-iteration-and-streams.md`](05-iteration-and-streams.md).

## See also

- [Iteration and Streams](05-iteration-and-streams.md)
- [Numerics and Math](07-numerics-and-math.md)
- [Concurrency Utilities](12-concurrency-utilities.md) — concurrency side of channels
- [Matrix Math and FFT](../19-use-cases/05-matrix-and-fft.md)
