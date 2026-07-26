# What Replaces Macros: Use Cases

<!-- TODO: review -->

[Macros](01-macros.md) sets the headline position: **Tel ships no macros at
all** — not author-written, not in the standard library. This page is the
companion catalogue. It walks the popular real-world uses of Rust macros,
Java/C#/Python annotations and decorators, Go struct tags, and the like, and
for each one shows *what it becomes in Tel* — almost always something that is
not a macro.

The point is to pressure-test the no-macros claim against a broad list of
concrete needs and confirm each lands somewhere sensible. The design rule that
decides where, taken from the [priorities](../02-philosophy/01-priorities.md):
build a thing into the language only if **~90% of users would want it and it is
clearly, permanently right**. Everything open-ended goes to codegen; everything
that is really "dispatch on a type" goes to generics; everything that is really
"a property of a value" goes to the type system.

## The toolbox

Every supported case below resolves to one of these mechanisms. None is a macro.

| Mechanism | What it is | Where it lives |
|-----------|------------|----------------|
| **Builtin** | Compiler builtin — looks like syntax or a normal call, no expansion phase | compiler |
| **Derive** | Fixed, language-defined `derive` set against a type's shape | compiler |
| **Types** | Refined types / newtypes / `Option` — make the property a *type*, not an annotation | type system |
| **Generics** | Parametric generics + traits with bounds and default methods | type system |
| **Closures** | A normal function taking a trailing closure; reads like a control structure | library code |
| **Comptime** | Compile-time evaluation of pure, capability-free functions | compiler |
| **Codegen** | Ordinary Tel over [`std.tel_ast`](../17-standard-library/18-tel-as-data.md) that writes a `.tel` file the next build reads | tooling / crate |

