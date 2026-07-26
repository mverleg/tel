# Language Cues for the IDE

<!-- TODO: review -->

The maxim **the IDE is a first-class reader — prefer features it can amplify,
reject features it can't follow**
([`../02-philosophy/02-maxims.md`](../02-philosophy/02-maxims.md)) cuts both
ways. The [Editor Integration](09-editor-integration.md) chapter covers the
*mechanism* — one LSP, sharing the compiler's resolved model. This chapter is
the *catalogue*: the specific Tel features that carry information a plain text
view throws away, and which therefore deserve to be made **discernible** in an
editor.

This book describes only *what should be distinguishable*, never how to paint
it. Whether a property shows up as a colour, a font weight, a gutter mark, an
inlay, or a fold — and which cues are on by default — is the IDE's and the
theme's choice, not the language's. The point here is to name the semantic
facts Tel keeps **syntactically local** — a sigil, a refined type, a
substructural obligation — precisely so the editor can surface them from the
token under the cursor without a definition lookup.

## Cues come from the resolved model, never a heuristic

Every cue below is a projection of something the compiler already knows — a
type, a substructural trait, a dispatch decision. None is a syntactic guess.
This matters for the same reason
[find-references is exhaustive](09-editor-integration.md#coupling-visible-by-design):
there is no `eval`, no reflective dispatch, so a cue the editor surfaces is
*exact*, not "probably." A cue that is sometimes wrong is worse than none,
because the reader learns to distrust it. Tel's rule is that a cue is either
backed by the resolved model or it is not shown.

A corollary: a cue is a **view concern**, never an edit. Folding a type
annotation or marking a `!T` changes nothing in the source — the same property
that lets the formatter be deterministic.

## Ownership and reassignability

The origin case (see [Mutability](../06-bindings-and-scope/02-mutability.md)).
Tel keeps two *orthogonal* axes locally spelled, and a reader benefits from
telling them apart:

- **Ownership (`!T`, affine).** `!T` is the **owned/affine** form; bare `T` is
  shareable. A reader scanning a function wants to see every *owned, move-only*
  value at a glance. This tracks **ownership, not mutation** — a `Mutex`
  mutates but is shareable (`Alias`), so it is not owned; an owned-but-immutable
  resource handle is. Ownership is the more useful thing to surface anyway:
  it's what controls moves, freezing, and what can cross a task boundary.
- **Reassignability (`mut` field, `uniq` binding).** Whether a *slot* is meant
  to change is the separate data-model axis. A `mut` field and a `let uniq`
  binding mark *intended* mutation/rebinding points — distinct from ownership,
  because a reader asks "what can change here?" separately from "what do I own
  here?" In Tel the plain binding is the common case and the rebindable one is
  the exception worth flagging.

A subtle, high-value cue: an **affine `!T` value goes dead** once it is moved
(across a task boundary, into a `&!T` borrow, or `finish()`-ed). The editor can
mark the binding as dead from the move onward — the compiler already rejects a
later use, so the cue just surfaces the error region before the reader writes
into it.

The declaration shapes all keep the `!` sigil constant, so an ownership cue
never hits an ambiguous bare name. A language that let a bare name *sometimes*
mean owned would defeat this; Tel deliberately does not.

## Concrete vs trait, and monomorphisation

A reader needs to know whether a type is **concrete** (this exact type) or a
**trait / type parameter** that will be monomorphised or dynamically
dispatched ([Traits](../10-data-modelling/03-traits-or-interfaces.md),
[Overloading and Dispatch](../09-functions/09-overloading-and-dispatch.md)).
The difference changes how the code behaves and what it costs.

```tel
fn count(animals: List[Animal]) -> PosInt   # Animal is a trait; List[Animal] is heterogeneous
fn count(ducks:   List[Duck])   -> PosInt   # Duck is concrete; monomorphised, homogeneous
```

- The difference between a concrete type and a trait bound or type parameter is
  worth surfacing, so `Animal` (will be dispatched/monomorphised) reads
  differently from `Duck` (fixed). This is the first thing most readers of a
  generic signature want.
- At a call site, the *resolved* instantiation — "`T = Duck` here" — turns a
  generic call back into the concrete one the compiler generated. Because
  dispatch is statically resolved, this is exact.

## Refined types and units

[Refined types](../05-types/12-refined-types.md) are *the* feature that stops
a `ChickenId` reaching a `DrinkMachine`, but only if the editor flags the
mismatch as the reader types it. What's worth surfacing:

- A **live mismatch** on a refinement violation — the same diagnostic the
  compiler emits, at keystroke latency. The value of `ChickenId` vs `DuckId` is
  entirely in this immediacy.
- The **predicate** a refined type carries (`PosInt` → `> 0`), and for
  [units](../05-types/13-units.md) the **dimension** (`kg`, `m/s`), available on
  demand. A reader should not have to open the declaration to see what
  constraint a value satisfies.

Refined types do **not** warrant a distinctive appearance of their own: most
Tel types are refined, so refinement is the norm, not an exception to single
out.

## Substructural obligations: must-use and single-owner

Tel makes every type [linear by default](../12-memory-and-runtime/08-substructural-types.md),
relaxed by the `Alias` and `Discard` capabilities. Each restriction is an
*obligation the reader must discharge*, and so is worth making discernible
before the reader hits the error:

- **Relevant (no `Discard`) → must-use.** A value that must be consumed before
  it leaves scope should be flagged on its binding until a use consumes it. An
  unconsumed relevant value at end of scope is a compile error; the cue shows
  the obligation *before* the reader reaches it. `Discard` types are the quiet
  default; the absence of `Discard` is what's worth flagging.
- **Affine (no `Alias`) → single-owner.** Tie this to the "value goes dead on
  move" cue above — affineness is *why* the move kills the old binding.
- **Linear (neither) → exactly-once.** Both at once: must-use *and*
  dead-on-move. A DB connection that must be `close`d and may have only one
  writer is the canonical case.

A related concurrency cue, already noted in editor-integration: an **unobserved
task failure** under structured concurrency is worth surfacing live
([Structured Concurrency](../14-concurrency-and-parallelism/04-structured-concurrency.md)).

## Inferred types and contracts

These overlap the [density levels](09-editor-integration.md#adjustable-detail--show-less-or-show-more)
already in editor-integration, listed here for completeness:

- Types the compiler **inferred** and the source omits should be available on
  demand — foldable, because for some readers the inferred type is noise and
  for others it's the point.
- **Contracts** ([Refined types](../05-types/12-refined-types.md)) shown inline
  on demand; they *are* the spec for some readers, so they fold but are never
  forced on a reader who doesn't want them.

## What this is not

- **Not a theme.** These are semantic projections of the resolved model, not a
  syntax-highlighter colour scheme. A token's cue depends on what it *resolved
  to*, not what it *looks like*.
- **Not editor-specific.** Like lints, the cues are defined once (over the LSP
  model) and merely *surfaced* per editor — see
  [Editor Integration](09-editor-integration.md#what-editor-integration-does-not-do).
- **Not a writer aid.** The maxim is *the IDE is a first-class reader*. These
  cues help a reader understand existing code; they are not completions or
  generation.

## LSP

The rule is simple: **everything that can be expressed over LSP is, and
everything else is not.** Inlay hints, diagnostics, hovers, and find-references
are standard LSP and ride it directly. Anything that cannot be carried by the
protocol is not a language concern — the editor renders it from the symbol
graph the server already exposes, or it is not shown.

## Open questions

- `TODO(open):` **error-propagation / fallback cue.** Should the points where
  control can leave a function early — `?`-style propagation and the
  [fallback operator](../07-expressions/11-fallback-operator.md) — be surfaced
  so a reader scanning for "where can this bail out" sees them? Some editors do
  this for Rust's `?`; unclear whether it earns its keep in Tel.

## See also

- [Editor Integration](09-editor-integration.md) — the LSP that computes all
  of the above.
- [Maxims](../02-philosophy/02-maxims.md) — *the IDE is a first-class reader*.
- [Mutability](../06-bindings-and-scope/02-mutability.md) — the `!T` / `uniq`
  cues.
- [Substructural Types](../12-memory-and-runtime/08-substructural-types.md) —
  must-use and single-owner.
- [Refined Types](../05-types/12-refined-types.md) and
  [Units](../05-types/13-units.md) — the live-mismatch cue.
</content>
</invoke>
