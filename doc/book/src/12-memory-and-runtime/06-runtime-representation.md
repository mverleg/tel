# Runtime Representation

TODO: review

How a Tel value is laid out in memory is an **implementation concern**, not a
language feature. A script reasons about values and types; it never sees the
bytes. But Tel still makes deliberate choices about representation, for two
reasons: some choices (like short-string storage) are worth allowing
explicitly, and the [Xolir IR](#per-variable-ir-metadata) carries metadata that
lets each backend pick a good representation. This page collects the
representation decisions that are settled enough to record.

## Per-variable IR metadata

Tel's frontend (parsing, name resolution, type checking) is shared; codegen is
per-target and may run in a different process. The boundary between them is
**Xolir**, a serializable cross-language IR. To let backends — especially ones
targeting low-level languages like Rust or wasm — generate good code, Xolir
annotates **each variable** with metadata the frontend has proven:

- **Mutability** — is the value ever mutated? Drives, e.g., a plain value vs a
  cell/`Mutex` in the target. Mutability is one-way: it may be recorded as
  "true for the first part of the lifetime, false after," but never the
  reverse — see [References and Aliasing](04-references-and-aliasing.md).
- **Aliasing** — is the value reachable from one name, several names in one
  fiber, or possibly more than one fiber? Drives plain value vs `Rc` vs an
  atomically reference-counted pointer. See
  [References and Aliasing](04-references-and-aliasing.md).
- **Scope-based lifetime** — is the value's lifetime bounded by a lexical
  scope, or does it escape? Drives stack vs heap placement; see
  [Stack and Heap](02-stack-and-heap.md).

Two principles govern this metadata:

- **Conservative by default.** Every field starts at its most-general value
  (mutable, possibly shared, escaping) and is only tightened when analysis
  proves it. A wrong-toward-general answer is merely slower; a wrong-toward-
  specific answer would be unsound.
- **Not all combinations are independent.** For example, a value proven
  non-aliased and scope-bounded is a strong candidate for the stack; a value
  proven immutable can be shared regardless of aliasing. The backend reads the
  metadata as a whole.

This analysis assumes a reasonably capable compiler that does escape analysis.
A simple interpreter backend may ignore the metadata entirely and box
everything — the metadata is an *optimisation channel*, never a correctness
requirement. Cheaply-copied small values may be handled specially: copying them
is so cheap that aliasing metadata barely matters.

TODO(open): whether every combination of (mutability × aliasing × lifetime) is
representable, and exactly how a backend should resolve conflicting hints, needs
to be pinned down once a real backend exists. Detailed IR shape belongs in
`impl-notes/`, not here.

## Text representation

`Text` is one type with one set of behaviours, but the runtime is free to store
it in more than one physical form. Two representation choices are recorded
because they are settled in intent (the exact thresholds are not):

### Short-string optimisation

A string short enough to fit inline — in the space a heap pointer plus a length
would otherwise occupy — may be stored **directly inside its container** (on
the stack, or inline in a struct) with no separate heap allocation and no
dereference. Long strings live behind a pointer as usual.

This is invisible to scripts: a short string and a long string are the same
type and compare, concatenate, and iterate identically.

### Length without capacity

If a string's backing buffer is always grown to the next power of two, then the
capacity is implied by the length and need not be stored separately — storing
just the length is enough. This shrinks the string header, which makes
short-string optimisation more effective (more inline bytes available).

### Stored prefix or hash

In comparison-heavy contexts — a sorted collection, a tree-backed map keyed by
strings — most comparisons fail on the first few characters. Keeping a small
**prefix** (and/or a hash) of the string inline, next to the pointer, lets many
comparisons resolve without chasing the pointer at all. A stored prefix
composes naturally with short-string optimisation: the bytes that would hold
the prefix *are* the inline storage for a short string.

TODO(open): these are representation strategies, not guarantees. Decide which
(if any) Tel *requires* of a conforming runtime versus merely permits. Exact
inline-size thresholds are an implementation detail and belong in
`impl-notes/`.

## The "do not optimize away" block

Some code must run exactly as written even though an optimiser would normally
be entitled to delete it as having no observable effect. Two cases recur:

- **Security zeroing.** Overwriting a buffer that held a password or key. A
  dead-store optimiser sees the buffer is never read again and deletes the
  overwrite — defeating the point.
- **Benchmarking.** A microbenchmark computes a value only to measure the
  computation; an optimiser that proves the result is unused may delete the
  whole computation.

Tel therefore provides a way to mark a block as **not to be optimised away** —
the compiler and every backend must emit it even if it appears dead.

```tel
do_not_optimize {                 # TODO(open): spelling of the construct
    secret_buffer.zero_fill()
}
```

This is a *codegen directive*, not low-level machine access — it asks the
compiler to *refrain* from an optimisation, it does not expose pointers,
intrinsics, or the memory model, so it does not conflict with the "no low-level
machine access" antifeature. It must still behave identically across hosts:
every conforming backend honours it.

TODO(open): exact spelling and scope — a block, an attribute, or a stdlib
function such as a `black_box`-style identity function. Also unresolved:
whether it constrains only Tel's own optimiser or is a contract a host's
downstream optimiser (a JS engine, LLVM) must also respect — the latter may not
be fully enforceable, in which case the docs must say so honestly.

## See also

- [Stack and Heap](02-stack-and-heap.md) — placement, driven by the lifetime
  metadata above.
- [References and Aliasing](04-references-and-aliasing.md) — the mutability and
  aliasing metadata in detail.
- [Value vs Reference Semantics](01-value-vs-reference-semantics.md) — why
  representation can vary without changing behaviour.
