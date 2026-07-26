# Compile Targets

<!-- TODO: review -->

## What

Tel is designed to run inside hosts written in many languages, and to run
either interpreted or compiled ahead of time. To make that practical, the
compiler does not emit code for any target language directly. Instead, every
target sits behind a stable intermediate representation called **xolir**
(*cross-language intermediate representation*, sometimes written *xo l ir*;
formerly called *TelIR*).

The compile pipeline ends at xolir. Everything that turns xolir into something
executable — a Java class file, a JavaScript bundle, a WebAssembly module, a
Rust crate, an interpreter image — is a **backend** that consumes xolir.

```
Tel source
   │
   ▼  lex, parse, resolve, type-check
front-end
   │
   ▼  lower
 xolir  ──►  interpreter
   │
   ├───►  Rust backend
   ├───►  Java/JVM backend
   ├───►  JavaScript / WebAssembly backend
   ├───►  Python backend
   └───►  ... (other hosts, written in any language)
```

A backend is **not required to be written in the same language it targets**: a
Python backend may itself be a Rust program, a JVM backend may be written in
Kotlin, and so on. The only contract is "consume xolir, emit code for the
chosen host."

For the user-facing two-execution-mode story (interpret vs AOT) see
[`01-compiler.md`](01-compiler.md) and
[`../01-overview/03-goals-and-non-goals.md`](../01-overview/03-goals-and-non-goals.md).

## Why a cross-language IR

Three reasons follow directly from Tel's priorities
([`../02-philosophy/01-priorities.md`](../02-philosophy/01-priorities.md)):

- **One script, many hosts.** Tel's standout case is the same script running
  bit-identically inside a Java backend, a JavaScript bundle, an iOS app, etc.
  That only scales if adding a new host means writing one backend — not
  reworking the front-end. Xolir is the seam.
- **The front-end stays the place truth lives.** Parsing, name resolution,
  type-checking, and trait dispatch are subtle, version-sensitive, and have to
  agree across hosts. Concentrating them above xolir means every backend
  inherits the same semantics, even when written by a different team in a
  different language.
- **Backends can be separate processes.** Xolir is serializable, so a backend
  may live in another process — useful for sandboxing untrusted codegen, for
  running an expensive optimiser out-of-band, or for distributing
  pre-checked Tel modules to constrained hosts that only ship a backend, not a
  full compiler.

A non-goal: xolir is **not** an optimisation IR in the LLVM sense. It is small
and explicit enough to be a clean codegen target across very different host
runtimes; deeper optimisation is the host runtime's job (the JVM JIT, V8,
LLVM, etc.). See *non-goals* below.

## What xolir guarantees

When a backend receives xolir, it can assume:

- **The program type-checks.** All Tel-level type errors have been rejected
  upstream. A backend never needs to re-run Tel's type system; types in xolir
  are concrete and resolved.
- **All names are resolved.** No unresolved imports, no overload ambiguity, no
  trait-dispatch uncertainty. Every call site references a concrete
  implementation (or an explicit dynamic dispatch node).
- **Generic dispatch is resolved; representation is the backend's call.** All
  trait dispatch and type-parameter resolution is done upstream (previous
  bullet) — xolir names a concrete implementation at every call site. But *how*
  a generic is represented in the target — monomorphised into per-type copies,
  passed a runtime type witness/dictionary, or a mix — is deliberately **not**
  pinned by xolir, because backends differ in what they support natively: a JVM
  backend can lean on the host's (erased) generics, a native backend may
  monomorphise for speed, a size-constrained target may prefer witnesses to
  avoid code bloat. So xolir carries generics in a *representation-agnostic*
  form — enough resolved type structure for a backend to monomorphise *or* to
  pass witnesses — and each backend picks. `TODO(open): the exact encoding —
  how much type structure xolir must retain so every backend strategy stays
  possible without re-deriving dispatch.`
- **Effects and capabilities are explicit.** I/O happens only through
  capabilities the host passes in
  ([`../02-philosophy/03-features.md`](../02-philosophy/03-features.md)); xolir
  carries that structure through so backends cannot accidentally introduce
  ambient I/O.
- **Stable across Tel versions, within Tel1.** Because Tel itself is frozen at
  1.0 ([priorities](../02-philosophy/01-priorities.md)), xolir aims for the
  same stability: an old xolir file should still run through a newer backend,
  and a new xolir file should run through any backend that supports its xolir
  schema version. `TODO(open): xolir's own versioning story vs Tel's "no
  editions" rule — exact compatibility window for old xolir files needs to be
  spelled out.`
