# Reflection

<!-- TODO: review -->

**Tel has no reflection.** There is no runtime type introspection, no `eval`,
no dynamic class loading, and no calling a function by a name assembled from a
string. This page records *what* is excluded and *why*, so the decision is not
re-litigated.

## What is excluded

- **No `eval`.** Tel source cannot be compiled and run from within a running
  Tel program.
- **No runtime type inspection** beyond the minimum that
  [trait dispatch](../10-data-modelling/03-traits-or-interfaces.md) needs
  internally. A program
  reasons about static types, not about a value's type discovered at runtime.
- **No dynamic loading** of code by name at runtime.
- **No call-by-string** — you cannot look up a function or field from a string
  built at runtime.
- **No downcasting of open types.** Turning a type-erased value back into a
  guessed concrete type (Rust's `Error::downcast_ref`) is not supported for
  open types. The reason: downcasting an open type makes the
  concrete implementation type a hidden API dependency, so changing it becomes
  a stealth breaking change. Pattern-matching over a *sealed* union remains the
  supported way to recover a specific case — see
  [union types](../10-data-modelling/02-union-types.md).

## Why

Reflection conflicts with four core priorities at once:

- **Embedding and safety.** Tel's security model is
  [capability-based](../16-ffi-and-interop/04-embedding-tel-in-a-host.md): a
  script can only touch what the host handed it, and that is checked
  statically. Reflection — `eval`, call-by-string, dynamic loading — punches a
  hole straight through that model: it lets a script reach code and behaviour
  the type checker never saw. A capability gate you can bypass is not a gate.
- **Ahead-of-time compilation.** Tel must compile AOT to many targets. Runtime
  type introspection and `eval` force a full type system (and often a
  compiler) into the runtime, defeating AOT and bloating every host
  implementation.
- **Stability.** Reflection makes implementation details — concrete types,
  field names, private structure — observable, so changing them silently
  breaks callers. Tel wants the opposite: only declared, explicit API is
  depended upon.
- **Tooling and refactoring.** A name reached via reflection is invisible to
  rename, find-usages, and dead-code detection. The Java observation applies:
  the absence of macros and runtime metaprogramming is what
  made Java's IDE refactoring story uniquely strong; the same logic applies
  to reflection. Tel banks on this.

### Reflection-with-generics is its own quagmire

Even where reflection seems harmless ("just walk the fields of a struct"),
it interacts badly with generics. The classic Java example: a field of type
`Map[T, Number]` reflects, at runtime, as a `Map[T, Number]` where `T` is a
*type variable*, not a concrete type — the runtime has erased what `T` was
bound to. C++ erases less; Rust monomorphises and erases differently again.
Every language reaches a different compromise, and every compromise leaks
into user code that uses reflection.

Tel skips the entire mess. A script that needs to *walk* a data model walks
it through generated code with concrete types baked in, not through a
runtime view of what the value claims to be.

## What to use instead

The replacement for reflection is **a stronger static type system plus
schema-driven code generation**:

- Need to walk a data model generically (serialise it, build a form for it,
  diff it)? Generate that code from a **schema** as a separate module — see
  [Derive and Attributes](03-derive-and-attributes.md) and the schema-first
  serialisation note in [Macros](01-macros.md). The machinery lives in
  [`std.tel_ast`](../17-standard-library/18-tel-as-data.md), which exposes a
  typed AST and a printer; a generator builds a `Module`, writes it as a
  `.tel` file, and the next compile picks it up. The generated code is
  ordinary, statically-typed, IDE-navigable Tel.
- Need to recover a specific case from a general value? Use a **sealed union**
  and exhaustive `match`, not a runtime type query.
- Need behaviour to vary by type? Use **traits**, resolved statically.

The key inversion: reflection asks the *runtime* "what is this value?" and
gets a partial, type-erased answer. Codegen asks the *schema* "what types
exist?" at build time and gets a complete, statically-typed answer compiled
into the program. Almost every reflection use case in mainstream languages —
ORM mapping, form rendering, validators, diff/equality walkers — translates
straightforwardly into the codegen idiom.

## A noted cost

Reflection-free languages pay a real price: some boilerplate is hard to avoid
without it. A concrete example — generating a fully pre-filled,
overridable form from a large input data model is tedious to wire by hand.
Other recurring asks that lean on reflection in mainstream languages:

- **ORMs and API clients.** "How to make code generation for things like ORMs
  and APIs easy?" Tel's answer is schema-first codegen: the schema is the
  source of truth, ordinary Tel modules are generated from it, and user code
  imports those modules. The generated code is editable in the sense that
  *user-written* code wraps or extends it; the generated file itself is
  regenerated and should not be hand-edited.
- **Generic form builders / diff viewers / validators.** Same answer:
  generate per-type code from the schema, do not introspect at runtime.

TODO(open): the boilerplate cost of having no reflection is acknowledged but
not fully answered. The intended answer is schema-driven codegen plus
[`derive`](03-derive-and-attributes.md); whether that covers cases like
"auto-generate an editable form for a big data model" needs validation against
real use cases before Tel1 freezes. Do not reintroduce reflection to solve it.

## See also

- [Macros](01-macros.md) — the broader metaprogramming stance.
- [Derive and Attributes](03-derive-and-attributes.md) — the supported way to
  cut boilerplate.
- [`std.tel_ast`](../17-standard-library/18-tel-as-data.md) — the typed AST
  and printer code generators use in place of reflection.
- [Antifeatures](../02-philosophy/04-antifeatures.md) — the formal exclusion of
  reflection and `eval`.
