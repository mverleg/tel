# Workspaces

<!-- TODO: review -->

A **workspace** is a grouping of related [crates](04-packages.md) — some
published, some [unpublished](#dev-only-members) — that are developed together.
It is a *development-time* construct: a workspace is **never published and is
invisible to the package index**. The unit that ships and that other projects
depend on is always a [crate](04-packages.md) — the workspace is the
scaffolding around several of them. All workspace properties (shared versions,
namespace, build mode) **propagate into the member crates**; nothing about the
workspace itself crosses the index.

Like the rest of [chapter 11](01-modules.md), a workspace is an opt-in,
project-scale feature. A single embedded script has no workspace; a single
published library does not need one either. Workspaces appear when one
repository builds **several crates at once** — a parser, a runtime, and a CLI
that share code and a release cadence.

> **Terminology note.** Some older notes call the published unit a "module" and
> reserve "workspace" for the development grouping. In this book the published,
> dependency-bearing unit is the **[crate](04-packages.md)**; *module* is the
> [namespace/directory unit](01-modules.md). A workspace groups crates.

## Crate vs workspace

The two concepts answer different questions:

| | **Crate** | **Workspace** |
|---|---|---|
| Unit of | publish, dependency, versioning | development, shared configuration |
| Visible to the index? | yes | **no** |
| Has a version? | yes | no (members do) |
| Default build mode | production | `--dev` (see below) |

A workspace holds one or more crates, including any
[unpublished, dev-only ones](#dev-only-members). Its on-disk shape and its
single shared [manifest](07-package-manifest.md) format are described in
[Project Layout](05-project-layout.md#workspace-layout); this page is about what
a workspace *means* and the conveniences it provides.

## "Current" versions for intra-workspace dependencies

Crates inside one workspace usually depend on each other and are released
together. Restating the same version number on every internal edge is noise
that drifts out of date. A workspace therefore offers a **`$current`
placeholder** (the Maven `${revision}` idea): an intra-workspace dependency may
ask for the *current* version rather than a literal one, and every member
resolves to the version the workspace is building.

```text
# Illustrative — syntax not pinned down.
# Inside crate `parser`, which lives in the same workspace as `tel_log`:
dependency "tel_log" { version = "$current" }
```

On [publish](../18-tooling/04-package-manager.md), `$current` is **substituted
with the concrete version** so the released crate carries an ordinary, pinned
dependency — a consumer outside the workspace never sees `$current`.

## Declaring version bounds once

A workspace can also set the version of a *shared external* dependency **once**,
so member crates do not each restate it. A member may depend on a crate and
**omit the version**; the workspace supplies the bound, and on publish the
concrete constraint is substituted into the released manifest.

```text
# Workspace manifest sets the shared bound:
shared_dependency "regex" { version = "^2.0" }

# A member crate omits the version and inherits the workspace default:
dependency "regex" {}

# ...or restates a version to override the default for this member only:
dependency "regex" { version = "^1.9" }
```

The workspace bound is a **default, not a mandate**: a member that omits the
version inherits it, and a member that states a version **overrides it** for
itself. The shared bound exists to remove repetition in the common case where
every member agrees, not to forbid a member from diverging when it genuinely
needs to. This is the same impulse as [one resolved version per connected
component](06-versioning.md#decided-one-version-per-connected-component), pushed
up to authoring time: declare the intended version in one place, let every
member agree by construction rather than by review — but let a member opt out.

TODO(open): what happens when a member is published *standalone* later and loses
the workspace that supplied its bounds — publish-time substitution must bake the
inherited version into the released manifest so it is self-contained. Cross-ref
the declare-once discussion in
[Crates](04-packages.md#the-crate-is-both-the-distribution-unit-and-the-dependency-edge).

## Listing members and paths

A workspace **must list its members**, and **may dictate their paths**. The
directory layout and a crate's name *may* match but are not forced to — a
crate is not opened in any particular directory combination in an IDE, so its
relative on-disk location should not be load-bearing.

- **Nested form** mirrors directories and yields a hierarchical (dotted) name.
  `user { auth { google } }` declares a member named `user.auth.google` living
  at `user/auth/google/`.
- **Explicit-path form** gives a member a path directly, so the name need not
  match the directory at all — useful for vendored or relocated members.
- When the workspace dictates neither, a **default directory mapping** applies.

```text
# Illustrative — syntax not pinned down.
members {
    user { auth { google } }          # -> user.auth.google at user/auth/google/
    pricing.fx = "src/pricing/fx"     # explicit path; name need not match dir
}
```

The resulting dotted names are [purely lexical](04-packages.md#crates-have-no-parents):
`user.auth.google` is not a child of any `user.auth` crate and inherits
nothing — workspaces carry no inheritance.

TODO(open): exact member-list syntax and the default mapping; see
[TIP-0003](../tips/0003-module-levels-and-dependency-direction.md#crate-names-hierarchical-but-the-workspace-decides-the-mapping)
and [Project Layout](05-project-layout.md).

## Namespaces

A **namespace** is a naming prefix shared by a group of crates — the
identifier-hygiene axis from
[TIP-0003](../tips/0003-module-levels-and-dependency-direction.md#namespaces),
kept separate from both the hierarchy and the distribution unit. It lets a
workspace of hundreds of crates occupy a handful of top-level identifiers.

- **Where it is set.** A namespace may be declared **on the workspace**, **on a
  crate**, or **both**. If both declare one, **the crate wins** — the same
  precedence as a [version bound](#declaring-version-bounds-once), so the two
  override rules are learned once.
- **Atomic publish is namespace-scoped.** A workspace can publish several
  members **atomically only when they share a namespace**. Members in different
  namespaces are released independently — atomicity is a property of a namespace,
  not of the workspace as a whole.
- **Namespaces do not nest, and crates within one are leaves.** A namespace is
  a single flat prefix — there is no `acme.pricing` *under* `acme`. Within a
  namespace, crate names form a [prefix-antichain](02-imports.md#how-a-dotted-root-stays-unambiguous):
  you cannot have both `a.b` and `a.b.c`. A fully-qualified name is
  `namespace.crate.module` (see [Imports](02-imports.md#the-fully-qualified-shape-namespace--crate--module--member)).

```text
# Illustrative — syntax not pinned down.
# Workspace sets a default namespace; a member may override it.
workspace { namespace = "acme" }

crate "pricing.fx" { }                       # namespace "acme" -> acme.pricing.fx
crate "telemetry"  { namespace = "widgets" } # overrides; publishes separately
```

## Dev mode is the workspace default

Tel has (at least) **two compile modes** — a **dev** mode tuned for the
inner-loop and a **production** mode tuned for release. What each mode actually
changes is specified elsewhere; here only the *default* matters:

- Building a **workspace** defaults to **dev** mode — you are in a workspace
  because you are *developing*.
- Building a crate **outside** a workspace defaults to **production** mode —
  a lone crate is almost always being *shipped*.

This is a default, not a constraint: either mode is reachable explicitly from
either context.

TODO(open): the precise list of what each mode changes, and whether there are
more than two, is specified elsewhere (not yet written). This page only fixes
the workspace/standalone default.

## Dev-only members

A workspace member need not be published. **Dev-only members** — test
harnesses, fixtures, internal tools, an unpublished helper crate — are built
and depended on *inside* the workspace but never reach the index. This is where
the dropped *sub-project* level's "encapsulate without releasing" job now lives:
an internal boundary is simply *a crate you do not publish* (see
[Modules](01-modules.md#encapsulation-without-publishing-an-unpublished-crate)).

One rule keeps dev-only members honest at release time:

- **An atomic release runs the project's checks with the dev-only members
  removed.** The release must mirror what an external consumer actually
  receives, so a check that secretly leaned on a dev-only member (a fixture
  crate providing a type a published crate quietly imported) **fails the
  release** rather than passing in-workspace and breaking for users. The
  dev-only members are present for *development*, absent for the *published*
  build's verification.

TODO(open): how a member is marked dev-only — a manifest flag on the member, a
list in the workspace manifest, or a directory convention. Lean: an explicit
`publish = false` (or equivalent) on the member, so the published set is read in
one place.

## Build hooks: the only extension point

A workspace may run a small, **fixed set of external commands** as part of its
build — a [linter](../18-tooling/07-linter.md), a
[formatter](../18-tooling/06-formatter.md), project-specific checks — at defined
steps. These are **workspace hooks**, and they are deliberately the *only*
plugin mechanism Tel offers.

Two rules keep this from becoming a build-script system:

- **Hooks never publish.** Like everything else about a workspace, hooks are
  development-time only. A consumer of a published crate never runs, sees, or
  is affected by the producing workspace's hooks.
- **Hooks are whole commands at fixed steps, not a programmatic API.** They
  cannot load user code into the compiler or inspect/rewrite the program being
  compiled. This is the same call made in
  [Build System](../18-tooling/03-build-system.md#what) and
  [Crates: crate kinds](04-packages.md#crate-kinds-no-plugin-kind-for-now):
  the toolchain is opinionated and kept simple by **not** allowing much
  customization. A richer compiler-plugin model is deferred until after the base
  language is ready — see
  [Deferred Features](../20-appendix/06-deferred-features.md#compiler-plugin-model-and-crate-distributed-lints).

## Declared build steps (deferred)

Older notes propose a workspace declaring a **list of build steps**, each a
separate Tel script run with **limited, explicitly-injected capabilities** —
useful for pre-computing lookup tables or generating code from a schema. This
overlaps with hooks above but is more ambitious (arbitrary Tel scripts, possibly
shell commands at publish time).

TODO(open): this **conflicts with the committed
[no build scripts](../02-philosophy/04-antifeatures.md) decision** and is
deferred. Two distinct shapes were raised and must not be conflated when it is
revisited:

- **Publish-time steps** — run once by the author before publishing, their
  *output* shipped as ordinary source ([static
  resources](04-packages.md#static-resources-crate-bundled-files), generated
  modules). Any shell command could in principle be used because the author runs
  it on their own machine. This is the safer shape and is close to "generate,
  then check in" — which the build system already says is the host's job.
- **Build-time / install-time steps** — published as hooks and run in the
  *consumer's* environment when they build. This is the dangerous shape: it
  reintroduces install-time code execution (`build.rs`, npm `postinstall`), the
  exact supply-chain hole the
  [capability model](04-packages.md#per-dependency-capability-declarations) is
  built to close. If it ever lands it must be capability-gated and almost
  certainly publish-time-only. Lean: keep generation at the host/author level;
  do not ship runnable build steps to consumers.

## Distributing Tel-based command-line utilities

A workspace is also how a team builds and ships an *internal tool* written in
Tel. Installing such a tool as a runnable binary on a developer's `PATH` is a
[package-manager](../18-tooling/04-package-manager.md#installing-binary-utilities)
concern, not a workspace one — but it is the natural endpoint of "we built a
useful util in this workspace, now put it where people can run it."

## See also

- [Modules](01-modules.md) — and the
  [unpublished-crate](01-modules.md#encapsulation-without-publishing-an-unpublished-crate)
  encapsulation a workspace can also contain.
- [Crates](04-packages.md) — the published unit a workspace groups; the
  [crate-is-both-distribution-and-dependency-edge](04-packages.md#the-crate-is-both-the-distribution-unit-and-the-dependency-edge)
  point that motivates workspaces.
- [Project Layout](05-project-layout.md#workspace-layout) — the on-disk shape
  and the shared manifest format.
- [Versioning](06-versioning.md) — what `$current` and shared bounds resolve to.
- [Package Manager](../18-tooling/04-package-manager.md) — publish-time
  substitution and installing Tel-based binaries.
- [Build System](../18-tooling/03-build-system.md) — why the build is
  opinionated and hooks are the only extension point.