- **Determinism upstream is preserved downstream.** If two Tel scripts produce
  the same observable behaviour under the language spec, they should
  consistently lower to behaviour-equivalent xolir.

## What xolir does not guarantee

- **Not a performance contract.** Xolir does not promise any particular
  asymptotic, allocation, or instruction count. Two backends are allowed to
  produce code with very different performance characteristics from the same
  xolir input.
- **Not an ABI.** Xolir is for codegen; it is not a calling convention or a
  binary layout shared between hosts. Cross-host interop goes through the
  host's FFI (see
  [`../16-ffi-and-interop/01-c-interop.md`](../16-ffi-and-interop/01-c-interop.md)),
  not by linking xolir-produced artifacts together.
- **Not user-facing.** Tel programmers should never have to read or write
  xolir. Diagnostics surface in terms of Tel source, not xolir nodes. Tools
  may expose xolir for debugging the compiler itself.
- **Not a stable extension point for backends to mutate.** Backends consume
  xolir; they do not amend it in place and pass it on. A "backend that
  rewrites xolir" is really a front-end pass and belongs upstream.

## How backends consume it

A backend is essentially a function `xolir.Module -> host code`:

- It walks the xolir module, emitting code for each declaration.
- It maps Tel types to host types (`Int32` → Java `Int`, JS `number`, Rust
  `i32`, …) according to a per-target lowering table.
- It implements the runtime support each xolir construct needs in the host
  language: union dispatch, trait tables, task scheduling, contract checks,
  and so on.
- It exposes capabilities by binding host-side implementations to the
  capability slots xolir declares.

Backends are expected to be **dumb and direct** — translation, not
optimisation. Anything clever (inlining, escape analysis, loop transforms) is
left to the host runtime, which already has battle-tested optimisers.

### The interpreter as a backend

The reference interpreter is just another backend over xolir: it walks xolir
nodes and evaluates them. This keeps interpreter and AOT-compiled behaviour
trivially aligned — both consume the same lowered program.

`TODO(open): the interpreter likely needs a small execution-oriented form
(byte-coded or threaded) for speed. Whether that form is xolir directly,
xolir-after-a-final-pass, or a separate "vm-ir" is an implementation choice.
See [`../impl-notes/intermediate-representation.md`](../impl-notes/intermediate-representation.md).`

### Adding a new host

Adding support for a new host language is, by design, a backend project:

1. Pick a host (say, Lua, or .NET).
2. Implement a xolir consumer in any convenient language.
3. Provide the standard set of capability bindings the host wants to expose.
4. Run the shared Tel test suite against it.

The front-end does not change.

## Schema and serialization

Xolir is specified as a schema (currently leaning on Protobuf3)
that compiles to client libraries in the major host languages. The schema
choice is driven by two needs:

- **Fast codegen targets in many languages.** Protobuf has mature language
  bindings, so a Rust backend, a Java backend, and a TypeScript backend can
  all consume the same xolir file without each writing a parser.
- **Compactness and speed over self-description.** Tooling that needs to
  introspect the IR can do so against the source-of-truth schema definitions;
  runtime introspection through e.g. JSON Schema would be more discoverable
  but slower and bulkier.

`TODO(open): final schema format — Protobuf3 is the working choice, but the
trade-off against alternatives (Cap'n Proto, FlatBuffers, a custom binary
format) is not yet documented. See
[`../impl-notes/intermediate-representation.md`](../impl-notes/intermediate-representation.md).`

`TODO(open): publishing and packaging the schema client libraries (cargo,
pypi, maven, npm) is an implementation detail; the user-facing promise is
"your backend's language has a xolir client." Concrete coordinates live in
[`../impl-notes/intermediate-representation.md`](../impl-notes/intermediate-representation.md).`

## Cross-references

- [Compiler](01-compiler.md) — pipeline that produces xolir.
- [Embedding Tel in a Host Application](../16-ffi-and-interop/04-embedding-tel-in-a-host.md)
  — how a host wires up a backend and capabilities.
- [Goals and Non-Goals](../01-overview/03-goals-and-non-goals.md) — the
  "two execution modes, identical behaviour" goal that xolir exists to serve.
- [`../impl-notes/intermediate-representation.md`](../impl-notes/intermediate-representation.md)
  — scratchpad for xolir's concrete shape (not part of the documentation).