A handful of needs are real but **declined** — Tel does them differently, with
no dedicated feature at all. Those are collected at the [end](#declined-done-differently).

## Builtin — what other languages ship as a macro, Tel builds in

Because the compiler implements these directly, they are *by definition not
macros*: no macro language, no user-visible expansion, nothing for the IDE to
see through (see [Macros](01-macros.md#what-tel-allows-no-macros-at-all)).

- **String formatting / `println`** *(Rust `format!`/`println!`; Python
  f-strings; Java `String.format`).* Interpolation and formatting are a builtin:
  format-string parsing, argument-count and type checking, and width/precision
  specifiers handled by the compiler. `Log` and friends still arrive as
  [capabilities](../02-philosophy/04-antifeatures.md#the-host-boundary) — there
  is no ambient `print`.
- **Collection / literal constructors** *(Rust `vec![]`, `hashmap!{}`; Clojure
  literals).* `[1, 2, 3]` and map/set literal forms are language syntax, not
  macros. Comprehension-style building is ordinary iterator code
  (`range(0, 100).filter(...)`).
- **`assert` / `assert_eq`** *(Rust `assert!`; Java `assert`; C `assert.h`).*
  A builtin, because the valuable part — capturing the **source text** of the
  asserted expression and the **call-site location** for the failure message —
  is exactly the compiler magic a builtin can do and a plain function cannot.
- **Test and benchmark markers** *(Rust `#[test]`/`#[bench]`; JUnit `@Test`).*
  A small, fixed, language-defined **attribute** set the runner and tooling
  recognise. Parameterised cases are an ordinary loop over inputs against a
  plain test function — no per-framework macro DSL. See
  [Testing](../14-testing/01-testing.md). The "do not optimise away" marker
  (`black_box`) is the same shape — an attribute if kept — and is tracked in
  [Compile-Time Evaluation](04-compile-time-evaluation.md).
- **Deprecation markers** *(Rust `#[deprecated]`; Java `@Deprecated`).*
  A fixed attribute drives a compiler warning. The *migration* half — "rewrite
  this old call to its replacement" — is the
  [IDE suggestion patterns](03-derive-and-attributes.md#ide-suggestion-patterns)
  (structured AST matching, `filter(p) → keep(p)`), a tooling feature, not codegen.

## Derive — structural implementations against a type's shape

A **fixed, language-defined** derive set, generating field-by-field
implementations from a type's declared shape. Always opt-in — never
auto-derived — because a type with identity, a cache field, or a normalisation
rule has a *wrong* structural default. Full detail in
[Derive and Attributes](03-derive-and-attributes.md).

- **Equality, hashing, ordering, debug-printing** *(Rust
  `#[derive(PartialEq, Eq, Hash, Ord, Debug)]`; Java `record`/Lombok; Kotlin
  `data class`).* The canonical derives — `Eq`, `Hash`, `Debug` settled, `Ord`
  plausible.
- **Closed-union (enum) reflection** *(Java enum `values()`/`valueOf`; Rust
  `strum`).* Because Tel [unions](../10-data-modelling/02-union-types.md) are
  closed and the type is the tag, the compiler already knows every variant. A
  built-in `derive(Enum)` gives a static `.values()` list and name↔case
  conversion — no runtime reflection, no `strum`-style proc-macro. Open-ended
  display formatting stays the author's job.

## Types — make the property a type, not an annotation

When the thing being annotated is really *a property of a value*, Tel makes it a
**type**, checked once at construction and guaranteed everywhere after.

- **Validation constraints** *(Java Bean Validation `@NotNull`/`@Size`/`@Email`;
  pydantic validators).* A [refined type](../05-types/12-refined-types.md)
  carries the predicate:
  `type Probability = Real64 where 0.0 <= self and self <= 1.0`,
  `type NonEmptyText = Text where len(self) > 0`. No per-parameter annotation,
  no validator framework.
- **Null-safety annotations** *(Java `@Nonnull`/`@Nullable`; JSR-305).* Subsumed
  by the type system: there is **no `null`**, optionality is an `Option`-shaped
  type, and a non-null wrapper is just a type.
- **Lazy / one-time initialisation** *(Rust `lazy_static!`/`once_cell`; Kotlin
  `by lazy`).* A stdlib type, not a macro: a settable `Once[T]` cell covers
  resolve-later / first-writer-wins. For pure constants prefer comptime (below).
- **Bitflags / flag sets** *(Rust `bitflags!`; Java `EnumSet`).* A
  `Set[Variant]` over a closed union, or a small flags type — ordinary generic
  library code that combines with the compiler-known variant list from
  `derive(Enum)`.

## Generics — anything that dispatches on a type

The chapter is firm: **anything that dispatches on a type is generics with
trait bounds**, not a typed macro. A trait with **default methods** gives
"free" behaviour to any implementor without generating per-type code.

- **Custom trait-impl derives** *(Rust user `derive` proc-macros, e.g.
  `#[derive(Parser)]`).* If a use case seems to need a custom derive, the fix is
  to *generalise generics/traits*, not add a macro. Genuinely data-driven impls
  (clap-style arg parsing from a struct) go to codegen.
- **Operator overloading** *(Rust `impl Add`; C++ `operator+`).* Implementing a
  language trait (`Add`, `Eq`, …) gives a type operator behaviour — a normal
  trait impl. But the **operator set and precedence are fixed**; user-defined
  operators are
  [rejected](../02-philosophy/04-antifeatures.md#surface-and-semantics).
- **async / await / generators** *(Rust/C# `async`; `yield` transforms).* Tel
  has **no function colouring and no `Future`/`Promise` type**: a spawned
  [task handle](../14-concurrency-and-parallelism/02-tasks.md) is the awaitable
  and `join` is the await. Iteration goes through the iterator trait, not a
  `yield` macro. See
  [async and function colouring](../14-concurrency-and-parallelism/03-async-and-function-colouring.md).

## Closures — "do something around a block"

A whole class of "wrap the body" annotations becomes a normal function taking a
**trailing closure** that reads like a built-in control structure. The
user-visible feature Tel needs is *good closures* — trailing-closure syntax,
non-local `return`/`break`/`continue`, inherited receivers — not macros that
look like closures.

- **Context / structured logging, scoped instrumentation** *(slf4j MDC; Rust
  `tracing::instrument`/`info_span!`; OpenTelemetry spans).* The motivating
  case:

  ```tel
  log.with_context(USE_NEW_THING) {
      # body runs with the context set, removed afterwards
  }
  ```

- **Transactions / retry / AOP "around" advice** *(Spring
  `@Transactional`/`@Retryable`; aspect weaving).* The same shape:
  `with_transaction { ... }`, `retry(3) { ... }`. Tel deliberately has **no
  aspect weaving** — hidden behaviour injected at compile time is exactly the
  IDE-opaque, reviewer-hostile property the chapter rejects. The wrapper is
  visible, ordinary code.
- **Runtime memoisation** *(Python `@lru_cache`).* An ordinary higher-order
  function wrapping a closure with a cache — no decorator syntax. (A *pure*
  table computed once is comptime instead.)
- **Embedded "DSL" builders** *(Kotlin type-safe builders; `html!`).* Builder
  APIs with trailing closures give the "looks like a DSL" feel
  (`html { div { ... } }` as nested closure calls). No macro-defined control
  flow — every library inventing its own `if`-shaped construct is rejected.

## Comptime — pure code folded at build time

Pure, capability-free functions applied to constants fold at compile time and
cache. Tel leans toward this being an **invisible optimisation** rather than a
Zig-style `comptime` keyword (every top-level binding is already `const`, and
capabilities make purity checkable). See
[Compile-Time Evaluation](04-compile-time-evaluation.md).

- **Compile-time constants and lookup tables** *(Rust `const fn`/`build.rs`;
  C++ `constexpr`; Zig `comptime`).* `const LOOKUP = build_table(256)` folds
  because `build_table` is capability-free.
- **Pure memoised tables** — the build-time half of the memoisation case above.

## Codegen — open-ended, schema-driven generation

Anything domain-specific and open-ended is ordinary Tel over
[`std.tel_ast`](../17-standard-library/18-tel-as-data.md) that writes a `.tel`
file the next compile reads. Not a macro: no expansion phase, no call-site
magic. One reviewable generator, one readable generated file — see
[Macros, Alternative 1b](01-macros.md#alternative-1b-script-authored-codegen-via-stdtel_ast).

- **Serialisation / deserialisation** *(Rust `serde`; Jackson `@JsonProperty`;
  Go struct tags; pydantic).* **Not** a derive and **not** built in. Tel data
  models are **schema-first**: a declarative schema drives a generated data
  module *and* its (de)serialiser. This avoids serde's timing bug — derive
  macros run before type info exists, so serde cannot emit a JSON schema from
  the same declaration that drives serialisation. See
  [TIP-0007](../tips/0007-serialisation-data-model-and-formats.md). `TODO(open):`
  a structural `Serialize`/`Deserialize` derive is proposed as the bridge for the
  pure-structural case — confirm against the schema-first model.
- **ORM / typed DB access** *(Hibernate/JPA `@Entity`; Diesel; sqlx `query!`).*
  A generator reads the DB schema and emits typed accessors, row records, and
  prepared-statement wrappers. Compile-time-checked SQL is the same story: the
  schema is a registered build input, so incremental builds re-run when it
  changes.
- **Reflective field mappers** *(Java reflection + MapStruct `@Mapper`; bean
  copying).* Tel rejects runtime [reflection](02-reflection.md). A mapper
  between two data shapes takes inputs beyond a single type (the *other* shape +
  rules), so it is not a derive — it is a generator emitting a typed conversion
  function.
- **DSL-as-source** *(SQL/grammar files compiled to code).* When a DSL is really
  a separate source language, compile it to `.tel`. Tel building Tel is itself a
  `std.tel_ast` program.

## Declined — done differently

These needs are real, but Tel adds no feature for them; idiomatic Tel covers the
job. All are formalised in the
[antifeatures](../02-philosophy/04-antifeatures.md#inheritance-and-dynamism).

- **Dependency injection** *(Dagger; Spring `@Autowired`; Guice `@Inject`).*
  Declared too complex, and unnecessary: Tel's
  [capability model](../02-philosophy/04-antifeatures.md#the-host-boundary)
  already does DI's core job. Dependencies (`Clock`, `Log`, a repository) are
  **explicit parameters** threaded from the host; wiring is ordinary constructor
  code, not a graph a framework synthesises from annotations. No container, no
  classpath scanning. This is the
  [*No dependency-injection framework or implicit wiring*](../02-philosophy/04-antifeatures.md#inheritance-and-dynamism)
  antifeature.
- **Web routing / endpoint declaration** *(Spring `@GetMapping`; Rocket
  `#[get]`; axum attribute routing).* Routes are ordinary data — a value holding
  method + path + handler, registered with a router by a normal call. The
  "static route tree with compile-time path validation" ask wants compile-time
  eval or dependent types and loses more than it wins; let the host do a
  startup-time completeness check. Same antifeature as DI.
- **Plugin / service registration / auto-collection** *(Java `ServiceLoader` +
  `@AutoService`; Rust `inventory`; pytest plugin discovery).* "Collect every
  type annotated `@X` across the program" violates the
  [predictable name footprint](01-macros.md) requirement — importing one name
  must not force scanning every unit. Registration is **explicit** (a list the
  author maintains, or each module calling `register(...)`), or a codegen step
  that scans a *declared* set of inputs. Same antifeature as DI.
- **Conditional compilation / feature flags** *(Rust `#[cfg]`; C `#ifdef`).*
  No in-file source forking — Tel's
  [one-script-many-hosts](../01-overview/02-when-to-use-tel.md) guarantee wants a
  script valid on every host. Real platform variation lives in
  **platform-conditional crates / stdlib APIs** a host may omit — the same
  model as the platform-conditional `Sync` primitives
  ([concurrency utilities](../17-standard-library/12-concurrency-utilities.md#shared-mutable-types-platform-conditional))
  — plus ordinary runtime config for toggles.
- **Builder generation** *(Lombok `@Builder`; `derive_builder`).* Mostly
  evaporates: Tel has
  [named + default arguments](../09-functions/04-default-and-named-arguments.md),
  so `Person(name = ..., age = 30)` is already the builder for the common case.
  `TODO(open):` reserve a builder derive only for the staged/partial-construction
  case where named args don't suffice — confirm whether it makes Tel1.

## See also

- [Macros](01-macros.md) — the no-macros position and the parked leaf-code shape.
- [Reflection](02-reflection.md) — why runtime introspection and `eval` are out.
- [Derive and Attributes](03-derive-and-attributes.md) — the fixed derive and
  attribute set in detail.
- [Compile-Time Evaluation](04-compile-time-evaluation.md) — pure code at compile time.
- [`std.tel_ast`](../17-standard-library/18-tel-as-data.md) — the typed AST that
  hosts codegen in place of macros.
- [Antifeatures](../02-philosophy/04-antifeatures.md) — the formal exclusions.
