# Project Layout

<!-- TODO: review -->

This page describes how a multi-file Tel project is arranged on disk. It is the
concrete counterpart to [Modules](01-modules.md): modules *are* directories, so
the project layout *is* the module structure.

A single-file script has no layout to speak of — one `.tel` file is a complete
program. Everything below is opt-in, project-scale machinery.

## Directories are modules

The central rule: **module structure mirrors the directory layout.** There is
no module-declaration syntax. A folder is a [module](01-modules.md); the `.tel`
files directly in it contribute to that module; a sub-folder is a sub-module
whose path extends its parent's.

```text
pricing/                 module  pricing
  rules.tel              part of pricing
  rounding.tel           part of pricing
  fx/                    module  pricing.fx  (sub-module of pricing)
    rates.tel            part of pricing.fx
    spread.tel           part of pricing.fx
```

A reader who sees the name `pricing.fx.rates` knows the file is
`pricing/fx/rates.tel`. The mapping is one-to-one and needs no index.

Because [super-modules can see their sub-modules](01-modules.md#super-modules-see-sub-modules),
`pricing` can use `pricing.fx` without an import; `pricing.fx` reaches back out
with a normal [import](02-imports.md).

## Packaging shape

A [crate](04-packages.md) is exactly this directory tree, zipped. The layout
that a developer edits is the layout that ships — there is no separate build
output structure to learn.

TODO(open): the exact conventions — manifest filename and location, where the
root module sits relative to the manifest, naming of script vs module files —
are not pinned down. See the open questions in [Crates](04-packages.md).

## Manifest: one file, two scopes

Tel uses **one manifest file format** at every scope where one is needed —
**crate** and **workspace**. (There is no third *sub-project* scope; that
level was [dropped](01-modules.md#encapsulation-without-publishing-an-unpublished-crate)
— an internal encapsulation boundary is just an unpublished crate.) The
crate scope uses a subset of the keys the workspace scope accepts; nothing has
to learn two formats.

The working filename is `mod.tel`. The format itself is a **declarative,
restricted notation**, not a general scripting form:

- Declarative, never Turing-complete (a manifest is read, not executed).
- A small, fixed set of keys with clear conventions, in the
  [Maven/Bazel](https://bazel.build/basics) spirit rather than the Gradle
  spirit.
- Comments allowed.
- Lean: a **subset of Tel's own surface syntax** — string keys, lists, simple
  values — so a reader does not learn a second language and the compiler can
  reuse its own parser. The subset must be small enough that it could not
  drift into a turing-complete dialect by accident.

```text
# Illustrative — syntax not pinned down.
crate "pricing" {
    version = "1.4.2"
    capabilities = []
    dependency "datetime"  { version = "3.3" }
    dependency "csv-tools" { version = "1.4" }
}
```

TODO(open): commit on filename (`mod.tel` vs something else) and confirm the
"subset of Tel" framing. The risk of using a Tel subset is that the manifest
*feels* programmable when it is not; mitigate by validating the subset
strictly at parse time.

## Workspace layout

A **workspace** is a directory that owns several [crates](04-packages.md),
some of which may be [unpublished](10-workspaces.md#dev-only-members). Its
manifest sits at the workspace root and may:

- Declare **shared dependency versions** that member crates reference by name
  rather than restating (one version of `regex` across every parser crate).
- List which sub-directories are workspace members, and which are published.
- Carry the shared **capability grants** for own-code dependencies.

```text
my-project/
  mod.tel                # workspace manifest
  override.tel           # local-only overrides (gitignored)
  crates/
    parser/
      mod.tel            # published-package manifest
      src/...
    runtime/
      mod.tel
      src/...
  internal/
    fixtures/
      mod.tel            # an unpublished crate (dev-only member)
      src/...
```

TODO(open): exact directory naming (`crates/`, `internal/`, …) is illustrative,
not prescribed. Decide whether the layout is fixed by convention or controlled
by manifest entries. Lean: convention with a manifest override for the rare
project that needs it.

## Local override file

The committed manifest only describes the public dependency graph. Developer
machines often need to point a dependency at a local checkout or a snapshot
build — see [Versioning: local dependency
overrides](06-versioning.md#local-dependency-overrides).

A separate **override file** (working name `override.tel`, gitignored by
convention) carries those redirections. The IDE and build tool must flag the
presence of an active override visibly on every build, so an override is never
silently in effect.

TODO(open): align the override filename with the manifest filename above; pick
the pair together.

## Scripts vs module files

TODO(open): inputs raise using a **different file extension** for *scripts*
(allowed to have top-level executable logic) versus *module files*
(declarations only). This connects to the unresolved question of
[what importing a file with top-level logic does](01-modules.md#top-level-logic-in-an-imported-file).
Lean: a clear split — script files are entry points and may run logic; module
files are declarations-only and safe to import. Decide whether that split is by
extension, by directory convention, or by a marker. Philosophy does not yet
cover this.

## Generated and codegen output

When part of a project is produced by code generation (for example, data
classes generated from a schema — see
[metaprogramming](../15-metaprogramming/03-derive-and-attributes.md)), the
generated files should live in their own clearly-marked location, and a
regeneration should **remove output that the current input no longer
produces**. Inputs note a real failure mode: a renamed generated artifact left
its stale predecessor behind, so two implementations were discovered at once.

TODO(open): where generated modules sit in the tree, and whether the compiler
or a separate tool owns cleanup of stale output. This overlaps with
[derive and attributes](../15-metaprogramming/03-derive-and-attributes.md);
keep the layout decision here once settled.

## See also

- [Modules](01-modules.md) — the semantics behind the directory mapping.
- [Imports](02-imports.md) — how paths across the tree are referenced.
- [Crates](04-packages.md) — zipping the layout for distribution.
