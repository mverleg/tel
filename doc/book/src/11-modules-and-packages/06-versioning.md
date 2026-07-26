# Module Versioning

<!-- TODO: review -->

This page is about versioning of **crates** a project depends on — not about
the Tel *language* version. The language itself does not version: Tel is frozen
at 1.0 (internally *Tel1*), with no editions and no runtime churn (see the
[stability priority](../02-philosophy/01-priorities.md)). What *does* change
over time is the third-party [crates](04-packages.md) a project pulls in, and
this page is about resolving those.

## The starting point: one version per dependency

The simplest, preferred outcome of dependency resolution is a **single version
of each crate** across the whole project. When that succeeds there is no
versioning problem to reason about.

The hard question is what to do when it *fails* — when two parts of a project
need two different versions of the same crate.

## When a single version cannot be found

Several strategies were considered. Tel does not adopt the most permissive
ones.

### Rejected: trust semver

One option is to allow mixed versions and decide compatibility from the
version numbers — treat a semver-minor bump as compatible, a major bump as not.
**Rejected.** This trusts library authors to apply semver correctly, and
[safety over flexibility](../02-philosophy/01-priorities.md) says Tel should not
extend that trust. A misapplied minor bump would silently produce a broken
build.

### Rejected: complex usage-based compatibility rules

Another option is to inspect *how* a type is used and decide per use-site
whether two versions are interchangeable. **Rejected** as too complex,
unpredictable, and hard to explain — it fails *one good way over many clever
ones* and *if it looks correct, it probably is correct*.

## Decided: one version per connected component

Tel does **not** force a single global version of each crate, and does **not**
allow npm-style "every requirer gets its own copy." The rule is scoped to where
two versions could actually *meet*:

- **Within a connected part of the dependency graph** — where the two crates'
  types can reach the same type-checking context — a crate resolves to
  **exactly one version**. A real diamond (both arms feed the same consumer) with
  incompatible ranges is a **hard resolution error the user fixes**
  (the [Maven Enforcer](https://maven.apache.org/enforcer/) behaviour, scoped to
  the component), never a silent double-install.
- **Disconnected parts of the graph may resolve to disjoint versions** of the
  same crate. The two versions then produce **entirely separate types**:
  a `pricing.fx.Rate` from v1 and a `pricing.fx.Rate` from v2 are different types
  (`Rate@1` ≠ `Rate@2`); a value of one cannot be passed where the other is
  expected. Mixing is allowed precisely because, being disconnected, the two
  types never meet; silently *confusing* them is what is ruled out.

This is the old "Candidate B (versions distinct, cross-version types
incompatible)" rule **constrained to disconnected subgraphs**, with the strict
"Candidate A (reject mixed versions)" rule applied *inside* each connected
component.

