# Allocators

TODO: review

Tel has **no script-visible allocators**. There is no allocator type, no
allocator argument threaded through collections or constructors, no global
allocator to swap, and no way for a script to choose how memory is obtained.

This follows directly from *high abstraction over low-level control* (see
[`../02-philosophy/01-priorities.md`](../02-philosophy/01-priorities.md)) and
from [Memory Management](03-memory-management.md): allocation strategy is a
host/runtime concern, deliberately unspecified so each host can do what fits.
A host with its own GC, a host reusing the JVM or a JS engine, and a host
running short scripts with bulk cleanup at the end all want different
allocation strategies — pinning one, or exposing the choice in the language,
would constrain embedders for no user-visible benefit.

Allocator-parameterised collections (Rust's `Vec[T, A]`, C++'s allocator-aware
containers) are a deliberate **non-feature**. They are aimed at standalone
systems work, not at embedded scripting, and they would leak an implementation
choice into every collection signature.

## Where allocation behaviour *is* discussed

The runtime still makes allocation-related choices — they are just not the
script's to make:

- **Stack vs heap placement** — chosen by the backend per value; see
  [Stack and Heap](02-stack-and-heap.md).
- **Per-fiber heaps and bulk cleanup** — each fiber allocates into its own
  heap, dropped wholesale on failure or at script end; see
  [Memory Management](03-memory-management.md).
- **Short-string and similar inline representations** — see
  [Runtime Representation](06-runtime-representation.md).

## See also

- [Memory Management](03-memory-management.md)
- [Stack and Heap](02-stack-and-heap.md)
- [`../02-philosophy/04-antifeatures.md`](../02-philosophy/04-antifeatures.md)
  — no low-level machine access.
