# Tel Improvement Proposals

This directory holds **TIPs** — design proposals for Tel that are larger than a
chapter edit but not yet ready to be merged into the chapter docs.

A TIP is the right shape when a question is:

- **Cross-cutting** — it touches multiple chapters at once (e.g. mutability
  affects bindings, types, memory, concurrency).
- **Unresolved** — current docs may already lean one way, but a concrete
  alternative is on the table and worth writing out before committing.
- **Worth recording the alternatives for** — even if one option wins, the
  rejected paths should be findable later.

TIPs are **not** part of the chapter docs. The chapter docs describe what Tel
*is*; TIPs describe a proposed change. When a TIP is accepted, its content
moves into the relevant chapters and the TIP is marked *Accepted* (or
*Superseded*) but kept here for history.

## Lifecycle

- `Draft` — under discussion, no commitment.
- `Accepted` — content integrated into chapter docs; TIP kept as historical
  record.
- `Rejected` — explored, not adopted. Keep so the reasoning is recoverable.
- `Superseded` — replaced by a later TIP. Note the successor inline.

## Conventions

- One TIP per file, named `NNNN-short-slug.md` (e.g. `0001-mutability.md`).
- Numbers are allocated in order; do not reuse a number for a rejected TIP.
- Each TIP starts with a `Status` block.
- Use the same style as the chapter docs (markdown, ```tel fences, terse
  prose, `TODO(open):` markers for unresolved sub-questions).

## Index

- [`0001-mutability-and-borrowing.md`](0001-mutability-and-borrowing.md) —
  mutability model, references, lifetimes. **Accepted** and migrated into the
  chapter docs (2026-06-13); kept as the historical record.
- [`0002-untagged-unions-and-sealed-traits.md`](0002-untagged-unions-and-sealed-traits.md)
  — **unify onto unions**: the union is the one closed-set construct, traits are
  bounds only, "sealed" is dropped as a trait kind, and "closed set with a
  contract" becomes a union with a member bound `(A | B) : Trait`. Also: structural
  alias identity, `newtype` for a distinct family, and mandatory `(A | B)` parens.
  (Reverses the earlier keep-both recommendation.) **Accepted** and migrated into
  the chapter docs (2026-06-16); kept as the historical record.
- [`0003-module-levels-and-dependency-direction.md`](0003-module-levels-and-dependency-direction.md)
  — **three levels** named **module / crate / workspace** (sub-project dropped;
  the distributable is the *crate*, not "package"), namespaces as a separate
  axis (crate wins over workspace), crates have no parents (dotted lexical names
  forming a prefix-antichain per flat namespace, workspace decides the mapping),
  a **two-rule api/impl/executable flag** (`api↛impl`; nothing imports an
  `executable`), three-level visibility (`private`; `export { … }` block, no
  `from`, that defines a public API tree **decoupled from the code layout** so
  refactors keep backwards compat), dev-only workspace members, and
  major-version backwards-compat enforcement (pre/postconditions deliberately
  *not* enforced). **Accepted** and migrated into the chapter docs
  (2026-06-17); kept as the historical record. (Revised after the
  2026-06-15/16 reviews.)
- [`0004-how-far-refinement-types-go.md`](0004-how-far-refinement-types-go.md)
  — the refinement-type spectrum, what other languages settled on, and the
  proposed Tel1 tier line (cheap rungs in, SMT/dependent out).
- [`0005-trait-coherence-and-the-orphan-rule.md`](0005-trait-coherence-and-the-orphan-rule.md)
  — the diamond/conflicting-impl problem; orphan rule (incl. the covering rule for
  generic impls) vs local overrides, named instances, and specialisation;
  same-owner specialisation kept while cross-crate is rejected; and the move of
  giving `Eq`/`Hash`/`Ord` a single owner so the diamond can't corrupt data.
- [`0006-tuples-as-argument-bundles.md`](0006-tuples-as-argument-bundles.md) —
  leaning into "a call's arg list is a tuple": splat application (`f(...b)`),
  composition by feeding return tuples as argument bundles, and prefix-based
  partial application; and how to keep every tuple↔arglist bridge explicit so Tel
  avoids the Swift "tuples-as-arg-lists" regret. **Accepted in part** and migrated
  (2026-06-16): monomorphic `...b` splat and tuple-return composition shipped;
  partial application and the polymorphic (row-polymorphism) forms **deferred**.
- [`0007-serialisation-data-model-and-formats.md`](0007-serialisation-data-model-and-formats.md)
  — keep serde's good half (a built-in data model + `Serialize`/`Deserialize`
  bridge traits) and drop its bad half (the derive proc-macro): formats are
  library-implemented over one `Format` trait, and per-type mappings come from a
  parameterless `derive` or schema-first codegen, never a macro.
- [`0008-named-axis-dataframes.md`](0008-named-axis-dataframes.md) — a
  pandas-style named-axis **dataframe** (heterogeneous per-column types, an
  auto-derived row type, `filter`/`groupby`/`pivot`) is not a matrix; it is the
  columnar carrier of a closed **record-shape calculus** (`project`, `extend`,
  `merge`, `mapfields`, `partition`) the compiler recognises. Pins the method
  signatures (add-column / summary / aggregate / pivot), the row-level type rules,
  and structural naming of derived schemas; reuse is **monomorphic only** (row
  polymorphism excluded). **Accepted and migrated** (2026-06-19) into the
  [Dataframes](../10a-dataframes/01-overview.md) chapter; kept as the historical
  record. Also removes the matrix chapter's *named axes* bullet.
- [`0009-inline-lambdas-and-non-local-control-flow.md`](0009-inline-lambdas-and-non-local-control-flow.md)
  — how a block argument can `return`/`break`/`continue` out of the *enclosing*
  function (so a user-defined `with_lock`/`log.sub` reads like built-in syntax):
  collects Tel's scattered mentions, surveys the field (Kotlin `inline`, Ruby
  block/lambda, Scala 3 `boundary`, the Java/Swift/Rust "don't", macros), and
  proposes **explicit `inline` on the function, non-escaping block params only,
  never implicit** — rejecting Ruby's two closure kinds and Scala 2's exception
  hack.
- [`0010-lambda-receivers-and-builder-dsls.md`](0010-lambda-receivers-and-builder-dsls.md)
  — an implicit *receiver* (context object) for a builder block, so `html { … }`
  reads like markup syntax: surveys the field (Kotlin `T.() -> R`, Groovy
  delegate, Ruby `instance_eval`, Scala `?=>`, the deprecated JS `with`) and
  finds every regret is about *bare-name resolution*. Proposes **the receiver
  supplies context but every use is the explicit leading-`.`** (the implicit-
  `self` rule extended to a block param) — bare names stay lexical, resolution
  stays static, no `@DslMarker`-style patch. Argues receivers are **orthogonal
  to [`0009`](0009-inline-lambdas-and-non-local-control-flow.md)** (receiver =
  context, `inline` = control flow) but co-designed: the full DSL uses both.
- [`0011-resuming-borrows-and-linear-iterators.md`](0011-resuming-borrows-and-linear-iterators.md)
  — a **linear iterator** that cannot be polled after exhaustion (the end leaves
  no iterator in scope), and the sugar that makes it ergonomic: a **resuming
  borrow** — a `&!self` whose reinstatement is conditional on the return variant
  (given back on the producing branch, `consume`d on the terminal one). Author
  writes a borrow-shaped `next` with one `consume` mark; caller writes an
  ordinary `for`/`while let` with no `it = rest` rewire; compiler threads the
  owning move underneath at identical codegen. Generalises `&!`'s suspend-then-
  reinstate to *conditional* reinstatement; contrasts the fused `Option[T]`
  model and the explicit `Option[(T, Self)]` thread. **Draft.**
- [`0012-task-cancellation-abort-and-shutdown.md`](0012-task-cancellation-abort-and-shutdown.md)
  — how tasks stop and how the program stops, unified under one lens: **must-use
  discharge ("discard") must run on every deliberate termination**. Derives the
  spine — **cancellation is a *value* propagated by a normal return, not a hidden
  unwind** — so relevance's existing normal-path check already proves cleanup
  runs on the cancelled path (no unwinder, no shielding). Recommends: cooperative
  **cancellation token** in the stdlib, threaded through blocking primitives
  (Go-`context`-shaped), value-observed at yield points; **no user-callable
  system abort** (violates the lens, is surprise control flow, and a guest can't
  kill its host — involuntary OOM/host/root-panic abort stays as the one
  sanctioned discard-skip); **detached tasks block shutdown** like every task
  (dropping a handle ≠ leaving the tree). Unstoppable work is a **bug**, not a
  force-kill case. **Draft.**
- [`0013-machine-facing-toolchain-surface.md`](0013-machine-facing-toolchain-surface.md)
  — the toolchain already computes resolved APIs, diagnostics, fix-its,
  dependency-graph diffs, and full-step traces, but every one of them is
  specified as something a *person* reads in an *editor*. Records the goal —
  **a program should be able to ask the same questions over a documented,
  parseable interface** — with the bounds that look settled (reader-side only,
  one source of truth, headless, and the embedding constraint: a guest has no
  process tree to attach to, so the offline half is the safe half) and defers
  transport, query set, schema stability, and whether live introspection
  happens at all. **Draft.**
- [`0014-nested-copy-update.md`](0014-nested-copy-update.md)
  — updating a field two levels down forces `with` to nest and the path to be
  written twice, so depth (not breadth) is the whole problem. Proposes a
  package: **`with` chains left-associatively**; **dotted paths on the left of
  `=`** (`context with { user.last_active = now }`) desugaring with **one
  rebuild — and so one invariant check — per level**, restricted to stored
  record fields with visibility checked at every hop; and **`it` as the source
  binding** for relative updates, chosen over field-scoping because that would
  turn every pun into an identity no-op. Keeps an inner `with` in the block as
  the fallback, and rejects bare-`{}` right-hand sides, first-class optics,
  string paths, setter sugar, and Elm's "flatten your records". **Draft.**
