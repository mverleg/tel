# Tel for AI-Assisted Development

<!-- TODO: review -->

Tel is designed to be read, checked, and edited by tools as much as by people —
and an AI assistant is just another such reader and writer. This page is a
**summary**: it collects the language and tooling choices that make Tel a good
fit for AI-assisted work and links to where each is specified. Nothing here is
unique to this page; the detail lives in the chapters it points to.

The thread running through all of it: **push meaning into places a machine can
see it** — types, contracts, capabilities, names, tests — so that an assistant
reasons from the code itself instead of guessing, and so that a mistake becomes
a compiler error rather than a silent bug.

## Context lives in the code

An assistant works best when the information it needs is *in* the code, not in a
wiki it cannot see. Tel pushes context to where it is checkable and local:

- **Named arguments** make a call site self-documenting: `connect(host, port =
  8080, retries = 3)` says what each value *means* without opening the
  definition. See
  [Default and Named Arguments](../09-functions/04-default-and-named-arguments.md).
- **Named tests** read as a specification — `test fn discount_for_gold_is_ten_percent()`
  states intent in its name, and the body is a worked example of the function in
  use. See [Testing](../14-testing/01-testing.md).
- **Doctests** keep examples *live*: an example in a `##` doc comment is compiled
  and run by the test runner, so it cannot drift from the code it illustrates.
  See [Examples as tests](../20-appendix/06-deferred-features.md#examples-as-tests).
- **Doc comments become teldoc**: a `##` comment
  ([comments](../03-lexical-structure/07-comments.md)) is structured
  documentation the toolchain surfaces, not a discarded aside. Ordinary `#`
  comments stay a last resort — the maxim is *names, types, and asserts carry
  the explanation* ([maxims](../02-philosophy/02-maxims.md)) — but the
  explanation that *is* written travels with the code and into the generated
  docs.
- **Declared example values** give each type concrete instances (and
  counter-examples), so an assistant can see *what a `Mass` looks like* and what
  the type rejects. See
  [declared example values](../17-standard-library/19-testing-utilities.md#declared-example-values-and-counter-examples).

The shared payoff is **local reasoning**
([why](../06-bindings-and-scope/07-no-global-mutable-state.md#why)): the reader —
human or AI — understands a function from its signature, its contracts, its
tests, and its names, without chasing context across the program.

## Meaningful types over bare primitives

A domain modelled as `Int64`, `Text`, and `Real64` puts the whole burden of "which
value goes where" on the writer's memory — exactly where an assistant is
weakest. Tel pushes the other way:

- **Refined types and newtypes** are costless to define and do everything the
  primitive does, so an `AccountId`, a `EurAmt`, or a `Probability` is the
  natural choice rather than a bare scalar. A swapped argument or a mixed-up
  unit becomes a *type error*, not a runtime surprise. See
  [Refined and Newtype Types](../05-types/12-refined-types.md).
- The **linter discourages the bare primitive** from the other side, flagging
  signatures with several same-typed primitives where a wrapper would prevent a
  swap — *primitive obsession* — see
  [built-in pattern lints](../18-tooling/07-linter.md#built-in-pattern-lints-from-the-bug-catalogue).
- **Pre- and post-conditions** ([`requires`/`ensures`](../09-functions/01-function-declaration.md#pre-and-post-conditions))
  and **record invariants** ([records](../10-data-modelling/01-records.md)) carry
  the rules a value must obey *in the type*, so an assistant generating a value
  is told the constraint instead of inferring it.

The effect is that more of the specification is *checked*: the assistant gets
immediate, precise feedback rather than producing plausible-looking code that
only fails later.

## Probable-bug feedback, not just provable errors

The type checker rejects what it can *prove* wrong. Plenty of real mistakes are
not provable — they are only *likely*. Tel surfaces those too, so an assistant
gets a second, advisory channel of feedback:

- The **linter** flags *probable but not provable* bugs as warnings the build
  can proceed past: a `.contains` on a hot list, a `null`-shaped sentinel,
  primitive obsession, a `Result` whose error is silently dropped. See
  [Linter](../18-tooling/07-linter.md).
- **Review invariants** re-surface prose obligations (*"transfer always checks
  authorisation"*) whenever the annotated code changes, giving an assistant the
  exact list of things to re-verify on a diff. See
  [review invariants](../18-tooling/07-linter.md#review-invariants--unprovable-re-checked-on-change).
- **Dependency-graph diffs** show the structural fallout of a change — a new
  dependency, a fresh cycle, a layering violation — beside the diff. See
  [dependency-graph diffs](../11-modules-and-packages/08-dependency-graph-and-locking.md#visualising-the-graph-and-its-diffs).
- A **full-step trace** of one run (`tel trace`) lets a reviewer or assistant
  see what a function *actually did* on an input rather than guess. See
  [tracing a run](../18-tooling/08-debugger.md#tracing-a-run-to-a-log).

The dividing line is deliberate: provable mistakes are *rejected*, probable ones
are *flagged* and left to the human or assistant to judge — the same stance Tel
takes for [provable panics](../13-error-handling/04-panics-and-aborts.md#provable-panics-warn-do-not-reject).

## See also

- [When to Use Tel](02-when-to-use-tel.md) — the broader audience-and-fit story.
- [Features](../02-philosophy/03-features.md) and
  [Antifeatures](../02-philosophy/04-antifeatures.md) — the design choices this
  page draws on.
- [Editor Integration](../18-tooling/09-editor-integration.md) — the LSP surface
  an assistant rides on, including the
  [adjustable-detail views](../18-tooling/09-editor-integration.md#adjustable-detail--show-less-or-show-more)
  that fit a context budget.

`TODO(open): this page is a living summary — keep it a linked index, not a
second copy of the detail. When a new AI-relevant feature lands in a chapter,
add a one-line pointer to the right cluster here. Candidate clusters not yet
called out on their own: capability-gated I/O and determinism (reproducible
runs an assistant can trust) and the adjustable-detail LSP views (fitting code
into a context budget) — both linked above but not yet given their own
section.`
