# Linter

<!-- TODO: review -->

## What: there is no separate official linter

"Linting" in Tel is **two distinct things**, and neither is a standalone
official linter tool:

1. **Official warnings are part of the compiler.** Every check Tel ships —
   unused symbols, suspicious patterns, deprecated stdlib calls, the
   bug-catalogue patterns below — is a *compiler warning*, surfaced by
   `tel check` and the LSP like any other diagnostic. There is no separate
   `tel lint` binary to keep in sync, and no official lint set that lives
   outside the compiler.
2. **Custom lint rules are a build-tool customization.** Project- or
   crate-declared rules — structural assertions, import-layering rules,
   review invariants — are one of the *few* customizations the build tool
   deliberately accommodates (see [Build System](03-build-system.md)). They
   are declarative matchers over the resolved IR, never arbitrary code that
   runs at compile time.

The rest of this chapter uses "lint" for the union of the two. Where a check
is built in, read it as a **compiler warning**; where it is project-declared,
read it as a **custom build-tool lint**.

Both are Tel's tool for **probable but not provable** bugs. The type checker
rejects what it can *prove* wrong; many real mistakes are not provably wrong —
a `.contains` on a hot list, a domain value left as a bare `Int64`, a
likely-stale field — only *probably* wrong. These surface as **advisory**
diagnostics (`warn`) the build can proceed past, rather than hard errors that
stop it. This is the same "do not reject what is only *probably* a mistake"
stance the language takes for
[provable panics](../13-error-handling/04-panics-and-aborts.md#provable-panics-warn-do-not-reject):
warn loudly, but let the human (or the AI assistant reading the warning)
make the call.

## Why

The compiler already enforces the things that *must* be true for code
to run; warnings and custom lints flag what *should* be true for code to fit a
project's local rules. In Tel two things make this load-bearing:

- **Stability discipline.** Because Tel itself almost never changes,
  the `std` library does the visible evolution (see
  [`../17-standard-library/01-stdlib-organisation.md`](../17-standard-library/01-stdlib-organisation.md)).
  When a stdlib item is deprecated and an automated migration ships,
  a compiler warning surfaces the deprecation and offers the rewrite.
- **Compile-time invariants over a project.** There is a need for
  structural assertions across the whole project ("every implementer
  of `X` is `Y`"). These are not properties of one type — they are
  properties of a *codebase*, and are exactly the custom build-tool lints
  of point 2: declarative, over the resolved symbol graph, surfaced in the
  editor.

## Categories of rules

The checks fall into three buckets:

1. **Style (compiler warnings).** Naming conventions, redundant `let uniq`,
   dead code, unused imports, suspicious patterns (`expect` outside a test, a
   `must` on a value statically provable to be present).
2. **Deprecation and migration (compiler warnings).** A stdlib item marked
   deprecated triggers a warning with a fix-it that rewrites the call to the
   replacement. The same mechanism is available to third-party crates via a
   deprecation attribute.
3. **Structural / project rules (custom build-tool lints).** Rules declared by
   the project itself — see below.

## Structural rules

A project (or a library) can declare structural rules that the
compiler/linter enforces across the codebase. The shape:

- "Every type implementing trait `T` must also implement trait `U`."
- "Every type in module `api` must be `Serializable`."
- "Module `core` may not import from module `experimental`."
- "No function may panic without an explanatory comment."
- "Every public function in `api` must have a doc comment with at
  least one example block."

A sketch:

```tel
# project-level lint declaration
lint serializable_api {
    "all types in module api must implement Serializable"
    for ty in module(api).types {
        require ty.implements(Serializable)
    }
}
```

These overlap with the *trait test* and *compile-fail test* forms in
[`11-testing.md`](../14-testing/01-testing.md) — both let a project assert
something about its own shape. The line between them:

- **Testing** is the home for assertions that run, even when they are
  compile-time evaluated. A test owns example instances, expected
  failure messages, and is reported in the test summary.
- **Linting** is the home for assertions that constrain the *shape* of
  the codebase without running anything. A lint violation is reported
  alongside other diagnostics in the editor, not in the test summary.

`TODO(open): the boundary is fuzzy. Several inputs treat
"test for every implementation of an interface" as a test feature, but
"no module under core imports from experimental" feels much more
linty. Pick a consistent rule of thumb and document it; possibly both
features compile to the same underlying mechanism.`

## Architectural / boundary rules

A common case worth calling out: enforce crate or module boundaries.
A monolithic project may want a lint that fails the build if a low-level
module reaches into a high-level one — the inverse of allowed
imports. Tel's import system is already explicit (see
[`../11-modules-and-packages/02-imports.md`](../11-modules-and-packages/02-imports.md));
the linter exposes a thin DSL for asserting which import edges are
permitted.

```tel
lint layering {
    deny import from "ui" to "data.persistence"
    deny import from "core" to "experimental"
}
```

`TODO(open): syntax for the layering DSL — strings vs module paths,
glob support, severity (warn vs deny).`

## Review invariants — unprovable, re-checked on change

Some invariants a programmer cares about cannot be expressed as a type,
a contract, or a decidable lint, because confirming them takes
judgement: *"`transfer` always performs an authorisation check before
moving money"*, *"this cache is only read on the task that filled it"*,
*"callers must already hold the account lock"*. Tel lets these be
**stated** as a review invariant attached to a declaration, so they live
next to the code instead of in a design doc nobody re-reads.

```tel
@invariant("every path performs an auth check before moving money")
fn transfer(from: Account, to: Account, amt: EurAmt) -> Result[Receipt] { ... }
```

(Spelling open — `@invariant("…")`, an `## invariant:` doc-tag building
on the [`##` teldoc comment](../03-lexical-structure/07-comments.md), or
a `note` keyword. `TODO(open)`.)

A review invariant is **not** machine-checked — the compiler does not
try to prove the prose. What the toolchain owns is the decidable half:
**staleness**. The linter records a fingerprint of the annotated
declaration (its body, and the bodies it calls into) at the point the
invariant was last acknowledged. When that fingerprint changes, the
linter raises a *"review invariant may be stale"* diagnostic — the code
the invariant describes moved, so a reviewer must re-confirm the prose
still holds and re-acknowledge it. This keeps the invariant honest
without asking the compiler to understand it.

The re-verification itself is a job for a **reviewer — human or AI**. It
is the natural anchor for the LLM-review hook the editor chapter leaves
open ([`09-editor-integration.md`](09-editor-integration.md)): an
assistant reviewing a diff gets, for each touched function, the exact
list of prose invariants it must re-establish, instead of guessing what
mattered. Tying the obligation to the declaration is what turns
automated review from open-ended into *checkable*.

### Requirements on callers

A review invariant can be aimed at a function's **callers** rather than
its body:

```tel
@requires_caller("validate the order against the live price book first")
fn submit(order: Order) -> Result[OrderId] { ... }
```

When a *new* call site appears, or an existing one changes, the
staleness check fires at the **call site**, not the definition, and the
reviewer confirms the caller meets the stated obligation. This is the
prose, unprovable sibling of a
[pre-condition](../02-philosophy/03-features.md): a requirement the
compiler can check becomes a contract; one that needs judgement becomes
a caller requirement that review tooling tracks.

`TODO(open): this may deserve its own topic, or a place in a future
"why Tel suits AI-assisted development" summary. For now it lives with
the linter because the decidable part — staleness tracking — is
lint-shaped. Decide the fingerprinting granularity (textual body vs
resolved IR vs call-closure), and how an acknowledgement is recorded (a
checked-in sign-off file, a hash carried in the annotation, or VCS-blame
integration).`

## Severity

Each lint has a severity: `allow`, `warn`, `deny`. A project file pins
severities per rule. That is the blanket switch — the right tool when a
rule is wrong *for the project as a whole* (a heuristic tuned for
long-running services firing all over a batch script, say).

### Per-declaration suppression: `@allow`

The blanket switch is the wrong tool when a rule is right for the
project but wrong at one site. Lints are heuristic by definition — a
check that is *always* right is a hard error, not a warning — so false
positives at individual sites are expected and normal. Without a local
escape, a false positive forces a bad choice: disable the rule
everywhere (losing it at the sites where it is right) or live with a
permanent warning (which trains readers to skim past the diagnostics,
devaluing every other warning). So Tel provides a per-symbol
suppression attribute:

```tel
@allow(list_contains, "config list has <10 entries, read once at startup")
fn load_config() -> Config { ... }
```

The design constraints:

- **An attribute, not a comment.** There is no `# noqa` /
  `// NOLINT`-style magic-comment suppression. `@allow` is part of the
  fixed attribute set (see
  [Derive and Attributes](../15-metaprogramming/03-derive-and-attributes.md)):
  it is parsed, so a typo in the rule name is itself a diagnostic, and
  tooling can enumerate every suppression in a project, report ones
  whose rule no longer fires, and offer their removal — none of which a
  comment convention can guarantee.
- **Declaration boundaries only.** `@allow` attaches to a declaration
  (function, type), never to an arbitrary expression or statement.
  Suppressions stay few, visible in review, and greppable, rather than
  accreting line by line.
- **A reason is required.** The second argument is mandatory prose
  saying why the rule does not apply here, in the same spirit as
  [`@invariant`](#review-invariants--unprovable-re-checked-on-change).
  It keeps the suppression honest to the next reader and makes the lazy
  reflex — silencing a warning instead of reading it — cost at least
  one justifying sentence.

`TODO(open): granularity. A wide function with one false positive
suppresses the rule for the whole body. Options: allow `@allow` on
local `let` bindings as well (still a declaration, finer grain), or
accept the coarseness as pressure to keep functions small. Lean: allow
on local bindings, no expression-level form.`

`TODO(open): should `tel check --strict` (or CI configuration) be able
to list suppressions for audit, or fail when a suppression is stale
(the suppressed rule no longer fires on that declaration)? Lean: yes to
both; staleness mirrors the review-invariant fingerprint machinery.`

## Integration with deprecation

When `std` deprecates a name, it ships a paired *rewrite* — a
structured transformation from the old call to the new one. The
linter applies the rewrite as a fix-it in the editor and as a
`tel fix` batch transform. This is how the *stability commitment* and
the *library is allowed to grow* commitment coexist: scripts using a
deprecated name keep working, the linter nudges, and an automated
rewrite is one keystroke away. See
[`../17-standard-library/01-stdlib-organisation.md`](../17-standard-library/01-stdlib-organisation.md)
for the deprecation model and
[`04-package-manager.md`](04-package-manager.md) for how third-party
crates plug in.

## Custom lints

Custom lints are the one place the toolchain accommodates project
customization: beyond the compiler's built-in warnings, a project (or a
published crate) can declare its own rules, and the **build tool** is what
loads and runs them (see [Build System](03-build-system.md)). This is a
deliberate, narrow extension point — Tel otherwise resists configuration, but
project-specific structural rules are valuable enough to earn it.

Custom lints are *not* arbitrary user code that runs at compile time — that
conflicts directly with *no proc-macros* in
[`../02-philosophy/04-antifeatures.md`](../02-philosophy/04-antifeatures.md).
Instead, they are *declarative* matchers over the resolved IR: patterns over
types, calls, imports, and module structure.

`TODO(open): the exact lint DSL. Possible shapes: a small query
language over the symbol graph, a set of fixed predicates parameterised
by user values, or a `match`-shaped pattern over IR nodes. Lean:
declarative query over the symbol graph, with no general escape
hatch.`

## Built-in pattern lints from the bug catalogue

A few representative patterns the built-in linter should catch, drawn
from a catalogue of real-world bug classes:

- **`.contains` on a `List` where the list is large or hot.** O(n) lookup
  on a list when the workload would clearly use a `Set`. See
  [`../17-standard-library/04-core-collections.md`](../17-standard-library/04-core-collections.md).
- **Same call appears in both "request" and "extract result" positions
  with the same name spelt slightly differently** (e.g. `forward_delta`
  vs `foreward_delta`, or `/forward_delta` vs `*forward_delta`).
  Catalogue: "two places out of three spelt the field one way, one
  spelt it another, only surfaced when the value wasn't 1.0." A
  lint that flags two distinct-spelling usages of an otherwise-paired
  identifier is a small Levenshtein check; cheap to ship.
- **Repeated work inside a tight loop / per-redraw computation.**
  Catalogue: "GUI recomputed a value for every entry on every
  repaint." A lint that flags an apparently-constant call inside a
  paint/render loop is heuristic but worth running.
- **Mutation of a value after handing it to another scope.** Catalogue:
  the simulation/persistence cases. The
  [`uniq` discipline](../06-bindings-and-scope/02-mutability.md) makes
  this a *type* concern; the lint surfaces the cases where the *type*
  is mutable but the dataflow shows a hand-off.
- **A `Result` whose error case is matched as `_`.** Indistinguishable
  from explicit drop, but worth a warning unless the caller used
  `.discard()`.
- **Time arithmetic on a bare numeric without a duration type.**
  Catalogue: nanos-vs-millis confusion in a field documented in
  neither name nor docstring.
- **`null`-shaped sentinels** (`-1` for "missing id", `0.0` for "no
  data", empty string for "absent"). The lint flags an `Int64` field
  named "id" being compared to `-1` or a literal numeric used as a
  missing-data sentinel.
- **Primitive obsession** — a domain value carried as a bare primitive
  where a [refined type or newtype](../05-types/12-refined-types.md)
  would prevent a mix-up. The sharpest heuristic is a signature with
  several **same-typed** primitive parameters (`fn transfer(from: Int64,
  to: Int64, amount: Int64)`): nothing stops a caller swapping `from` and
  `to`, and a `AccountId` / `EurAmt` trio would make the swap a type
  error. The lint suggests wrapping; it does not force it (a local
  one-off stays a primitive). This is the *discouragement* side of the
  language's push toward meaningful wrappers — see
  [refined types](../05-types/12-refined-types.md#bugs-this-prevents).
  `TODO(open): tune the heuristic so it nudges on genuine domain values
  (two `Int64` IDs, an unlabelled `Text`) without nagging on obviously-fine
  primitives (a loop index, a `Bool` flag). Likely keys off arity of
  same-typed params plus whether the value crosses a public boundary.`

## See also

- [Compiler](01-compiler.md)
- [Testing](../14-testing/01-testing.md) — compile-fail tests and trait tests
- [Package Manager](04-package-manager.md) — automated migration
- [Standard Library Organisation](../17-standard-library/01-stdlib-organisation.md) —
  the deprecation model that drives the migration lints
- [Antifeatures](../02-philosophy/04-antifeatures.md) — why custom
  lints stay declarative
