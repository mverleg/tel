# Dependency Graph and Locking

TODO: review

A project's *resolved* dependency graph — every transitive crate at its
chosen version (one version per [connected component](06-versioning.md#decided-one-version-per-connected-component))
— is recorded in a lockfile next to the manifest. The lockfile is what the build
actually consumes; the manifest only states constraints.

## What

- **Manifest** says "I need `parser` at `^1.2`" (a constraint).
- **Lockfile** records "the resolved graph picked `parser 1.2.7`,
  `lexer 0.4.0`, …", each pinned to a **content hash** (the resolved version's
  immutable tree hash from the [registry](09-package-registry.md)).
- A fresh checkout with a lockfile builds the same bytes everyone else builds.
- A fresh checkout *without* a lockfile re-resolves; the result should match
  unless the registry has been pruned.

Because each entry pins a content hash of *immutable* bytes, a locked
dependency is a fixed input to the [content-addressed incremental
cache](../18-tooling/01-compiler.md#two-compile-modes): its compiled
answers key on that hash with no project-local ingredient, so they are reusable
across every project on a machine (and warm CI starts), and can never go stale.
This is the incremental-compilation payoff of pinning content hashes rather than
bare version numbers.

## Why

Reproducibility. The same script, the same dependencies, the same result on
any host — see [Priorities and Trade-offs](../02-philosophy/01-priorities.md).
Without a lockfile, transitive version drift can silently change script
behaviour, which is exactly what Tel's stability commitment is built to
prevent.

## How it interacts with versioning

See [Versioning](06-versioning.md) for the version-compatibility model. The
resolver enforces those rules; the lockfile records the resolver's choice.

## Visualising the graph and its diffs

The resolved graph is **structured data**, so tools render it rather than
re-deriving it from source. Two consumers matter:

- **The graph itself.** Every edge — crate → crate, and within a project
  module → module and symbol → symbol — is exact, because all cross-module
  references go through explicit [imports](02-imports.md) and there is no
  `eval` or reflective dispatch (see
  [coupling-visible by design](../18-tooling/09-editor-integration.md#coupling-visible-by-design)).
  A `tel graph` util emits the edge set with a minimal rendering; Tel does not
  ship an opinionated architecture-diagram style — that is left to external
  tools, the same call the
  [documentation generator](../20-appendix/06-deferred-features.md#what-the-generator-does-not-do)
  makes.
- **The graph *delta* of a change.** Given a code diff, the same machinery
  computes how the edge set changed: dependencies introduced or removed, a
  cycle created, a layering rule
  ([linter](../18-tooling/07-linter.md#architectural--boundary-rules)) newly
  violated. A new edge from `core` into `experimental`, or a fresh crate
  dependency, is the kind of change a diff makes easy to miss in the source
  but obvious in the graph.

Because the delta is cheap to compute and high-signal, it is meant to be
**attached to code review automatically**: a reviewer — human or AI — sees
*"this change adds a dependency on `network` and creates a cycle between
`billing` and `accounts`"* beside the diff, without asking. This is the
dependency-graph analogue of the
[review invariants](../18-tooling/07-linter.md#review-invariants--unprovable-re-checked-on-change)
that re-surface on change: structural facts a review should not have to hunt
for.

`TODO(open): whether the renderer is a `tel graph` subcommand, an LSP code
lens, or an external consumer of an emitted data file (lean: emit structured
data plus a minimal rendering, leave rich diagrams to tools). Also where the
"post the delta to the review" integration lives — a CI step, a VCS-host bot,
or both — and how it reuses the layering rules the linter already checks.`

## Open questions

- **Decided: one version per connected component.** A dependency graph resolves
  to **exactly one version of each crate within a connected component** — *not*
  npm-style "every requirer gets its own copy," and *not* a single global version
  forced across unrelated subtrees. Disconnected parts of the graph may take
  disjoint versions, whose types are then distinct (`T@1` ≠ `T@2`). This is
  required for **trait coherence**: TIP-0005's orphan rule assumes one resolved
  `(trait, type)` impl wherever a type is visible, and version-scoped type
  identity guarantees exactly that within each component. A real diamond with
  incompatible ranges is a hard resolution error the user fixes. Full rationale
  and the cross-version type rule are in
  [`06-versioning.md`](06-versioning.md#decided-one-version-per-connected-component).
- TODO(open): resolver algorithm — MVS-style minimum-version, pubgrub-style, or
  custom. The *objective* is decided (minimise the number of distinct versions;
  hard constraint: one version per connected component; types identified by
  `(crate, version)` — see
  [`06-versioning.md`](06-versioning.md#how-types-diverge-and-how-versions-are-chosen));
  the algorithm that realises it is open.
- **Decided: the lockfile is a Tel-data file** (see
  [`std.tel_ast`](../17-standard-library/18-tel-as-data.md)), not a bespoke
  format — the same "configuration is just Tel data" stance the manifest takes,
  so the resolver reads and writes it with the standard data machinery and tools
  can diff it structurally. Each entry pins at least `(name, resolved version,
  content hash)`; `TODO(open):` whether it also records the resolved *capability*
  set per dependency (a capability bump is semver-meaningful — see below) and the
  resolver inputs needed to prove a re-resolution would reproduce the same graph.
- TODO(open): handling of *capability* changes between versions — bumping the
  capabilities a dependency requests is a semver-meaningful change even if no
  type signature shifts.
- TODO(open): vendoring — is `tel vendor` part of the toolchain, or is this
  the host's problem?

## See also

- [Crates](04-packages.md) — the unit being resolved.
- [Package Manifest](07-package-manifest.md) — where constraints live.
- [Versioning](06-versioning.md) — the rules the resolver applies.
- [Package Registry](09-package-registry.md) — where the named crates live.
