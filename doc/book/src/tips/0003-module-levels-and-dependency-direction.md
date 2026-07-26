# TIP-0003: Module Levels, Namespaces, and Dependency Direction

**Status:** Accepted (2026-06-17) — content migrated into the chapter docs; kept as the historical record. (Revised 2026-06-15 after review.)
**Touches:** `11-modules-and-packages/01-modules.md`, `11-modules-and-packages/02-imports.md`, `11-modules-and-packages/03-visibility.md`, `11-modules-and-packages/04-packages.md`, `11-modules-and-packages/05-project-layout.md`, `11-modules-and-packages/06-versioning.md`, `11-modules-and-packages/10-workspaces.md`, `02-philosophy/01-priorities.md`

## Summary

This TIP settles the entangled module/crate questions the chapters left as
scattered `TODO(open)` markers:

1. **How many levels are there, and which ones does the user name?**
2. **Namespaces** — how a project with hundreds of crates stays out of each
   other's way in the identifier space, decoupled from what gets released.
3. **Do crates have parents?** (Maven-style nested modules.)
4. **Dependency direction (api vs impl)** — whether splitting API from
   implementation deserves *language* support.

The 2026-06-15 review resolved most of these. The headline change from the
first draft: **three levels, not four** (the *sub-project* role is dropped and
absorbed into "an unpublished crate"), and **api/impl becomes a deliberately
tiny language feature** — three dependency flavours (`api` / `impl` /
unspecified) whose *only* rule forbids one of the nine combinations: an `api`
unit may not depend on an `impl` one.

## Recommended outcome (one-line summary)