**Why it preserves trait coherence.** [TIP-0005's orphan rule](08-dependency-graph-and-locking.md)
needs one `(trait, type)` impl wherever a given type is visible. Version-scoped
type identity guarantees exactly that: within the scope where `Rate@1` is
visible there is exactly one `Rate@1`, hence one impl. Two versions only ever
coexist where their types cannot meet, so coherence is never at risk — the
distinct-types rule is what *makes* mixed versions safe rather than what
threatens them. This is why [shading](04-packages.md#shading-not-the-answer) is
rejected: it hides the version distinction behind a rewritten name instead of
surfacing it in the type system.

### How types diverge, and how versions are chosen

**Decided (aggressive): a type is identified by `(crate, version)` — always.**
There is no cross-version type sharing, not even for *unchanged* types. This
drops the "only changed types diverge" refinement in favour of a rule that needs
no definition of "changed": every version boundary is a total type boundary. It
is safe because the resolver never lets two versions meet where their values
could interact (below), so the extra strictness is invisible in practice while
being trivial to implement and explain.

**Resolution objective.** Modules state version constraints — typically a
*minimum* with no upper bound, so compatibility runs to the next major. Across
the whole tree of build targets the lockfile then **minimises the number of
distinct versions** (prefer unifying to one), subject to one hard constraint:

- Where opt-in [transitive dependencies](04-packages.md#transitive-dependencies-opt-in-only)
  reach the **same** target through two paths, the versions **must align or the
  build fails** — the single-version-per-connected-component rule, enforced.
- Where dependencies do **not** transitively reach the same point, differing
  versions are **accepted**. Their `(crate, version)` types are distinct and,
  being disconnected, never touch — so the distinctness costs nothing.

"Minimise distinct versions" is a resolver *preference* (a tie-breaker toward
fewer copies); the hard constraint is the per-component single version. The exact
selection algorithm (MVS-style minimum, pubgrub-style, or custom) stays open
under [Dependency Graph and Locking](08-dependency-graph-and-locking.md). The
[package manager](../18-tooling/04-package-manager.md) owns this; the index only
exposes all versions and their declared ranges.

## Why this is strict

Tel's stability commitment is not just about the language; a script that
depends on crates should also keep building and behaving the same. Allowing
two versions of a type to be quietly interchangeable is exactly the kind of
"works until it doesn't" hazard Tel rejects. Whichever model is chosen, the
rule must be mechanical and explainable — never "the compiler guessed they were
close enough."

## Type inference and compatibility

A subtler hazard: even when an API is technically backwards-compatible,
[type inference](03-visibility.md#public-members-need-explicit-types) can break
downstream code. Adding a type or a trait implementation in a crate can make
inference in *user* code resolve to a different (still valid) type, changing
behaviour or failing a later call that needed the original concrete type.

Tel's mitigations:

- **Public functions require explicit signatures** — inference never silently
  decides a published API. See [Visibility](03-visibility.md).
- Inference rules are intentionally limited and one-directional, reducing how
  far a change can propagate.

TODO(open): inputs ask for a way to **pin a concrete type** at an inference
site — both as documentation and to stop inference from drifting. Decide
whether Tel offers such a type-ascription form and where it is documented
(likely the types chapter).

## Generated API summary

Inputs and [`03-features.md`](../02-philosophy/03-features.md) raise a
**generated public-API summary file** — signatures, inferred effects, declared
contracts — committed alongside source so that any change to the public API
shows up plainly in a diff. This is a stability-discipline aid: it makes API
churn reviewable.

TODO(open): keep, drop, or punt to tooling. If kept, it pairs naturally with
the [crate export block](03-visibility.md#crate-export-block) idea.

## Enforced major-version bumps

The [registry](09-package-registry.md) **requires a major-version bump when a
crate's exported API is not backwards compatible.** The
[generated API summary](#generated-api-summary) is the mechanical basis: the
index compares the new release's exported surface against the previous one and
rejects a non-major version whose change could break a consumer.

What "not backwards compatible" covers:

- **In scope — source-breaking changes the compiler can see:** a removed or
  renamed exported function, a changed signature, a removed exported type, a new
  non-defaulted union variant that breaks an exhaustive `match`. These are the
  changes where *calling code that compiled before would stop compiling*.
- **Out of scope — functional changes the compiler cannot see:** returning the
  wrong value, a behavioural regression behind an unchanged signature. No tool
  can catch these; the check does not pretend to.

### Pre- and postconditions are not enforced

**Decided: pre/postconditions are *not* part of the enforced compatibility
check.** Because Tel's [conditions](../tips/0004-how-far-refinement-types-go.md)
are checked against **listed examples**, the only way to make contract
compatibility checkable would be to forbid removing examples — and that is
unworkable (pruning or refactoring an example set would force a major version).
So conditions and their examples may change in any release.

The trade-off is real and must be **documented as a warning, not hidden**:
removing or loosening an example is a way to work *around* the
backwards-compatibility gate — it can let a genuinely breaking change ship under
a minor bump, because the [API-summary check](#generated-api-summary) no longer
has the example to catch it. Tooling and docs should flag a shrinking example
set as a deliberate escape hatch, not a routine edit.

The **union-exhaustiveness** break (a new variant breaking exhaustive matches)
is *separate* and stays in scope regardless; behavioural contracts the compiler
cannot verify were never in scope. See
[TIP-0003](../tips/0003-module-levels-and-dependency-direction.md#major-version-compatibility-enforcement-new).

## Per-method version support

The toolchain can, in principle, track *which crate versions each public
function or type first appeared in*, and use that to suggest a minimum version
bound for a dependency. If user code calls `Path.mkdirs` and that function was
added in `3.3`, the package manager can refuse a `<3.3` upper bound for that
dependency, or suggest tightening a wide range.

This pairs with the [generated API summary](#generated-api-summary): the same
data that makes API diffs reviewable can drive a per-symbol version-support
index.

TODO(open): keep this as a *suggestion* by the tool, or enforce it as a build
constraint? Lean: warn during development, fail the build if a dependency
range is tightened by hand and the lock file disagrees. Belongs partly here
and partly in [Tooling](../18-tooling/).

## End-of-life and retraction

A published version should be able to carry **machine-readable lifecycle
metadata**:

- An **end-of-life date** for a major (and possibly minor) version, so the
  toolchain can warn projects that pin a version line scheduled to stop
  receiving fixes.
- A **retraction** flag — published versions with known correctness or
  security problems can be marked retracted, so a fresh resolution avoids them
  and an existing lock file surfaces a warning at the next build.

Retraction does not *delete* a version (deleting would break reproducible
builds — see [Package Registry](09-package-registry.md)); it only signals that
the version should be avoided.

TODO(open): the exact metadata schema and where it lives — in the package
manifest, in the registry, or both. Lean: published in the registry so it can
be updated after the fact, mirrored into the lock file so offline builds still
see retractions resolved at last refresh.

## Automated rewrites for opt-in breaking changes

Tel itself is committed to *no* breaking changes
([stability priority](../02-philosophy/01-priorities.md)), but **crate
authors** do not have that guarantee from Tel — a crate can publish a v2 with
a different API. To soften that cost on downstream code, a crate can ship an
**automated rewrite script** alongside a new major version (the Rust-edition or
`2to3` model), so a consumer can upgrade most of the syntactic noise with a
command.

TODO(open): whether the rewrite mechanism is *defined by the language* (so
every crate can use one consistently) or left to the crate author's own
tooling. Lean: a language-defined, restricted rewrite format (text-level
pattern → replacement, scoped to the crate's exported names) so consumers do
not learn a new tool per dependency. Belongs in [Tooling](../18-tooling/) once
the shape is clearer.

TODO(open): how the rewrite tool interacts with Tel's *own* stability — Tel1
should not need a rewrite script for itself, ever, by construction. The
mechanism exists for *third-party crates*, not for the language.

## Pinning concrete types at use sites

A subtle inference hazard: a published API change in a dependency can shift
the type inference picks at a use site in *user* code, even when both old and
new are valid. Inputs ask for an explicit **type ascription** — a way to pin
the type at a binding so inference cannot drift later:

```tel
# Illustrative — exact syntax not pinned down.
let total: EuroAmt = items.sum_by(|i| i.price)
```

This is documentation as well as a defence against drift; reviewers see the
intended type, and the compiler refuses the assignment if a future change
narrows or widens it.

TODO(open): final spelling and where it is documented — likely in
[`05-types/`](../05-types/), with a back-reference from here.

## Version bumping discipline

A version number tends to live in several places — the manifest, a constant
inside the code, CI configuration, git tags. Inputs flag this as a recurring
source of bugs: skew between those copies leaves the wrong version on a
release.

Tel should pick a *single* authoritative location:

- **Manifest is the only source of truth.** The code reads the version through
  [`crate.info()`](04-packages.md#crate-metadata-from-code), CI reads it
  from the manifest, the git tag is produced by the release tool from the same
  manifest. There is no second copy to drift.
- **For in-project builds, no version is needed.** A workspace member used by
  another workspace member does not need to carry a numeric version at all —
  it builds against "whatever is in this tree." The numeric version only
  matters at publish time. (`-Drevision`-style overrides should not be
  necessary if there is no in-tree version to override.)

TODO(open): confirm the "no in-tree version" rule. The friction with
[per-method version support](#per-method-version-support) is mild — that
mechanism only kicks in across the dependency boundary.

## Reproducibility

A build is reproducible only if dependency resolution itself is reproducible:

- The **lock file** pins exact versions and checksums of every transitively
  resolved crate, so the same source produces the same dependency graph on
  every machine.
- A crate, once resolved by name+version, must yield the **same bytes**
  every time (verified by checksum). Registries that allow publishers to mutate
  a version in place are incompatible with this; Tel's
  [registry](09-package-registry.md) does not allow it.
- A local cache may serve a previously-fetched crate without re-asking the
  registry — but if the manifest changes (a version bumped, a dependency
  added) the cache must be consulted against the lock file, not silently
  preferred. Inputs flag the Maven `.m2` failure mode where a cached copy is
  used long after the registry copy diverged.

TODO(open): exact cache invalidation rules and how local
[overrides](#local-dependency-overrides) interact with the lock file's
checksum requirement. Lean: an override skips checksum verification but emits
a loud warning on every build that uses it.

## Local dependency overrides

A developer often needs to point a dependency at a **local checkout** — to
test a fix in a library alongside the project that uses it, to consume a
snapshot build, or to work offline. Tel supports this through a **separate,
gitignored override file**:

- The committed manifest lists the *public* dependency (`name`, `version
  range`).
- A local override file (committed to `.gitignore` by convention) can redirect
  a dependency to a local directory or a pre-built snapshot.
- The IDE and build tool **flag overrides visibly** in every build that uses
  one — overrides are easy to forget about, and a silent override is a debug
  trap.

This replaces the Maven `<relativePath>` pattern that forced everyone on the
team into the same directory layout. The override is a *local* developer
choice; the project layout is not.

TODO(open): the override file's exact shape and location, and whether
overrides may stack (file → workspace → user-global). Lean: a single override
file per project; user-global overrides only via an environment variable, not
a hidden home-directory file (which leads to invisible "works on my machine"
problems).

## Bugs the version discipline prevents

A representative subset of catalogue cases
that drive the strict-resolution and
single-source-of-version stance:

- **"`<dependencyManagement>` exclusion silently won during a repo merge."**
  Two repos merged; the parent POM's `dependencyManagement` took precedence
  over a child's; an exclusion that wasn't relevant before suppressed a
  transitive dep that *was* relevant. A day was lost. Tel's lock-file-as-
  effective-truth, plus computed-transitive-capability grants, makes the
  effective resolution legible to reviewers — not a chase through priority
  rules between layered manifests.
- **"`<dependencyManagement>` entry unused in goat but used by frog via
  scope=include."** Cross-project sharing through "include this manifest
  too" rules is exactly the surface where unintended overrides hide. Tel
  separates the *distribution unit* from the *dependency unit* (see
  [`04-packages.md`](04-packages.md)) so cross-crate version pinning is
  explicit at the workspace level rather than transitive POM osmosis.
- **"Gerrit pre-merge dep check didn't run for years."** A required CI
  step depended on a profile being active, which it wasn't. Tel's
  toolchain has a single declared build per project (see
  [`../18-tooling/03-build-system.md`](../18-tooling/03-build-system.md));
  there is no profile system whose subtle activation rules can hide
  whether a check ran.
- **"Local build succeeded after pulling latest, but the fix had been
  merged after the broken re-run started."** A re-run + local update
  ordered such that the dev was investigating a problem that no longer
  existed in mainline. The lock-file pin per build (and the discipline
  that the build hash identifies the resolved state) makes "is the
  failure reproducible against this exact graph?" an answerable question.
- **"Removed `public` method silently broke a downstream repo."** A
  method that "looked unused" was deleted; an unknown downstream consumer
  failed. The
  [generated public-API summary](#generated-api-summary), reviewed in
  diffs, makes public-surface removals visible *in the change* that
  causes them.
- **"Removed a Caribou field that an old test still used; bump broke."**
  Same shape, with a time delay: the deletion was tested at deletion
  time, but a new usage appeared later. The summary file + the per-method
  version index would surface the latent usage; the no-builders-for-
  generated-classes recommendation removes the runtime-only failure
  mode where a builder still accepts the call.
- **"Goat bump broke because a transitive dep wasn't pinned and shifted
  under us."** The lock-file rule is the structural answer.
- **"Single-thread bottleneck at compile because every module pulled in
  `util`."** A monorepo's util module dragged in all transitive deps.
  Tel's [non-transitive dependencies](04-packages.md#transitive-dependencies-opt-in-only)
  and unpublished workspace members let utility helpers be split into
  separate crates instead of living in a single bottom-of-graph module.
- **"Shaded dependencies caused identity bugs in metrics singletons."**
  Shading two versions of a metrics library produced two
  identity-distinct singletons; metrics were partial. Tel rejects
  shading (see [`04-packages.md`](04-packages.md)); the
  Candidate B "two versions, distinct types" approach surfaces the
  distinction in the type system rather than hiding it behind a
  rewritten class name.

## See also

- [Crates](04-packages.md) — what is being versioned, and per-dependency
  capability grants (which also fail the build when a dependency's needs
  change).
- [Visibility](03-visibility.md) — explicit public signatures as a stability
  tool.
- [Priorities and Trade-offs](../02-philosophy/01-priorities.md) — the
  stability and safety priorities behind these calls.
