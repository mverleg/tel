# Stack and Heap

TODO: review

Tel deliberately keeps the **stack/heap distinction out of the surface
language**. A script author should be able to write a 30-line modding hook or a
medium data transform without ever deciding whether a value "lives on the
stack" or "is heap-allocated." This follows directly from the priority
*high abstraction over low-level control* — Tel sits closer to Python than to C
(see [`../02-philosophy/01-priorities.md`](../02-philosophy/01-priorities.md)).

## What the script sees

Nothing. There is no `box`, no `new`, no allocator argument, no `&`/`*`, no
"this struct is by-value but that one is by-reference" rule to memorise. A
value is a value; the program reasons about values and types, never about the
bytes underneath them — see
[`../02-philosophy/04-antifeatures.md`](../02-philosophy/04-antifeatures.md).

This is also what lets one script run unchanged across very different host
runtimes. An interpreter, a JIT, and an AOT compiler targeting Rust or wasm
will each make different placement choices; if placement leaked into the
language, the same script would behave differently per host.

## What the compiler decides

Placement is an *implementation* concern, decided per value by the backend
using escape analysis and the per-variable IR metadata described in
[Runtime Representation](06-runtime-representation.md). Typical choices:

- A value whose lifetime is bounded by a scope and that never escapes can be
  placed on the call stack.
- A small, cheaply-copied value may live entirely in registers or inline in
  its parent.
- A value that outlives its defining scope, or whose size is not known
  statically, goes on the heap.

None of this is promised to the script, and none of it is stable across hosts.
A program must never depend on a particular placement.

## Indirection is automatic too

Because there is no `Box`, the compiler — not the author — inserts whatever
indirection a value needs. Three cases force it, and all are handled silently:

- **Unsized values.** A trait object (`dyn Trait`) has no statically known
  size, so it lives behind a pointer the compiler adds.
- **Recursive types.** A self-referential type — `Json = (... | List[Json])`, a
  tree node holding children of its own type — is infinite-size if stored flat.
  Languages that expose the heap make the author break the cycle by hand
  (Rust's `Box<Node>`); Tel inserts the indirection itself, so the type is
  written as if it were flat.
- **Values too large to copy.** Value semantics says assignment is a logical
  copy, but the compiler may realise a large copy as copy-on-write, or as a
  move when the source is dead. "Too big to copy" therefore never forces a
  physical copy and never changes behaviour.

In every case the indirection is invisible and value semantics is preserved;
only performance is affected.

TODO(open): hiding all allocation trades away *performance predictability*,
which some embedding hosts (game engines, real-time audio) care about. The
language surface stays clean; any "where did this allocate" visibility belongs
to tooling/profilers and `impl-notes/`, not the language — same stance as
`Text` and numeric representation. Re-confirm this is acceptable for
latency-sensitive hosts.

## Placement is not the value/reference distinction

Hiding the stack/heap choice does **not** mean every value behaves alike. Two
axes are kept apart, and only the first is hidden:

- **Placement** (stack / heap / inline / interned) — never user-visible, as
  above. A pure performance concern.
- **Value vs reference semantics** — *visible*, but carried by a type's
  **mutability and sharing kind**, never by where its bytes live:
  - an **immutable value** (`T`) has pure value semantics; aliasing it is
    unobservable, so "is this shared?" is a question with no answer — this is
    why "as if on the heap" is meaningless for it;
  - a **unique mutable builder** (`!T`) has a single live owner, and an
    exclusive lend (`&!T`) lets a callee mutate that one underlying object in
    place — observable and intended, but never aliased;
  - the **shared mutable** types from `std` (e.g. a concurrent map) genuinely
    behave as references: many holders see each other's writes. These are a
    curated, stdlib-only set — user code cannot define its own aliased-mutable
    type.

So reference-like behaviour, where it exists, comes from a value's mutability
kind, not from a placement you could observe. See
[Value vs Reference Semantics](01-value-vs-reference-semantics.md).

## Short-string optimisation

A concrete, allowed instance of placement freedom: **short strings may be
stored inline** rather than behind a heap pointer. A `Text` short enough to fit
in the space a pointer-plus-length would occupy can sit directly on the stack
or inline in its containing value, avoiding an allocation and a dereference.

This is purely a representation choice and is described, with the other string
representation details, in [Runtime Representation](06-runtime-representation.md).
It changes nothing observable: a short string and a long string are the same
type and behave identically.

## Guaranteeing a value on the parent stack ("super return")

One case where placement *would* matter to a script: a function that wants to
guarantee its return value is constructed directly in the **caller's** stack
frame rather than allocated and copied/moved out. This is necessary for
self-referential stack values and useful for avoiding an allocation in hot
return paths.

One idea: an opt-in **`super return`** marker on a function
signature. The function promises its result is built in place on the parent
frame; if the compiler cannot honour that (because, say, the value's size is
not statically known, or it escapes further), compilation **fails loudly**
rather than silently falling back to a heap allocation.

```tel
# Opt-in: result is constructed in the caller's frame, or compile error.
super return fn make_buffer() -> Buffer { ... }   # TODO(open): spelling
```

TODO(open): `super return` is a low-level escape hatch and sits in tension with
*high abstraction over low-level control* and with the idea that placement is
never script-visible. Re-justify against embedding: does an embedded scripting
language genuinely need self-referential stack values, or is this a pre-pivot
idea aimed at standalone systems work? If kept, the exact spelling, the precise
conditions under which the guarantee holds, and the interaction with value
semantics all need to be pinned down. Leaning: defer until a real use case
demands it.

## See also

- [Value vs Reference Semantics](01-value-vs-reference-semantics.md) — why
  copies are conceptual, which is what makes placement free to vary.
- [Runtime Representation](06-runtime-representation.md) — short-string
  optimisation and the IR metadata driving placement.
- [Memory Management](03-memory-management.md) — how heap values are reclaimed.
