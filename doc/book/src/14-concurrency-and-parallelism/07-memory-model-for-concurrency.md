# Memory Model for Concurrency

<!-- TODO: review -->

Tel's concurrency memory model has one defining property:

> **Concurrent shared mutable state is impossible.** Tasks do not share a heap.
> A data race cannot be written in Tel.

This is not enforced by a borrow checker watching shared references — it is
enforced *structurally*, by giving every [task](02-tasks.md) its own isolated
heap. This topic explains the model, why it was chosen, and what it costs.

## Per-task isolated heaps

Each task runs against its **own heap**. A task cannot reach another task's
objects, because there is no reference that crosses a heap boundary. When data
moves between tasks — captured by a spawned closure, sent through a
[channel](06-channels-and-message-passing.md), or returned from `join` — it is
**deep-copied** into the destination heap.

So there are exactly two ways data crosses a task boundary, and both copy:

1. **At spawn** — values the task body captures are copied into the new task's
   heap.
2. **At a message / result hand-off** — values sent on a channel or returned
   through `join` are copied into the receiver's heap.

Between those points, a task touches only its own objects. Two tasks therefore
*cannot* be looking at the same mutable object, so there is nothing to race on
and nothing to lock.

```tel
let uniq tally = Tally.empty()
let h = tasks.spawn("count", || {
    # This task gets its OWN copy of `tally`.
    tally.add(work())          # mutates the copy, not the original
    tally
})
let counted = h.join()         # `counted` is a copy back into this heap
```

## Every host must implement deep transfer

The isolation model rests on one capability every host **must** provide:

> A host must be able to **deeply transfer** any [`Send`](09-scoped-values.md)
> value into another task's heap, yielding a value that is **fully independent
> of the original yet structurally identical** — same shape, same contents, same
> internal sharing and cycles — reachable from no pointer the source heap still
> holds.

This is what `spawn` (captures), `join` (results), and a
[channel](06-channels-and-message-passing.md) send all rely on. The transfer is
a *runtime* operation driven by the value's layout — **not** a user-defined
`Clone`. Three obligations make "independent yet identical" precise:

- **Independence.** Afterwards the destination shares no mutable state with the
  source: writing through one is never observable through the other. (An affine
  value is moved-from and gone; an immutable value has nothing to write.)
