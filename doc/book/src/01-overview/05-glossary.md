# Glossary

<!-- TODO: review -->

Short definitions for terms used throughout the documentation. Where Tel
borrows a term from another language community, the *Also known as* line
lists the names you may have seen elsewhere.

## Tel-specific terms

**Tel** — Typed Embedded Language. The language this documentation describes.
*See also:* [Introduction](01-introduction.md).

**Tel1** — Internal working name for "Tel 1.0," reflecting the commitment
that the next breaking change would be a *separate* language called Tel2,
not a new version of Tel.
*See also:* [Priorities](../02-philosophy/01-priorities.md).

**Host** — The application embedding the Tel runtime. The host owns the OS,
the build, the process lifecycle, and decides what capabilities a script
receives. A game engine, an IDE plugin, a backend service, or a browser bundle
can all be hosts.

**Capability** — An object the host hands to a script that authorises a
specific external effect: a filesystem, a clock, a logger, a network client.
A script can only do what its capabilities allow. *See also:*
[Features](../02-philosophy/03-features.md).

**Xolir** — The cross-language intermediate representation Tel lowers to
before backends consume it. Pronounced *xo-l-ir*. Formerly called
*TelIR*. *See also:* [Compile Targets](../18-tooling/02-compile-targets.md).

**Backend** — A code generator (or interpreter) that consumes xolir and
produces something runnable in a host language (JVM bytecode, JS, WASM,
Rust, native code, …). *See also:*
[Compile Targets](../18-tooling/02-compile-targets.md).

## Data shapes

**Record** — A value built from named fields. Tel's product type.
*Also known as:* struct, class, data class, product type.
*See also:* [Records](../10-data-modelling/01-records.md).

**Union** — A type whose values are values of one of several member types.
Tel's sum type. Untagged: the type itself acts as the tag.
*Also known as:* sum type, enum, enumeration, oneof, sealed class.
*See also:* [Union Types](../10-data-modelling/02-union-types.md).

**Trait** — A description of behaviour that types can implement.
Polymorphism in Tel goes through trait dispatch, not inheritance.
*Also known as:* interface, protocol, type class.
*See also:*
[Traits or Interfaces](../10-data-modelling/03-traits-or-interfaces.md).

**Intersection type** — A type bound that combines several trait requirements
(`T: Add + Mul`). Used as a bound on type parameters, not as a concrete type
to construct values of.

**Type parameter** — A placeholder for a type, used to write code that works
across many types. *Also known as:* type argument, generic parameter.
*See also:* [Generics](../05-types/07-generics.md).

**Refined type** — A wrapper type adding a meaning or invariant to an
existing type (`Id[Person]`, `EuroAmt`, `NonEmpty[List[T]]`).
*Also known as:* newtype, branded type, opaque alias.

## Control and errors

**Result** — A union of *Ok* (success carrying a value) and *Err* (failure
carrying an error). Tel's standard way to return fallible results.
*Also known as:* Either, Try, expected.
*See also:*
[Error-Handling Philosophy](../13-error-handling/01-philosophy.md).

**Option** — A union representing "value or absent." Tel uses an `Option`
type instead of nullable references.
*Also known as:* Maybe, Optional, nullable.

**Match** — Tel's case-analysis construct. Pattern-matches a value against a
set of arms; exhaustive by default.

**Task** — A unit of concurrent work. Tel deliberately avoids naming
threads, fibers, or microtasks — a task is whatever the host's runtime
decides to make it.
*See also:*
[Concurrency overview](../14-concurrency-and-parallelism/01-overview.md).

## Tooling

**Compile-target** / **target** — A host language or runtime that Tel can
emit code for via a backend. *See also:*
[Compile Targets](../18-tooling/02-compile-targets.md).

**Clean / bulk mode** — The compile mode tuned for throughput (no
fine-grained caches, terse diagnostics). *See also:*
[Compiler](../18-tooling/01-compiler.md).

**Incremental / friendly mode** — The compile mode tuned for editor use:
symbol-level caching, rich diagnostics, continues past first error. Backs
the LSP server.
