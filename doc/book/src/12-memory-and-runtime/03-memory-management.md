# Memory Management

TODO: review

Tel makes **no promise about how memory is managed** — only that management is
**fully automatic**. There is no `free`, no `delete`, no manual cleanup. A
script never reclaims memory by hand; *how* reclamation happens is left to the
host implementation.

This follows *high abstraction over low-level control*: a script author
reasons about values, not about lifetimes or ownership ceremony. See
[`../02-philosophy/01-priorities.md`](../02-philosophy/01-priorities.md).

This is about **memory** specifically. *Host resources* — file handles,
sockets, capabilities — are a separate concern, with their own explicit,
scoped cleanup (see the open question at the end of this topic, and
[`../13-error-handling/`](../13-error-handling/)). "No ownership ceremony"
here means *no ceremony for freeing memory*; it is not a claim that nothing
else in Tel is governed by ownership-like rules.

## What Tel guarantees

Across every host, a Tel program can rely on exactly this:

- **Reachable memory is never reclaimed.** Anything still reachable from live
  code stays valid — there are no dangling references.
- **Unreachable non-cyclic memory is reclaimed eventually.** Once an object
  that is not part of a reference cycle becomes unreachable, it will probably
  be freed — but *when* is unspecified.
- **Unreachable cyclic memory may or may not be reclaimed.** Memory trapped in
  a reference cycle might be collected, or might leak for the lifetime of the
  program. A program must not depend on cycles being freed.
- **All memory is reclaimed when the program ends.** When an embedded Tel
  script finishes, every byte it allocated is released wholesale — see
  [Bulk cleanup at script end](#bulk-cleanup-at-script-end).

Nothing else is promised: not the timing of reclamation, not its order, not
whether collection pauses the program.

## Why so little is promised

Tel is embedded in many host runtimes. Some hosts already have a garbage
collector (a JS engine, the JVM, a CLR), and the natural implementation reuses
it. Other hosts ship Tel with its own runtime and a custom collector. Pinning
a single strategy — reference counting, tracing GC, arenas — would either
fight the host or constrain the embedder for no user-visible benefit.

Leaving the strategy open lets each host pick what fits while the user-facing
language stays identical everywhere: no `'a` lifetime syntax, no ownership
ceremony, no `free`. (Lifetimes exist as a structurally-propagated concept for
borrows, but are named, not spelled — see [Lifetimes](05-lifetimes.md).)

## Single-threaded, per-fiber heaps

Although the *strategy* is unspecified, the *shape* of the heap is constrained,
because it interacts with concurrency and with failure handling.

Tel's concurrency model gives **each task/fiber its own heap** (see
[`../14-concurrency-and-parallelism/`](../14-concurrency-and-parallelism/02-tasks.md)).
A fiber's heap is touched only by that fiber, so:

- **The garbage collector is single-threaded per heap.** It never has to
  stop-the-world across fibers or synchronise with other collectors. This
  keeps the collector simple and keeps the common case — a small script
  running to completion — fast.
- **No data is shared mutably between fibers.** Sending a value from one fiber
  to another is a deep copy into the receiving fiber's heap, never a shared
  reference. This is what makes per-heap, single-threaded collection sound.

A host is free to *implement* the heaps as one shared arena that merely
*behaves* as if isolated — the isolation is a guarantee about behaviour, not a
mandate to physically separate. The reverse (a genuinely shared heap presented
as isolated) is fine; presenting a shared heap *as shared* is not, because then
concurrent collection would be observable.

## Failure drops a whole heap

