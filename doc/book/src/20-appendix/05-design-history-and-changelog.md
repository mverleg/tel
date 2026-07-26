# Design History and Changelog

<!-- TODO: review -->

This appendix records *how* the design got here: the names the language has
worn, the directions that were tried and discarded, and the larger pivots
visible in the design's evolution. It is not normative — the chapters say what
Tel *is*; this page exists so that anyone tempted to re-add a recurring older
idea can see why it is *not* in the chapters.

## Naming history

The language has been called several things over the years; only the most
recent name is authoritative.

- **Mango (MGPL).** The original working name, used through the bulk of the
  early design. At various points the name was nearly abandoned because of
  collisions with other projects (`mango_languages.com`, a long-dead Savannah
  project), with *Mango code* floated as a softer alternative.
- **Steel.** A short-lived rebrand from an intermediate stage.
- **Tel.** The current name, short for *Typed Embedded Language*. Picked
  alongside the pivot to embedding (see [The embedding pivot](#the-embedding-pivot)
  below). Every Mango / Steel reference is treated as Tel; the old names are
  not preserved in the chapters.

The numbered working title is **Tel1**. The intent is that breaking changes
are not made *to* Tel — a successor would be a separate language called Tel2.
See [`../02-philosophy/01-priorities.md`](../02-philosophy/01-priorities.md)
for the stability commitment.

## The embedding pivot

The single largest shift in the design's history is the move from "general-
purpose language with a standard library, runtime, threads, package manager,
build tool, web framework opinions, ORM, …" to "**embedded** guest language
inside a host program." Most of the earliest design material predates this
pivot.

The shift cascades through many smaller decisions:

- **I/O becomes a capability**, not an ambient power. Pre-pivot, the design
  freely sketched `print`, filesystem walks, HTTP clients, and database
  connections as if `std` could just have them. Post-pivot, anything that
  touches the world is passed in by the host.
- **No first-class async/await runtime.** Earlier explorations covered async,
  fibers, green threads, event loops, and stackful vs stackless coroutines at
  length. Post-pivot, concurrency is *tasks* and the host decides what running
  a task means; the language does not bake a runtime in.
- **No microcontroller story.** Early thinking occasionally drifted into
  `no_std`, embedded *systems*, or hand-tuned memory layouts.
  "Embedded" in Tel means *guest in a program* — see
  [`../01-overview/01-introduction.md`](../01-overview/01-introduction.md).
- **A standard library kept opinionated and self-consistent**, rather than a
  community-grown ecosystem competing with itself. Many of the early
  "ecosystem" ideas (compile farms, namespacing schemes, CDN-delivered
  crates, GitHub as a backing store) belong to a standalone-language world
  and are dropped or punted to the host.

## Old ideas explicitly rejected

Each of these was considered seriously and is *not* in Tel; the matching
alternative is the current design.

### Surface and semantics

- **Mixfix operators (Agda-style `if_then_else_`).** Considered for
  readability. Rejected: hard to parse, hard for an IDE, conflicts with
  *familiarity over a "better" but novel surface*. Tel uses fixed
  precedence and associativity — see
  [`../04-syntax/04-precedence-and-associativity.md`](../04-syntax/04-precedence-and-associativity.md).
- **Custom operators and per-file precedence overrides** (Haskell / Scala
  style). Rejected for the same reason — code that reads differently in
  different files defeats *readability over writability*.
- **Two-token expressions like `A B`** — both as a *multiplication* shorthand
  and as **function application** by juxtaposition (`min a b c` for
  `min(a, b, c)`). The juxtaposition form was floated as a tentative
  convenience and then rejected: it is too easy to break the parser and too
  easy to confuse a reader. Function calls always use parentheses — see
  [`../02-philosophy/04-antifeatures.md`](../02-philosophy/04-antifeatures.md).
- **Implicit `return` of the last expression in a function body.** Initially
  rejected in favour of an explicit `return`. **Reversed:** implicit return of
  a block's final expression is now supported (in block, function, and guard
  position), with `return` reserved for early exit — see
  [`../09-functions/03-return-values.md`](../09-functions/03-return-values.md).