- **Three user-named levels: `module`, `crate`, `workspace`.** The
  *compilation unit* is still not one of them (the compiler owns it), and the
  earlier *sub-project* level is dropped — its two jobs move to the **crate**
  (the distributable, dependency-bearing unit) and to **unpublished workspace
  members** (encapsulation without release). Only `module` is mandatory; `crate`
  and `workspace` appear when a project grows. **Names decided** (see
  [below](#decided-level-names-module--crate--workspace)); the chapter docs
  have been migrated to *crate* for the middle level.
- **Namespaces are a separate axis from the hierarchy**, settable on the
  workspace or the crate; **the crate wins if both set it** (same rule as
  versions). Publishing a workspace's members atomically is only allowed when
  they **share a namespace**.
- **Crates do not have parents.** Crate directory nesting is *lexical
  organisation only* — a nested crate inherits nothing from the directories
  above it. Maven-style parent POMs are rejected; their real jobs move to the
  workspace.
- **Dependencies are non-transitive by default, with opt-in re-export.** On top
  of that, a unit may classify itself (and/or its dependency edges) as **`api`**,
  **`impl`**, or leave it **unspecified**. The whole feature is one forbidden
  combination out of the 3×3: **an `api` unit may not depend on an `impl` one.**
  Everything else is allowed, and unspecified pays no tax — so a script names
  none of it (see [below](#dependency-direction-the-lightweight-apiimplexecutable-flag)).
- **Visibility has three levels** (crate-default, module-private, external
  export), and a **crate's export block is mandatory and explicit**.

## The level count: three, not four

The first draft proposed four roles (module / sub-project / crate /
workspace). Review collapsed this to **three**:

| Level | Problem it solves | Mandatory? | Nesting | Versioned? |
|-------|-------------------|------------|---------|------------|
| **module** (files) | code organisation; the unit where `private` applies and where internal references need no import | yes | meaningful (children see parents) | no |
| **crate** (distributable) | the publishable unit; **where dependencies apply** | no | lexical only (no inheritance) | yes |
| **workspace** | the bundle of crates developed together: patches internal versions, builds in dev mode, runs tests inside-but-not-outside | no | n/a | no (members differ) |

The **compilation unit** still has no surface representation — the compiler
chooses its units freely; source never names one. This is unchanged from the
first draft and from [`01-modules.md`](../11-modules-and-packages/01-modules.md).

### Decided: level names (module / crate / workspace)

The distributable level is named **`crate`**, not `package`. The names problem is
real: `module` and `package`/`crate` mean *opposite* things across ecosystems
(Rust: module ⊂ crate; Java: package ⊂ module), so any choice reads backwards to
some audience. The tiebreaker:

- **`module`** (internal/file unit) is kept — it is the majority intuition
  (JS/Python/Rust/Go-files all read "module" as file-level/internal) and inverts
  fewest readers.
- **`crate`** is chosen for the distributable over `package` because of a
  decision *this TIP* made: distributable names are **dotted and hierarchical**
  (`user.auth.google`). A dotted name looks exactly like a **Java package** —
  which is Java's *internal* unit — so calling it "package" doesn't merely invert
  Java terminology, the **dotted syntax actively reinforces the wrong mental
  model** for a very large audience. `crate` has **one meaning** in programming
  (no inversion anywhere), carries a *distribution* metaphor (a shipping crate)
  that pairs with `workspace`, and fits Tel's existing open Rust influence.
- **Familiar-but-inverted causes active mis-modelling** (a Java/Go dev confidently
  builds the wrong model); **unfamiliar-but-correct causes a one-time lookup**
  (no wrong model ever forms). Unknown beats backwards.

Rejected: `package` (the dotted-name inversion above), `library` (imports a
binary/non-binary distinction Tel does not have — Rust's lib-vs-bin), `bundle`
(no edge over `package` on the "combines modules" axis, less familiar),
`artifact`/`assembly` (jargon; "assembly" collides with assembly language).

`crate` names are conventionally flat in Rust, so Tel's *dotted* crate names are
a slight stretch of the term — an accepted, minor cost.

### Why the sub-project level is dropped

The first draft kept a *sub-project* — a non-published encapsulation boundary
with its own dependency edges. Review folded it away because its two jobs each
already have a home in the three-level model:

- **The dependency edge** moves to the **crate** ("crate is where
  dependencies apply").
- **Encapsulation without publishing** becomes an **unpublished workspace
  member** — a crate the workspace builds and depends on internally but never
  releases (see [dev-only members](#dev-only-workspace-members)).

So "encapsulate a chunk of a large project without giving it a registry name"
is just *a crate you do not publish*, not a fourth concept to learn.

## Namespaces

A **namespace** is a naming prefix that groups crates for identifier hygiene.
It is **not** the hierarchy and **not** the distribution unit.

- **Namespaces do not nest.** A namespace is a single flat prefix, not a tree —
  there is no `acme.pricing` *under* `acme`. The hierarchy lives in the crate and
  module names, not in the namespace.
- **A fully-qualified name is `namespace . crate-path . module-path`** — one flat
  namespace, then the crate's (possibly multi-segment, lexical) name, then the
  (possibly nested) module path, then the member. e.g. in
  `acme.user.auth.google.token.secret`: `acme` is the namespace,
  `user.auth.google` the crate, `token` the module, `secret` the member. The
  namespace as the leading segment is what makes every FQN globally unique, so
  two namespaces may each hold a `google` crate without collision.
- **Where it is set:** on the **workspace** or on the **crate** — either or both.
  **If both set it, the crate wins** (identical to how a crate's version
  overrides a workspace-supplied bound; see
  [`10-workspaces.md`](../11-modules-and-packages/10-workspaces.md)).
- **Crates within a namespace are leaves** — their names form a prefix-antichain:
  you cannot have both `a.b` and `a.b.c` as crates in one namespace. A dotted
  crate name is therefore always a leaf, never also a grouping parent.
- **Atomic publish is namespace-scoped:** a workspace may publish several
  members **atomically only when they share a namespace**. Cross-namespace
  members release independently.

This answers the inputs' *"a workspace with hundreds of crates should not eat
hundreds of top-level identifiers."*

## Crates have no parents — and what their names look like

Maven's parent-module hierarchy does two real jobs, both already homed
elsewhere in Tel:

- **Shared config / dependency versions** → the **workspace** (declare shared
  deps once; `$current` for intra-workspace edges — see `10-workspaces.md`).
- **Aggregation / grouped naming** → the **namespace** axis above.

So **a crate has a workspace (release coordination) and a namespace (naming),
but never a parent crate.** A crate nested in a directory under another crate
inherits *nothing* from it.

**Modules are the exception: modules *do* have parents.** Only crates and
workspaces are parentless. A module has a parent module and **can see items from
its parents** (the nesting is meaningful in both directions — a parent reaches
its children, and a child reaches its ancestors' items, all without an import).
And where a crate's name need not follow its directory layout, a **module's path
*must* mirror the directory structure** (that is the default for crates too, but
only crates let the workspace override it).

### Crate names: hierarchical, but the workspace decides the mapping

Directory layout and crate name *may* match, but they are **not forced** to —
a crate is not opened in any particular directory combination in an IDE, so
its relative on-disk location should not be load-bearing. The resolution:

- **The workspace MUST list its members**, and **MAY dictate their paths**. When
  it does not, a **default directory mapping** applies.
- The member list can be written as a **nested block that mirrors directories**
  and yields a hierarchical (dotted) name:

  ```text
  # Illustrative — nested form mirrors dirs: yields user.auth.google
  members { user { auth { google } } }

  # Explicit-path form — no nesting needed, name need not match the dir:
  members { google = "src/vendor/google-auth" }
  ```

- The dotted name (`user.auth.google`) is **purely lexical**: the segments are
  *not* parent crates, and `user.auth.google` neither requires nor inherits
  from any `user` or `user.auth`. This keeps the "no parents / no inheritance"
  decision honest while matching [module](../11-modules-and-packages/01-modules.md)
  path syntax so users learn one form.
- **Published names form a prefix-antichain:** a name is either a *leaf* (a real
  crate) or a *grouping prefix*, **never both**. So `user.auth` and
  `user.auth.google` cannot both be published — that is what stops one crate
  from growing into another's import path. The
  [registry](../11-modules-and-packages/02-imports.md#how-a-dotted-root-stays-unambiguous)
  enforces it per namespace (owner-scoped, so the check is local).

This rejects the bare leaf-only scheme (collides with every other `google`) and
the slash-flattened `user-auth-google` scheme (clashes with a literal top-level
crate of that name and fights the
[hyphen↔underscore rule](../11-modules-and-packages/02-imports.md#hyphens-and-underscores)).

`TODO(open):` the exact member-list syntax (nested block vs explicit paths) and
the default mapping; track in
[`10-workspaces.md`](../11-modules-and-packages/10-workspaces.md) and
[`05-project-layout.md`](../11-modules-and-packages/05-project-layout.md).

### Will Tel miss Maven's inheritance?

Maven leans hard on parent-POM inheritance for: shared dependency management,
shared build/plugin config, shared properties, reactor aggregation, and common
metadata. In Tel:

- dependency management → workspace `shared_dependency`;
- build config → the opinionated build + workspace hooks;
- properties → unneeded (the manifest is declarative, not scripted);
- aggregation → workspace membership + the dependency graph;
- common metadata → workspace defaults propagated to members (the namespace
  axis is the main one).

**The one thing genuinely given up** is Maven's *cross-project, independently
published* parent (the `spring-boot-starter-parent` pattern): a published config
baseline that *other* repos inherit. Tel deliberately has no equivalent —
inherited config across the dependency edge is exactly the "transitive POM
osmosis" the [versioning bug catalogue](../11-modules-and-packages/06-versioning.md#bugs-the-version-discipline-prevents)
blames for real outages. **Recommendation: accept the loss.** If a shared
baseline is ever wanted, it belongs to a scaffolding/template tool, not a
runtime inheritance mechanism.

## Dependency direction: the lightweight api/impl/executable flag

**Decision (review):** Tel adds a **deliberately tiny** dependency-direction
feature. A crate is classified one of four ways — **`executable`**, **`api`**,
**`impl`**, or left **unspecified** — and the feature has just **two rules**:

| | → `executable` | → `api` | → `impl` | → unspecified |
|---|:---:|:---:|:---:|:---:|
| **`executable`** | ❌ | ✅ | ✅ | ✅ |
| **`api`** | ❌ | ✅ | ❌ **forbidden** | ✅ |
| **`impl`** | ❌ | ✅ | ✅ | ✅ |
| **unspecified** | ❌ | ✅ | ✅ | ✅ |

1. **`api` → `impl` is forbidden** — an API layer must not rest on an
   implementation detail.
2. **Nothing may depend on an `executable`** — it is the *top* of the stack: it
   may import anything, but no crate may import it.

`executable` is the application/entrypoint crate. It **must expose at least one
entrypoint**, can depend on every other class, and is depended on by nothing
(the whole right column is forbidden because the only way *to* an executable is
to run it, not import it). Everything else compiles, and **unspecified is the
default that pays no tax** — a small script writes no classification and meets
neither rule.

The point is that this is *cheap*: two forbidden edges, no new visibility or
re-export machinery (those already exist), and it stays out of the way until a
team marks a crate `api` or `executable`.

### Relationship to "crate kinds"

This subsumes the old [`04-packages.md` "executable as a crate kind"](../11-modules-and-packages/04-packages.md#crate-kinds-no-plugin-kind-for-now)
question. `executable` *is* that kind, expressed as the top of the dependency
ladder rather than as a separate build-output concept: it is the home for
Tel-built CLI tools and workspace utilities. The *host-embedding* case is
unchanged — when Tel runs as a guest, the **host** is still the executable and
the script is an ordinary (often unclassified) crate; `executable` is for the
case where a Tel crate is itself the program's entry point.

### What the flag buys that convention alone cannot

The same split can be *approximated* by putting traits/types in one module and
logic in another and wiring with dependency injection. What the flag adds:

1. **Compiler-enforced "api may not depend on impl"** — the one thing convention
   cannot give; it makes the architectural violation a *compile error* instead
   of a review catch. This is the feature's entire reason to exist.
2. **Selective stable-on-unstable enforcement** — an `api` dep on an unstable
   crate can be the *hard error* case while an `impl` dep is merely
   risky-but-allowed (the rule in
   [`04-packages.md`](../11-modules-and-packages/04-packages.md#stable-depending-on-unstable)).

Other potential payoffs (impl-type non-leakage, incremental-build scoping, a
mechanical docs boundary) Tel **already** gets from opaque types, the mandatory
[crate export block](../11-modules-and-packages/03-visibility.md#crate-export-block),
and non-transitive-by-default deps — so the flag is not justified by *those*; it
is justified solely by reason 1, which is exactly why it stays a two-rule
feature rather than a full Gradle-style configuration system.

**Decided: the classification lives on the unit, not the edge.** A crate
declares "I *am* an `api` crate" (or `impl`, or `executable`, or leaves it
unspecified); the forbidden-edge rules then follow from the two endpoints'
classes — an `api` crate depending on an `impl` crate, or *any* crate depending
on an `executable`, is the error. This is what teams actually want to say, and
it avoids re-stating a direction on every edge.

`TODO(open):` only the **spelling** of the classification remains; tracked in
[`04-packages.md`](../11-modules-and-packages/04-packages.md#the-lightweight-apiimplexecutable-flag).

## Visibility: three levels, mandatory crate export

Adopt a three-level model that fits the embedded-scripting default:

1. **Crate-visible (default).** A top-level item with no marker is visible
   throughout its **crate** but not outside it. This is the default because
   most scripts are a single file where everything should "just be usable."
2. **Module-private (`private`).** Opt in with a marker; the item is visible
   only inside its defining **module and that module's children** (modules nest
   meaningfully).
3. **Externally public.** An item is visible to *other crates* iff it is
   **(a) listed in the crate's `export` block and (b) not `private`.**

A **crate's export block is mandatory and explicit** — external surface is
never implicit. A **module may also carry an export block**, which the crate
export block then references (so a crate curates its outward API from the
modules' offered surfaces).

This dissolves the first draft's "public-by-default vs explicit-export-block"
tension: *internal* visibility is crate-default (frictionless for scripts),
while *external* surface is the deliberate export-block act. We do **not** need a
separate "visible in the whole crate but not externally" marker — that is
exactly the default state.

### Recommended `export` syntax and `private` keyword

**`export { … }` lists in-scope names — no `from` clause.** Bringing names into
scope is `import`'s job; `export` only decides which of the names already visible
in this unit cross the boundary. So **re-export is just import-then-export**:

```tel
import regex                          # bring the path into scope
export { convert, Money, regex.Regex } # own items + a re-exported external path
```

A bare name in the block re-exports the in-scope item under its own name. This
drops the ES-module `from` clause as redundant (the path already says where a
name is from) and beats Rust's scattered `pub` / `pub use` (no single place to
read the API).

**The export block defines the public API tree *independently* of the code
layout.** This is a deliberate change from the first draft's "re-export never
renames" rule. Every entry is **`<public-path> = <in-scope-internal-path>`**,
where *either side may be dotted/nested* — so `a.b` can be exposed as `c.d`. A
bare entry is the degenerate case (`name` ≡ `name = name`):

```tel
export {
    convert                              # public `convert`  = internal `convert`
    Money     = pricing.money.Amount     # shallow public name, deep internal item
    c.d       = a.b                      # deep -> deep: expose internal a.b as c.d
}
```

A nested left-hand block is just sugar for shared public prefixes — these two
are identical, so use whichever reads better:

```tel
export { c.d = a.b,  c.e = a.f }     # dotted public paths
export { c { d = a.b,  e = a.f } }   # same, grouped under the public `c`
```

The payoff is **backwards compatibility through refactors**: move or rename code
internally and only the *right-hand sides* of the export block change — the
public left-hand paths, and therefore every consumer, are untouched. The nested
left-hand form lets the public shape be curated in one place; **a distributed
form (each module carrying its own `export`) is still allowed but never required
for nesting.**

Why this does not reopen the [no-rename](02-imports.md#renaming-only-at-the-export-boundary)
problems: renaming is still forbidden everywhere *except* this one declared,
reviewable place. Consumers still cannot alias on import; crate-internal code
still uses real paths; only the crate's own export facade may map internal →
public — which is exactly "the parent decides what it exposes," taken to its
conclusion. Local readability holds on both sides, and "what is this public name
and where does it come from?" has a single place to read.

**The private marker is `private`** (spelled in full). Tel abbreviates only its
*ultra-frequent* keywords (`fn`, `let`, `pub`) and spells the rest out
(`match`, `struct`, `trait`, `return`, `import`, `export`); a visibility opt-out
is rare, so readability wins over brevity → the full word. `local` is **not**
available (it is already a [keyword](../20-appendix/01-keywords.md) for
scope-local bindings), and with public-as-default there is no `pub` sibling that
a `priv` would need to match. `private` is also the most universally understood
visibility word (Java/C#/Kotlin/Swift/TypeScript).

## Transitivity and version conflicts (confirmed)

- **Dependencies are non-transitive unless opted into** via re-export
  ([`04-packages.md`](../11-modules-and-packages/04-packages.md#transitive-dependencies-opt-in-only)).
- **Version conflicts** follow the already-decided
  [one-version-per-connected-component](../11-modules-and-packages/06-versioning.md#decided-one-version-per-connected-component)
  rule: the resolver tries a single shared version; if it succeeds, fine; if not,
  the *types-from-different-versions-are-different-types* rule governs — a
  `Person@1.2.0` and a `Person@2.0.5` may coexist as long as they never "touch."
  They cannot touch unless **both** transitively reach the same point; on
  disconnected branches they are independent. This confirms the first draft's
  two-branch lean and needs no new mechanism — it is the existing versioning
  rule.

## Dev-only workspace members

A workspace may have **members that are not published** alongside the others —
test harnesses, fixtures, internal tools. These are the new home for "encapsulate
without releasing" (the old sub-project job). One subtlety carried from review:
an **atomic workspace release must run the project's checks *without* the
dev-only members present**, so the release mirrors what an external consumer
actually receives (a check that secretly leaned on a dev-only member would pass
in-workspace and fail for users).

## Major-version compatibility enforcement (new)

The registry should **require a major-version bump when a crate's public API
is not backwards compatible**. Scope:

- **In scope:** changes where *calling code could stop compiling* — a removed or
  renamed public function, a changed signature, a removed type, a new
  non-defaulted union variant that breaks exhaustive matches. The
  [generated API summary](../11-modules-and-packages/06-versioning.md#generated-api-summary)
  is the mechanical basis.
- **Out of scope:** functional changes the compiler cannot see (returning wrong
  values) — no tool can catch these; honesty requires saying so.

### Pre/postconditions: not enforced, by decision

**Decided: examples can be freely removed, so pre/postconditions are *not* part
of the enforced compatibility check.** Requiring examples to be kept was the only
way to make contract-compatibility checkable, and that is unworkable (it would
make pruning or refactoring an example set a breaking change). So conditions and
their examples may change in any release.

The cost is honest and documented: **removing or loosening an example is a way to
work *around* the backwards-compatibility gate** — it can hide a real break from
the [API-summary check](../11-modules-and-packages/06-versioning.md#generated-api-summary).
The docs should warn that this is an escape hatch, used deliberately, not a
free edit. The **union-exhaustiveness** break (a new variant breaks exhaustive
matches) is *separate* and stays in scope regardless.

## What this TIP does *not* do

- **No compilation-unit syntax.**
- **No parent crates, no published config inheritance.**
- **No Gradle-style dependency-configuration system** — the api/impl feature is
  one forbidden edge, not a configuration matrix.
- **Does not settle the dependency-version *ownership* file** (workspace vs
  package manifest) — that remains
  [`06-versioning.md`](../11-modules-and-packages/06-versioning.md)'s open
  question.
- **Does not define capabilities** — orthogonal; see
  [`04-packages.md`](../11-modules-and-packages/04-packages.md).

## Open questions (remaining after review)

**Decided** (this section is now mostly resolved):

- **Level names: `module` / `crate` / `workspace`** (see
  [Decided: level names](#decided-level-names-module--crate--workspace)). The
  `package → crate` rename across the chapter docs is
  [done](#migration-plan-if-accepted).
- **Classifications:** `executable` / `api` / `impl` / unspecified, on the unit;
  rules `api↛impl` and `nothing→executable`.
- **Namespaces do not nest**; FQN is `namespace.crate.module`; crates are leaves
  per namespace (prefix-antichain).
- **`export` block** lists in-scope names, no `from` (`export { a, B, m.C }`;
  re-export by importing first). It **defines the public API tree decoupled from
  the code layout** — `public.path = internal.item` re-homes, so refactors keep
  backwards compat; nesting allowed, distributed per-module export optional.
  **`private`** is the visibility marker.
- **Pre/postconditions are not enforced** (examples may be removed; doc-warn it
  works around the compat gate).

`TODO(open):` what genuinely remains —

- **Exact member-list grammar** — the nested-block-mirrors-dirs form vs explicit
  per-member paths, and the default mapping when the workspace dictates neither.
  (Scheme settled; only the surface syntax is open.)
- **Final spellings** — confirm the `executable`/`api`/`impl` keywords, the
  `export`/`private` tokens, and whether a crate-root `export` may glob a
  module (`export { pricing.fx.* }`) once the wider grammar is pinned.

## Migration plan if accepted

All of the chapter edits below are **done** (2026-06-15; the
`package → crate` rename completed 2026-06-17).

1. ✅ `01-modules.md` — three-role model; sub-project section deleted and
   re-homed (dependency edge → crate; encapsulation → unpublished workspace
   member); compilation-unit drop kept.
2. ✅ `03-visibility.md` — three-level model (crate-default /
   module-private / export-gated external); mandatory crate export block
   referencing optional module export blocks; item-not-field export granularity.
3. ✅ `04-packages.md` — no parents, directory nesting lexical-only; dependency
   edge on the crate; **the one-rule `api`/`impl` flag** (classification on the
   unit); mandatory export block.
4. ✅ `10-workspaces.md` — namespace settable on workspace or crate (crate
   wins); atomic publish only within a shared namespace; member-list/paths;
   dev-only members and the run-checks-without-them rule.
5. ✅ `06-versioning.md` — major-bump backwards-compat enforcement
   (pre/postconditions stay `TODO(open)` pending the examples question).
6. ✅ `02-imports.md` / `09-package-registry.md` — dotted import root and the
   prefix-antichain rule.
7. ✅ `05-project-layout.md` — manifest is **two** scopes, not three; dropped the
   `internal/ sub-project` example.
8. ✅ **`package → crate` rename** across all chapters and this TIP's body
   (manifest keyword, prose, section anchors, cross-references). The folder
   `11-modules-and-packages`, the chapter filenames, and the compound tool
   terms ("package manager / registry / index / manifest") are kept verbatim.
9. ✅ Cross-linked affected chapters back here.