- **Structural fidelity.** Internal *sharing* and *cycles* are preserved, not
  unfolded. Two fields pointing at one sub-object stay that way in the copy
  (one sub-object, two paths), and a cyclic graph is reproduced as a cycle, not
  chased forever. A correct deep transfer is therefore graph-aware — a
  visited-set / object table, the way a serializer is (see the
  [open questions](#open-questions) on cycles).
- **Totality.** Every `Send` value can be transferred. The only things that
  cannot are `not Send` (thread-affine host resources, stack borrows); those are
  rejected at the boundary, never copied. (A *scoped* task is the proposed
  exception that lets a child hold a borrow into the parent rather than a copy,
  because the scope bounds the borrow's lifetime — see
  [structured concurrency](04-structured-concurrency.md#borrowing-in-a-scoped-task).)

The mechanism is the runtime's, because the runtime owns the layout: an AOT
backend can emit a per-type relocate routine, an interpreter can drive a generic
graph-copy from a type descriptor. Neither needs user code.

### What gets copied, and what gets shared

A deep transfer recurses through a value's **affine, owned structure** — which,
because affine ownership cannot alias or cycle (see the cycles
[open question](#open-questions)), is always a *tree* — copying it node by node.
When the recursion reaches a sub-object that is **immutable** or a **`Sync`
stdlib type**, it **shares** that sub-object by reference (a refcount bump)
rather than descending into it:

- For **immutable** sub-objects the choice is invisible — share or copy, no
  reader can tell, because immutable identity is not observable. So it is a pure
  optimisation: cheaper to share; a separate-heap host with no shared region
  copies instead. See
  [the copy is not always physical](#the-copy-is-not-always-physical).
- For **mutable `Sync`** sub-objects (a `Mutex`, an atomic, a concurrent map)
  sharing is **required, not optional.** The whole point of the value is that
  every task sees the *same* one, so a copy would wrongly fork the shared state
  — and such a value often *cannot* be copied at all (it may wrap a host lock).
  Sending one just hands over another handle to the same object.

Two consequences fall out:

- **The affine part is a tree, so its copy is simple** — a plain recursive
  descent that always terminates, no visited-set needed. All aliasing and all
  cycles sit behind `Sync`/immutable shared sub-objects, which are shared rather
  than traversed, so the copy never chases a cycle. The graph-aware visited-set
  machinery is only needed by a host that *physically copies* shared immutables
  (a separate-heap target), and there the only cycles left to worry about are
  immutable ones.
- **Shared-memory `Sync` types need a shared region to live in.** A host with
  genuinely separate address spaces (multi-process, Web Workers with no shared
  buffer) cannot share a mutable object across tasks at all; there,
  shared-mutable `Sync` types are simply *not offered*, and tasks coordinate
  through a [channel](06-channels-and-message-passing.md) to one owning task
  instead.

Other optimisations the same contract permits:

- **No real boundary crossed** (single-threaded host, inline task, one shared
  heap) lets a move degrade to a shallow "relocate the header, invalidate the
  source" — affinity guarantees no alias is left behind.
- **Lazy / copy-on-write** transfer is allowed where it preserves the semantics
  (see the [open questions](#open-questions)).

A host with separate per-thread heaps performs the full structural deep copy of
the affine tree; a host with one shared heap may do almost nothing. Both satisfy
the same contract.

## Why isolation, not locks

[The priorities](../02-philosophy/01-priorities.md) — *safety over flexibility*,
*high abstraction over low-level control*, *embedded scripts over standalone
projects* — all point the same way.

- **A whole bug class disappears.** Data races are among the hardest bugs to
  find and reproduce. If the language makes them unrepresentable, no script
  ever has one, and no host has to debug one. This is *prevent, don't fix*.
- **No machine-level concepts leak.** Locks, atomics, and memory barriers stay
  out of user code (see [antifeatures](../02-philosophy/04-antifeatures.md)).
  A script reasons about *values and messages*, never about cache coherency.
- **It is the portable choice.** The model is specified as *behaves as if heaps
  are isolated*. A host with real threads can give each task a genuinely
  separate heap; a single-threaded host can use one shared heap and still
  behave identically, because no script can observe the difference. Crucially,
  the implication only runs one way: a language that *assumed* a shared heap
  (e.g. a concurrent hash map visible across tasks) could not be faithfully run
  on an isolating host — so Tel picks the model that every host can satisfy.
- **Cleanup is trivial.** When a task ends — including when it
  [panics](04-structured-concurrency.md) — its entire heap can be dropped at
  once. There is no half-updated shared structure to repair. (Reference
  *cycles* — see [memory management](../12-memory-and-runtime/03-memory-management.md) —
  are still possible inside one task's heap.)

The reference points are Erlang (per-process heaps, copy on message),
**Dart isolates** and **JavaScript Web Workers** (each thread runs in its own
isolate; data is exchanged by sending it, never aliased), and
[Gluon](https://github.com/gluon-lang/gluon) (separate heaps for embedding).
The actor / isolate model is the standard ergonomic choice for "make data
races structurally impossible" — Tel sits in that lineage.

The cost in those languages is real: an isolate-only model rules out some
zero-copy patterns (concurrent readers of a large immutable structure cannot
share its memory without help from the runtime), and forces sender-side
serialisation of anything you want to share. Tel's "copy is semantic, not
necessarily physical" rule (see below) is what buys back the zero-copy
sharing for *immutable* values, while keeping the script's mental model
isolate-shaped.

## The cost, stated honestly

Isolation is restrictive, and the docs do not pretend otherwise:

- **Copies cost.** Sending a large structure between tasks copies it. For
  fine-grained work this can outweigh the parallelism — which is exactly why
  the [spawn strategy](02-tasks.md#the-spawn-strategy) tapers off and runs
  small items inline.
- **No shared concurrent data structures.** A concurrent hash map shared by
  many tasks is not expressible. The intended pattern is instead to give one
  task ownership of the structure and have others talk to it through a
  [channel](06-channels-and-message-passing.md).

Tel accepts this cost deliberately: it is a guest scripting language for
small-to-medium work, not a high-performance shared-memory runtime.

## The copy is not always physical

"Deep copy" describes the *semantics*, not the *implementation*. **Immutable**
data cannot be observed to change, so a host is free to share it behind a
reference instead of copying the bytes — the script cannot tell the difference.
Whether such a shared value uses a non-atomic or an atomic reference count is
itself an implementation choice (a value that never leaves its origin task can
use the cheaper non-atomic form). See
[memory management](../12-memory-and-runtime/03-memory-management.md). Only
*mutable* data genuinely must be copied when it crosses a task boundary,
because the two sides must not alias it.

This is why the mutable/immutable distinction matters for concurrency.

The freedom runs *both* ways, and that is what lets Tel target hosts with no
cross-thread sharing at all. Some platforms cannot point two tasks at one
allocation even in principle:

- **Single-threaded WebAssembly** (no shared-memory threads / no
  `SharedArrayBuffer`), and the Dart-isolate / Web-Worker model generally —
  separate linear memories, nothing shared.
- **The Erlang/BEAM model** — per-process heaps, every inter-process message
  physically copied.

On such a host, an immutable value crossing a task boundary is simply
**deep-copied** instead of shared by reference. Because immutable identity is
not observable (see [below](#identity-is-not-observable-on-values)), copy and
share are indistinguishable to the script, so this is a conforming
implementation, not a degraded one — the program behaves identically, it only
pays more to pass large immutables around. The language spec is written to
permit exactly this: no rule anywhere requires the share-by-reference form (see
[Shared heap is never required](#shared-heap-is-never-required)), so "implement
sharing as copying" is always a valid host strategy for immutable and affine
data alike.

## Identity is not observable on values

The "host may share or copy, indistinguishably" rule above only holds because
Tel does not expose any operation that reveals a value's *identity* — its
address, whether `==` answers "same allocation," or whether two references
point at one object or at two clones.

- **No identity equality.** `==` on values is *structural*. There is no
  reference-equality operator (`is` / `===`), no "same instance" comparison.
- **No address operations.** No `&value`, no pointer arithmetic, no way to
  observe where a value lives in memory.
- **No identity reflection.** There is no runtime facility that distinguishes
  "this is a clone of X" from "this is a reference to X" (see
  [antifeatures](../02-philosophy/04-antifeatures.md)).
- **No identity-keyed hashing or weak references by default.** Hash is
  structural; "weak reference to *this specific allocation*" would let the
  caller distinguish share from copy, so it is not part of `std`.

This is the property that makes thread-local heaps viable for "shared
immutable" data: a host can serve two tasks looking at the same logical
immutable value either by pointing them at one allocation or by copying it
into each task's heap, and no script can tell which. Lose this property and
the copy-vs-share freedom disappears. If a host binding accidentally exposes
an identity-revealing operation (e.g. an FFI handle whose `==` is
pointer-equality), the binding must wrap it before handing it to scripts.

## Shareable vs affine, and data-race safety

Heap isolation removes races between tasks. The **ownership** axis (`Alias`
shareable vs affine `!T`) is the *complementary* mechanism that keeps the model
cheap and sound. The distinction that matters here is shareable-vs-affine, **not
immutable-vs-mutable**:

- **Shareable (`Alias`) values** cross a task boundary by reference for free —
  no copy needed. Plain immutable data is the common case; the stdlib
  synchronised types (`Mutex`, atomics, a concurrent map) are *also* shareable
  even though they mutate, because they coordinate internally.
- **Affine (`!T`) values** must be **moved or copied** at a boundary. A task
  that receives an affine value gets its own private copy (or the moved-from
  original is consumed), so the two sides never alias it.

Ownership is a property of the **type** as well as the binding
([TIP-0001](../tips/0001-mutability-and-borrowing.md) — see below): a `!List`
(affine) is distinct from a `List` (shareable), where a shareable collection may
only hold shareable elements. Any type that transitively contains a `mut` field
or an affine element is itself affine and "not freely shareable", which lets the
compiler reject — or silently copy — such a value crossing a task boundary.

Tel's standard library is expected to contain **essentially no values that are
pinned to one task by nature**. The only things that cannot move freely between
tasks are host-introduced resources — see the next section.

The model is settled
([TIP-0001](../tips/0001-mutability-and-borrowing.md), Accepted), owned by
[bindings and scope](../06-bindings-and-scope/02-mutability.md). Of the two
candidates — (a) Rust-style ownership of arbitrary values with mutable
references, (b) separating shareable and affine *types* — Tel chose **(b)**:
ownership is a property of the type (`!T` affine vs `T` shareable), and affine
values move. This chapter relies only on the affine half *plus* heap isolation:
heap isolation already guarantees race freedom, and the type distinction is what
turns the copy-vs-share decision at a task boundary into a static one.

## Host resources can be task-affine

The language core and standard library are task-agnostic: their values move
between tasks freely (copied if mutable, shared if immutable). The one
exception is **host-introduced resources**, which a host may pin to a single
task or OS thread:

- GUI / windowing handles — most UI toolkits are main-thread-only.
- Thread-local host state and thread-pinned FFI handles.
- A host [capability](../02-philosophy/03-features.md) the embedder
  deliberately scoped to one task.

Such a resource — and anything transitively holding one — **cannot be captured
into another task**. It is marked task-affine in the
[FFI / host-binding layer](../16-ffi-and-interop/), never in ordinary Tel code.
A script that tries to move a window handle into a spawned task is rejected.

```tel
# Host GUI handle — pinned to its task. Cannot be captured elsewhere.
# tasks.spawn("redraw", || window.redraw())   # rejected: window is task-affine
```

## Shared heap is never required

No part of the language and no `std` API *requires* the host to provide a
shared heap. Every value-passing rule — sending on a channel, spawning a
task, joining a result — is specified so that an isolate-only host (deeply
separated per-task heaps, no shared region whatsoever) can implement it. The
one bounded exception is the **platform-conditional shared-mutable
primitives** in
[`../17-standard-library/12-concurrency-utilities.md`](../17-standard-library/12-concurrency-utilities.md#shared-mutable-types-platform-conditional) —
concurrent maps, cloneable channel senders, mailboxes. Hosts that cannot
provide a real shared region for these may omit them or back them with the
actor-based alternative described in the same topic.

This is a **hard rule for new features**: a proposal that needs a shared heap
to behave correctly is flagged as such and must either join the
platform-conditional set or be rejected. Heap sharing is an *optimisation*
that capable hosts may use — most prominently for sharing immutable values
between tasks (see the optimisation note below) — not a behaviour the rest
of the system can depend on.

## Optimisation note

Because the model is specified by *observable behaviour*, the Tel compiler is
allowed to assume there is no implicit sharing between tasks: all aliasing is
task-local or goes through an explicit channel. Target-language code generators
(especially for low-level targets like WebAssembly or Rust) may use this — for
instance choosing a plain value vs a reference-counted box, or a non-atomic vs
atomic count — without changing observable behaviour.

A related optimisation worth recording: a **thread-local
arena** wrapper that behaves like a regular value but is allocated out of
per-worker storage. The motivating case is buffers that are repeatedly
created and dropped within a task — a per-thread arena reuses the slab
without coordinating with other threads. This is purely an implementation
freedom — the script just sees an ordinary value — but it depends on the
task / heap isolation invariant the rest of this topic specifies. If a
backend's optimiser can prove a value never leaves its origin task, it may
allocate it in such an arena (or, equivalently, on a thread-local
bump allocator); see the implementation notes.

A second possible optimisation: a **separate shared-memory region**
for values that the program intends to share between tasks, distinct from
each task's local heap. The argument is cache-locality and GC simplicity —
the local heap stays small and worker-local, while genuinely shared values
(stdlib concurrent types, large immutable structures) live in a region that
all workers can reach. This is implementation territory; the *user-visible*
model is still "tasks have isolated heaps, immutables can be shared by
reference." Whether a host actually splits storage or unifies it is up to
the backend.

## Bugs this prevents

A few catalogue cases the heap-isolation rule rules out by construction:

- **"`HashMap` to `ConcurrentHashMap` migration broke on a `null` value."**
  A switch from `HashMap` to `ConcurrentHashMap` failed because one of the
  values was `null`. In Tel there is no `null` (see
  [antifeatures](../02-philosophy/04-antifeatures.md)), and shared mutable
  maps are not the way to coordinate between tasks — a channel and a single
  owning task is. The whole bug shape disappears.
- **"Race condition NPE on a mutable datasource."** A thread published a
  value from a mutable source; the source set it to `null` between the
  check and the publish. The familiar fix is "assign to a local first."
  Tel's structural fix: the publisher receives an immutable copy (no later
  writer can change it), and there is no `null` to race on.
- **"`ConcurrentModificationException` because a `synchronized` was lost in a
  rebase."** A subtle data race surfaced under load because a sync block had
  been removed. In Tel there is no place a user-written `synchronized` could
  live in user code — shared mutable types are stdlib primitives that ship
  with their own locking discipline, and ordinary mutable user values are
  task-local.

## Open questions

- **Decided: mutability model** — settled in
  [TIP-0001](../tips/0001-mutability-and-borrowing.md); see the resolved note
  inline above (type-level `!T` + affine, mechanism (b)).
- **Resolved: one keyword, `uniq`; affinity is not separately spelled.** The
  dangerous quadrant is *mutable ∧ aliasable*: a write through one alias seen
  through another — a data race unless synchronised. Tel confines that to stdlib
  `Sync` types (`Mutex`, atomics, a concurrent map), which mutate through a
  *shared* API with interior synchronisation — the user never marks them `uniq`,
  exactly as Rust's interior-mutability types are reached through `&`, not
  `&mut`. For user-defined types only two cases remain: `uniq` (exclusive,
  mutable, moved on transfer) and immutable-shareable. So `uniq` *implies*
  exclusive/affine and immutable *implies* freely-shareable; there is **no
  separate `!affine` marker** in user syntax, and the aliasable-and-mutable
  quadrant stays sealed inside the stdlib. The two axes remain distinct
  *semantically* (the compiler still needs both to decide move-vs-share and
  copy-on-transfer) even though only `uniq` is written. The exclusive borrow is
  `&!T`. See the mutability model in
  [bindings and scope](../06-bindings-and-scope/02-mutability.md).
- TODO(open): **Cyclic references.** A cycle needs a value reachable by two
  paths, so it requires *aliasing* — only `!affine` references can form one;
  affine (unique-owner) ownership is a forest and cannot cycle. Since
  user-defined mutable values are affine, the loop-closing power is concentrated
  in stdlib `Sync`/shared-mutable types (and any `letrec`-style knot-tying), so
  cycles originate only there. Two things to pin down: (1) **reclamation** — a
  task's heap is dropped wholesale at task end, so intra-task cycles never leak
  across a task's lifetime; reclaiming a cycle *earlier* inside a long-lived
  task needs a strategy (cycle collector, weak references, or arena scoping),
  owned by [memory management](../12-memory-and-runtime/03-memory-management.md).
  (2) **deep transfer** — the cross-heap copy above must be cycle-aware
  (reproduce the cycle, don't chase it).
- **Decided: no user-visible weak references.** They make GC harder (every
  strong-ref operation must consult/clear weaks; they interact badly with the
  share-by-reference transfer and with refcounting backends), and — decisively —
  a weak reference *leaks the share-vs-copy distinction* the model hides, so it
  is an identity-revealing operation by another name. Committed in
  [antifeatures](../02-philosophy/04-antifeatures.md#no-weak-references-user-visible).
  A host's GC *may* use weak references internally for early cycle reclamation;
  Tel code never sees one. The narrow cases that might have reached for them are
  served otherwise: (a) early reclamation of an intra-task cycle is the host's
  GC strategy, not a language feature; (b) non-retaining caches/observer lists
  and (c) non-owning parent↔child back-pointers use `Id`-indirection (a
  `Map[Id, Node]` resolves them) or stdlib `Sync` cells. Re-open only if a
  concrete case forces it — which would be a Tel2-scale change, not a tweak.
- TODO(open): Whether the copy at a task boundary is *eager* (at send) or
  *lazy* / copy-on-write. Lazy copy-on-write would cut the cost of sending
  large mostly-read structures; it is an implementation freedom the
  "behaves-as-if-isolated" spec already permits, but it is worth stating
  whether the docs guarantee anything observable about timing.
- TODO(open): How task-affinity is surfaced in error messages and whether a
  script can *query* whether a value is shareable. Lean: no runtime query —
  it is a compile-time property.
- TODO(open): Interaction with [FFI](../16-ffi-and-interop/) for host values
  that are neither plain immutable data nor explicitly task-affine — the
  default classification for an opaque host handle needs a rule. Lean:
  treat an opaque host handle as task-affine unless the binding says
  otherwise (safe default).
