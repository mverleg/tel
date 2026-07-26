# Value vs Reference Semantics

TODO: review

Tel uses **value semantics everywhere by default**. Every binding, argument,
and field behaves as if it holds its *own* value; assigning, passing, or
returning a value never silently produces a second name for the same mutable
object. There is no user-visible distinction between "a primitive" and "an
object" — an `Int64`, a `Text`, a `Point`, and a `List[Order]` all behave the
same way.

This is a deliberate, high-priority choice. It serves two priorities at once:

- **High abstraction over low-level control** — a script author never has to
  ask "is this a reference or a copy?" The answer is always the same.
- **One script, many hosts** — value semantics compile cleanly *down* to
  mutable host languages, but the reverse is not true. Picking value semantics
  as the universal model keeps behaviour identical whether Tel runs
  interpreted, compiled to Rust, or compiled to JavaScript. See
  [`../02-philosophy/01-priorities.md`](../02-philosophy/01-priorities.md).

## What value semantics means

When a value is bound to a new name, passed to a function, or stored in a
field, the program behaves *as if* an independent copy was made:

```tel
let a = Point { x = 1, y = 2 }
let b = a            # b is conceptually its own Point
# nothing a does can be observed through b, and vice versa
```

Because Tel has no `null`, no uninitialised bindings, and tightly scoped
mutation (see [`../02-philosophy/04-antifeatures.md`](../02-philosophy/04-antifeatures.md)),
there is no way to mutate `a` and have the change "leak" into `b`. Aliasing of
*mutable* state is simply not expressible in the surface language.

This is a semantic guarantee, not an implementation mandate. "As if a copy was
made" does not mean a copy is *actually* made — see
[Copies are conceptual, not literal](#copies-are-conceptual-not-literal).

<!-- TODO(open): How are `Lazy`/memoised values copied? A value that computes
itself on first read and then caches the result (a `Lazy[T]`, or any field with
lazy/memoised initialisation) raises two questions value semantics has not yet
answered. (1) Does an unforced→forced `Lazy` count as *mutable*? Its cache is
written on first read, even though the logical value never changes. (2) For a
non-affine (freely-copyable) value, when a `Lazy` is copied *before* it is
forced, does each copy compute independently, or do they share one memoised
result? Swift hit exactly this: copying a struct copied the lazy property's
initialised-or-not state, so independent copies re-initialised separately —
subtle and counterintuitive (Jordan Rose, "Swift Regret: Lazy Vars in Structs",
https://belkadan.com/blog/2021/12/Swift-Regret-Lazy-Vars-in-Structs/ ). Tel has
no struct-vs-class split, so the rule must be uniform across all types. Lean: a
*forced* `Lazy` copies as its already-computed value; an *unforced* one
recomputes per copy (value semantics preserved), and forcing is not observable
mutation. Decide and document — see also the mutability model in
[`../06-bindings-and-scope/02-mutability.md`](../06-bindings-and-scope/02-mutability.md). -->


## Why this rules out a whole class of bugs

Aliasing — two names for one mutable object — is the structural cause of a recurring family of production bugs the design wants to prevent. A few representative shapes from the catalogue:

- **"The simulation input changed under us."** A computation step mutates its input in place; another step kept a reference to that input for later persistence; the value that ends up on disk is no longer what production saw at the time. Variants: an in-place interpolation step, an in-place "move" step, an `add_all` followed by later mutation of the list.
- **"The API input mutated after we sent it."** A class kept the latest N events in an array list inside a map; a "shallow copy" of the map was passed across an API boundary; the list inside kept changing, causing serialization failures and inconsistent reads.
- **"Two callers shared one cache slot."** A cache stored a builder-style object both callers held a reference to. One caller mutated; the other observed the change.

Tel's answer is the value-semantics rule plus [transitive mutability](../06-bindings-and-scope/02-mutability.md): there is no way to reach an alias of a mutable value, and no way to silently widen a *shallow* copy into one that still shares mutable state. A value sent across an API boundary cannot be mutated by the sender afterwards, because no live name in the sender's scope refers to the receiver's value. See also [the per-task heap isolation rule](../14-concurrency-and-parallelism/07-memory-model-for-concurrency.md) for the concurrency form of the same guarantee.

## Copies are conceptual, not literal

A naive reading of value semantics — "copy everything on every assignment" —
would be ruinously slow for large collections. Tel does not require that.

The language guarantees the *observable behaviour* of copying; the compiler and
runtime are free to implement it however they like:

- **Immutable values can be shared freely.** If a value can never change, any
  number of names pointing at the same storage is indistinguishable from each
  number of independent copies. Most Tel values are immutable, so most
  "copies" cost a pointer.
- **Copy-on-write.** A value shared between names may be physically copied only
  at the moment one name is about to mutate it.
- **Real copies** are used when small (a few machine words) or when the
  compiler's analysis shows it is cheapest.

The decision is left to the backend, informed by per-variable IR metadata —
see [Runtime Representation](06-runtime-representation.md). A script author
reasons only about the value model.

## Denoting copy vs rename vs swap

Value semantics removes the *accidental* alias, but a script still needs to
express common data-movement operations clearly. Three operations recur and
should each be visually distinct:

- **Value copy** — "give me an independent value equal to this one." Writing
  `let row = matrix_row` or passing a value as an argument already means this.
  Because it is the default, it needs no special mark.
- **Rename** — "this value is now known under a different name; the old name is
  done." This is a move, not a copy: it lets the compiler skip even a
  conceptual copy and reuse the storage. Common in builder-style code and in
  double-buffer algorithms.
- **Swap** — exchange two values without copying either. The classic case is
  swapping two buffers between simulation steps; both names survive, their
  contents are exchanged.

```tel
# copy: front is an independent value
let front = back

# rename / move: `back` is consumed, no copy needed
let front = move back        # TODO(open): spelling of move/rename

# swap: exchange contents, no copy of either buffer
swap(front, back)            # TODO(open): swap as stdlib fn or operator
```

TODO(open): the exact surface syntax for *rename/move* and *swap* is unsettled.
How should a value-copy be denoted versus a "just a rename"
in double-buffer code? Candidate answers: a `move` keyword/postfix for rename;
a `swap` builtin or stdlib function for swap; plain `let`/argument passing for
copy. This interacts with the mutability model — see the open question in
[`../02-philosophy/04-antifeatures.md`](../02-philosophy/04-antifeatures.md).
For a small immutable value, *copy* and *rename* are observably identical and
the distinction is purely a performance hint the compiler may derive on its
own.

## References and aliasing

Tel does not give scripts raw references, pointers, or addresses (see
[`../02-philosophy/04-antifeatures.md`](../02-philosophy/04-antifeatures.md)).
Value semantics is the default for every binding, argument, and field. On top
of that default Tel adds a bounded, controlled **borrow** form — `&T`
(read-only) and `&!T` (exclusive mutable) — a scoped *value*, never an
address, so `&` always means "borrow" and never "address-of". Borrows let a
function read or mutate a caller's value without taking it, without breaking
the no-raw-pointers guarantee; their surface forms are covered in
[References and Aliasing](04-references-and-aliasing.md) and their scopes in
[Lifetimes](05-lifetimes.md). The implementation-facing notion of aliasing —
used only by the IR and codegen — is covered in the same References and
Aliasing topic.

## See also

- [Stack and Heap](02-stack-and-heap.md) — why placement is not a script-level
  concern.
- [Runtime Representation](06-runtime-representation.md) — the IR metadata that
  lets a backend implement value semantics efficiently.
- [Memory Management](03-memory-management.md) — how values are reclaimed.