Per-fiber heaps make failure handling cheap. When a fiber fails (panics /
aborts — see
[`../13-error-handling/04-panics-and-aborts.md`](../13-error-handling/04-panics-and-aborts.md)),
its **memory** is reclaimed **wholesale** — there is no need to walk the stack
freeing values one by one. A [cleanup
unwind](../13-error-handling/04-panics-and-aborts.md#cleanup-and-the-abort-path)
does run first, settling live linear *resources* by running their NoPanic
`AutoUse`/`finally` actions, but that discharges must-settle obligations; it is
not about reclaiming memory, because:

- Tel has no *recovering* unwinding — a failed fiber cannot catch the panic or
  resume, and the cleanup unwind runs only NoPanic settle actions for live
  linear resources. Ordinary in-heap values run no per-value destructor; they
  are dropped in bulk.
- Everything the fiber allocated lived in *its* heap and nothing else, so
  discarding that heap reclaims all of it at once and cannot dangle a
  reference held elsewhere.

This is the Erlang-style "let it crash" arrangement: a failure is contained to
one fiber with its own memory, and the rest of the program is unaffected. It
also means the runtime never has to reason about half-constructed values left
behind by a failure.

What bulk-dropping a heap does *not* release on its own is **host resources** —
file handles, sockets, and other capability-backed things. A host resource
**owned by a linear value** is settled by that value's `AutoUse`/`finally` on the
cleanup unwind above. One that **no linear value owns** is the host's to reclaim;
see the open question below and
[`../16-ffi-and-interop/`](../16-ffi-and-interop/04-embedding-tel-in-a-host.md).

## Bulk cleanup at script end

The same mechanism applies to a whole script. A typical embedded Tel script
runs one task — a per-message transform, a valuation, a modding hook — and then
finishes. When it does, the runtime reclaims **all** of its memory in one step;
it does not need to have collected anything incrementally during the run.

For short-lived scripts this can make a "collector" almost trivial: allocate
freely, never collect mid-run, drop everything at the end. A host running many
short scripts gets clean isolation between them for free.

## No way to prevent leaks — and no `forget`

A program *can* still leak in the practical sense: a reference cycle may never
be collected, and a value parked in a long-lived collection and never removed
is retained on purpose as far as the runtime can tell. Tel does not try to
prevent this and does **not** provide a `forget`-style operation to explicitly
abandon a value — there is nothing for it to do that dropping the last
reference does not already do, and per-fiber bulk cleanup bounds the damage
anyway. *Serves: one good way over many clever ones.*

## Eager drop

Tel reclaims a value's logical lifetime at its **last use**, not at the end of
its lexical scope. Once a binding is provably never read again, the runtime is
free to release it immediately.

Eager drop matters for **tail-call optimisation**. A call in tail position can
only reuse the current frame if nothing in that frame still needs to live
across the call. Dropping bindings at last use — rather than at scope end —
means a recursive tail call commonly has nothing left alive, so the frame can
be reused and the recursion runs in constant stack space.

```tel
fn sum(items: List[Int64], acc: Int64) -> Int64 {
    match items.split_first() {
        None        => acc
        Some(h, t)  => sum(t, acc + h)   # `items`, `h` dead before the call
    }
}
```

Eager drop is an observable-as-*timing* choice, not an observable-as-*behaviour*
one: because reclamation timing is already unspecified, eager drop never
changes results. It is described here because it is the mechanism that makes
TCO reliable.

TODO(open): deterministic release of *host* resources that are **not** owned by
a linear value. A host resource wrapped in a linear (relevant) type is settled by
its `AutoUse`/`finally`, which now runs on the [cleanup
unwind](../13-error-handling/04-panics-and-aborts.md#cleanup-and-the-abort-path)
on fiber failure too — so the common case (a `File`/socket held as a linear
resource) is resolved. What remains open is capability-backed host state that no
linear value owns: a `with`-style scope may release it at a defined point, and
whether that scope also runs on failure. This is a philosophy-chapter gap: the
antifeatures list should say host resources are released by linear settling on
both the normal and abort paths, with only the un-owned remainder left to the
host.

## See also

- [Stack and Heap](02-stack-and-heap.md) — placement is also automatic and
  host-chosen.
- [Runtime Representation](06-runtime-representation.md) — IR metadata that
  helps a backend manage memory well.
- [`../14-concurrency-and-parallelism/02-tasks.md`](../14-concurrency-and-parallelism/02-tasks.md)
  — per-fiber tasks, the unit that owns a heap.
- [`../13-error-handling/04-panics-and-aborts.md`](../13-error-handling/04-panics-and-aborts.md)
  — failure semantics that bulk heap-dropping supports.