- **Truthy / falsy coercion of non-`Bool` values** (Python / JS style).
  Rejected — see *no implicit conversions or DWIM* in
  [`../02-philosophy/04-antifeatures.md`](../02-philosophy/04-antifeatures.md).
- **Tilde-prefix `~q` to mean "this `Option` becomes its default" and the
  paired auto-`Into` coercions.** Considered as syntactic sugar. Rejected:
  hidden conversions are exactly the failure mode Tel is built to avoid.
- **Ruby-style trailing modifiers (`do_it() if x`).** Rejected: prefer
  one form of `if`, even at the cost of brevity.
- **Custom unary postfix operators in user identifiers** (Ruby-style trailing
  `!` / `?` in method names — `empty?`, `save!`). Rejected: it collides with
  the propagation / abort operator family reserved for `?` and `!!`,
  and a fixed sigil per operation is harder to grep than a normal name. A
  predicate is `is_empty`; a fail-loudly accessor is a method whose contract
  says it aborts.
- **Mixfix per-identifier postfix tags for booleans** (Ruby's `attr?`).
  Rejected for the same reason: confusing for non-Ruby readers and indistinct
  from the propagation operator.
- **Tilde (`~`) for backward indexing** (Python-3.12-style "from the end").
  Rejected: Tel has `Option`-returning lookups and explicit `len - 1`; an
  extra sigil pays little for the readability cost.
- **`a = b = c` chained assignment.** Rejected; the rule and its move/ownership
  rationale live with assignment semantics in
  [Mutability](../06-bindings-and-scope/02-mutability.md).
- **The `===` triple-equals operator** (Ruby/JS — "fits inside" or
  "strict equal"). Rejected: Tel's `==` is structural and the type system
  already rules out the comparisons `===` was invented to discipline.

### Type system

- **Heavy dependent types (Coq / Agda style).** Considered for expressing
  matrix-dimension safety, value-level invariants, etc. Rejected: undecidable
  inference, prohibitive complexity for a language aimed at small embedded
  scripts. Refined types ([`../05-types/12-refined-types.md`](../05-types/12-refined-types.md))
  and design-by-contract ([`../02-philosophy/03-features.md`](../02-philosophy/03-features.md))
  cover the realistic needs.
- **Structural typing (Go style).** Considered. Rejected: marker behaviours
  like `Send` and `Copy` cannot be expressed structurally, the orphan-like
  failure modes are subtler, and *familiarity* favours nominal typing.
- **Inheritance, mixins, and abstract base classes.** Considered repeatedly.
  Rejected — see
  [`../02-philosophy/04-antifeatures.md`](../02-philosophy/04-antifeatures.md).
  The replacement is traits with delegation; this is the single most
  *consistent* direction the design has held to throughout.
- **Sigil-distinguished concrete vs trait types** (the earlier `#Fruit` vs
  `Fruit` proposal). Rejected: too much surface novelty for what is already
  expressible at signatures. The concrete-vs-abstract distinction is carried by
  a single keyword instead — a bare trait name (`Fruit`) means "some concrete
  type implementing `Fruit`," statically dispatched, and `dyn Fruit` is an
  explicit, type-erased trait object. A `dyn Fruit` also satisfies a bare
  `Fruit` parameter, and there is no syntax to *force* a concrete
  representation, because the only thing concreteness buys is performance and
  that is the caller's choice — see
  [`../10-data-modelling/03-traits-or-interfaces.md`](../10-data-modelling/03-traits-or-interfaces.md).
- **The `impl Trait` spelling for static dispatch** (Rust-style "some type
  implementing this trait" in argument and return position). Dropped: a bare
  trait name in a value position already carries that meaning, so the keyword
  was redundant. Only the *dynamic* form, `dyn Trait`, is spelled out; static
  dispatch is the unmarked default. The cost — `x: Fruit` reads the same whether
  `Fruit` is a struct or a trait — is accepted, for the same reason `#Fruit` was
  rejected: concreteness is a performance detail, not a contract. This concerns
  only the *type-position* `impl Trait`; the `impl Trait for Type`
  *implementation* block is a separate construct and is unaffected. See
  [`../10-data-modelling/03-traits-or-interfaces.md`](../10-data-modelling/03-traits-or-interfaces.md).
- **A user-visible heap / `Box` type** (Rust-style explicit boxing). Rejected:
  stack-vs-heap placement is never part of Tel's surface — the compiler decides
  placement *and* inserts any indirection that unsized, recursive, or too-large
  values need. What stays visible is value-vs-reference *semantics*, carried by
  a type's mutability kind (`T` / `!T` / stdlib shared-mutable), not by where
  bytes live — see
  [`../12-memory-and-runtime/02-stack-and-heap.md`](../12-memory-and-runtime/02-stack-and-heap.md).
- **Aspect / mixin style instance upgrades** (`Claim & WithCustomers`).
  Considered for the ORM use case. The current direction prefers
  *projections* — distinct types for distinct loaded shapes — see
  [`../19-use-cases/09-entity-identity-and-projections.md`](../19-use-cases/09-entity-identity-and-projections.md).
- **Multimethods / multiple dispatch.** Considered. Rejected: trait dispatch
  plus explicit `match` covers the realistic cases without the dispatch-
  ordering complexity.
- **Function overloading by argument type.** Considered repeatedly. Rejected:
  it hurts type inference (the function picks the type instead of the type
  picking the function), it explodes combinatorially with optional and union
  parameters, and the *call site* becomes ambiguous about which body runs.
  The replacement is one function with [union parameter types](../05-types/05-function-types.md),
  [default and named arguments](../09-functions/04-default-and-named-arguments.md),
  and [trait-based dispatch](../09-functions/09-overloading-and-dispatch.md)
  where genuinely different bodies are needed.
- **Heterogeneous lists statically typed (HList).** Considered. Rejected: too
  exotic for a familiarity-first language; the realistic use case is a
  [tuple](../05-types/04-tuples-and-arrays.md) for short cases and an
  ordinary list-of-union-type for longer ones.
- **Type-classes-as-objects with separate-implementation lookup**
  (Haskell-style `Eq Int` value-level dictionary, or Rust's
  `MyHashTable[i8: my_mod::MyHash[i8]]` proposal). Considered as an
  alternative to orphan rules. Rejected: the surface complexity is
  enormous, and the orphan rule plus crate-scoped trait implementations
  cover the realistic conflicts.
- **Trait coherence settled (TIP-0005).** The lean above is now spelled out:
  adopt the orphan rule (own the trait *or* the type), extend it to generic
  impls via a covering rule, **allow same-owner specialisation**
  (most-specific-wins over a co-located, import-independent candidate set) while
  rejecting cross-crate specialisation. `Eq`/`Hash`/`Ord` need no special rule —
  the orphan rule plus conflict-rejection already keep them constant per type (an
  earlier single-owner refinement, "O6", was dropped as redundant). No runtime
  conformance registry. This narrows the earlier blanket "no specialisation"
  stance. See
  [`../tips/0005-trait-coherence-and-the-orphan-rule.md`](../tips/0005-trait-coherence-and-the-orphan-rule.md).
### Concurrency and control flow

- **Exceptions and stack unwinding.** Considered many times,
  including checked exceptions. Rejected — errors are values; see
  [`../13-error-handling/01-philosophy.md`](../13-error-handling/01-philosophy.md).
- **Function colouring (`async` keyword in the signature).** Considered.
  Rejected — see
  [`../14-concurrency-and-parallelism/03-async-and-function-colouring.md`](../14-concurrency-and-parallelism/03-async-and-function-colouring.md).
- **`go fn(...)` style fire-and-forget tasks** (Goroutine syntax).
  Considered. Rejected because an unobserved task failure is a silent bug —
  tasks are scoped, see
  [`../14-concurrency-and-parallelism/04-structured-concurrency.md`](../14-concurrency-and-parallelism/04-structured-concurrency.md).
- **Ruby-style lambdas that can `return` from the enclosing function.**
  Control flow that crosses an opaque function boundary works against *if it
  looks correct, it probably is correct*. Still open, because the same
  feature is genuinely useful for DSLs (early-out inside a block passed to a
  builder reads naturally). `TODO(open): non-local return from a lambda. Pro:
  excellent for DSLs — a block can bail out of the surrounding function like
  a built-in control structure. Con: a call that looks like an ordinary
  function can divert control past its caller, which is exactly the kind of
  hidden control flow Tel avoids. Lean: undecided; if added, restrict to
  lambdas that are syntactically inline at the call site.`
- **`defer` keyword (Go).** **Rejected.** Relevant (must-use) types are
  superior: a value whose type says "must be consumed", combined with
  `AutoClose`, guarantees cleanup at the type level rather than relying on the
  author to remember a `defer`. See
  [`../12-memory-and-runtime/08-substructural-types.md`](../12-memory-and-runtime/08-substructural-types.md).
- **Stackful coroutines / fibers as a language primitive** (Go-style green
  threads with their own stacks). Considered. Rejected: every host has to
  reproduce the stack-switching machinery exactly, and the per-stack memory
  overhead is at odds with embedding into hosts that already have their own
  thread model. Tasks are a *user-visible* abstraction; the host chooses
  whether to implement them as stackless state machines, fibers, OS threads,
  or sequential continuations — see
  [`../14-concurrency-and-parallelism/03-async-and-function-colouring.md`](../14-concurrency-and-parallelism/03-async-and-function-colouring.md).
- **GIL-style global interpreter lock.** Considered as a fallback for hosts
  whose runtimes are not thread-safe. Rejected as a language-level
  promise: per-fiber heap isolation removes the *reason* the GIL exists in
  Python and JS, and a host that genuinely needs single-threaded execution
  can simply schedule all tasks on one OS thread without telling Tel.
- **Continuation-passing style (CPS) as a user-facing form.** Considered for
  expressing async and event-handling code. Rejected: CPS is something a
  *compiler* may use internally if it helps codegen, but exposing it would
  drag in the readability problems async-style callbacks have, which is
  exactly what task-based concurrency replaces.

### Tooling and ecosystem

- **A compile farm / cloud build service** as part of the language story.
  Rejected for embedding: the host already drives compilation, often at
  load time.
- **Bundled web framework, ORM, or REST client as part of the language.**
  Rejected; these are library territory. The host owns the world. See
  [`../02-philosophy/04-antifeatures.md`](../02-philosophy/04-antifeatures.md).
- **Multi-language standard library** (English plus Chinese aliases for
  every name). Rejected as a language feature; could be an editor-side
  presentation, but not stored in source.
- **A separate file extension for "script-mode" Tel** with a different,
  looser dialect. The opt-in module/visibility approach
  ([`../11-modules-and-packages/01-modules.md`](../11-modules-and-packages/01-modules.md))
  is intended to make this unnecessary — small scripts stay small without
  switching dialects. There is **no strict mode** and no looser dialect; one
  language, one set of rules. Two related mechanisms cover the real needs
  without forking the dialect: **compile warnings** (which a project may
  choose to treat as errors) and a **debug mode** that turns on extra runtime
  checks such as integer-overflow detection.
- **Per-file language-version pragma** (HTML-style `<!doctype tel-1.4>` at
  the top of every source file, with mixed versions allowed in one project).
  Rejected: Tel is frozen at Tel1, so there are no version dialects to
  switch between within a project. A successor (Tel2) would be a separate
  language with its own toolchain, not a file-local opt-in.
- **GPU / quantum / FPGA sub-dialects baked into the core language.**
  Considered repeatedly (Futhark-style array kernels,
  Halide-style schedules, GPU-only loops). Rejected for the core: these are
  specialised host capabilities, and the host that needs them exposes them
  as a capability — they do not belong in the language itself. A host that
  ships a GPU runtime can pass GPU kernels to Tel as opaque handles.
- **"Compile to JS" or "compile to LLVM" as a language-level promise.**
  Considered (a Tel program that targets the browser
  must compile to JS). Rejected as a language-level guarantee: which
  target a host produces is up to the host. The language constrains what
  is *expressible*, not what binaries fall out.

## Versioned changelog

Tel has no release versions yet — this section is a placeholder for the
day the first published release happens. The intent (see
[`04-versioning-and-compatibility.md`](04-versioning-and-compatibility.md))
is for the changelog to be terse, to record only behaviour-visible changes,
and to never list a breaking change to Tel itself — those would belong to a
Tel2 changelog.

`TODO(open): when the first public version is cut, decide the changelog
format (per-release sections, per-feature one-liners, links to PRs).`
