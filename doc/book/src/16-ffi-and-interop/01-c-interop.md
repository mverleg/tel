# C Interop

TODO: review

Tel has **no direct C FFI**. A Tel script cannot declare `extern` functions,
cannot load a shared library, cannot pass raw pointers, and cannot describe a
C struct layout. This is deliberate.

## Why there is no C FFI

A C FFI would require exactly the things Tel rules out as antifeatures (see
[`../02-philosophy/04-antifeatures.md`](../02-philosophy/04-antifeatures.md)):
raw pointers, addresses, manual memory layout, an `unsafe` escape hatch, and a
stable ABI. It would also break the sandbox: a script that can `dlopen` a
library has ambient power the host never granted.

Tel is a **guest** language. Anything native — including C code — is reached
*through the host*, never directly:

- The host links whatever native libraries it needs, in the host's own
  language, with the host's own FFI.
- The host then exposes the slice of that functionality it chooses, as
  **host operations / capabilities**, to the script.
- The script calls those operations as ordinary host functions, with immutable
  values crossing the boundary.

So "calling C from Tel" is really "the host calls C, and exposes a capability
for it." A hot inner loop that must run native code lives in the host language
behind that capability boundary — see
[`../01-overview/02-when-to-use-tel.md`](../01-overview/02-when-to-use-tel.md).

## See also

- [Embedding Tel in a Host Application](04-embedding-tel-in-a-host.md) — how
  the host exposes operations and capabilities to a script.
- [Calling Conventions and ABI](02-calling-conventions-and-abi.md) — why there
  is no ABI.
- [`../02-philosophy/04-antifeatures.md`](../02-philosophy/04-antifeatures.md)
  — no low-level machine access, no ambient I/O.
