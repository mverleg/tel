# Binding Other Languages

TODO: review

Tel is designed to be embedded in **many host languages** — a Java backend, a
JavaScript browser bundle, a Rust service, a Python tool — and to behave
identically in each. This page covers how a Tel script binds *to* whatever host
language is running it.

## One frontend, many backends

Binding works the same way regardless of host language, because the binding is
**source-level**, not ABI-level (see
[Calling Conventions and ABI](02-calling-conventions-and-abi.md)):

- Parsing and type-checking happen once, in the shared Tel frontend, producing
  the Xolir IR.
- A per-target backend turns that IR into the host language — or runs it on a
  small interpreter embedded in the host.

So "binding Python" and "binding the JVM" are two backends over one frontend.
There is no per-language reimplementation of the language itself, which is what
keeps behaviour identical across hosts.

## What "behaves identically" requires

For one script to mean the same thing in every host, the binding layer must
agree on a small, fixed contract — this is best pinned down
as a **spec**, separate from any one compiler implementation. Since the
compiler is just one implementation, things like type-inference details do not
matter for cross-host equivalence; **runtime behaviour** does. The spec should
nail down in particular:

- Which functionality a **host must provide** for a conforming embedding.
- How a host **exposes operations, types, and capabilities** to a script (see
  [Embedding Tel in a Host Application](04-embedding-tel-in-a-host.md)).
- How values **cross the boundary** — only immutable types, and how a
  Tel-implemented type reaches the host when the host lacks a native
  equivalent.
- How the boundary crossing differs between **interpreted and compiled** mode
  while staying observably identical.

TODO(open): whether Tel ships a **conformance test suite** that a new host
binding must pass — and how such a suite runs across languages — is unresolved.
It is the natural way to keep "behaves identically" honest, but its design is
non-trivial.

## Binding direction

The binding is two-way, mirroring the two module APIs at the boundary (see
[Embedding Tel in a Host Application](04-embedding-tel-in-a-host.md)):

- **Host → script.** The host instantiates the Tel runtime, supplies inputs,
  grants capabilities, and invokes the script's entry point.
- **Script → host.** The script calls host-exposed operations and returns
  immutable results.

Neither direction exposes raw pointers, host object identity, or a binary
layout — everything is mediated by the source-level boundary contract.

## Implementation language of the backends

A practical, non-user-facing point: a backend's own
implementation language need not match the language it generates code *for*. A
single Rust-implemented toolchain could emit Java, JS, and Python; or each
backend could be written in its target language so that host-language
communities can contribute their own. This is a toolchain-architecture
decision and its details belong in `impl-notes/`, not in user documentation.

## See also

- [Embedding Tel in a Host Application](04-embedding-tel-in-a-host.md)
- [Calling Conventions and ABI](02-calling-conventions-and-abi.md)
- [`../01-overview/02-when-to-use-tel.md`](../01-overview/02-when-to-use-tel.md)
  — the one-script-many-hosts use case.
