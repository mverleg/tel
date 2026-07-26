# Modules

<!-- TODO: review -->

A **module** is a named unit of Tel code. Modules group related types and
functions, give them stable names, and form the boundary at which an *API* is
described — for libraries, and especially for the host/script boundary.

Modules are an [opt-in, project-scale feature](../02-philosophy/01-priorities.md):
a 30-line modding hook never has to think about them. They appear only when a
script grows into several files, or when code is packaged for reuse.

## What a module is

- **A module is a directory of Tel files.** Module structure mirrors the
  directory layout — there is no separate module-declaration syntax that can
  drift from where the files actually live. A folder is a module; the files in
  it contribute to that module; a sub-folder is a sub-module.
- **Everything top-level in a module is a `const` binding.** A function is just
  a value assigned to a name (see [the tour](../01-overview/04-tour.md)); a type is a
  name bound to a type. Because every top-level binding is `const`, importing a
  module can never observe a mutating value, and there is no import-order
  hazard. This is *why* top-level bindings are safe to import.
- **A module exposes an API.** The API is the set of names a module makes
  visible to importers — public functions, public types (including
  [opaque types](#module-level-apis)), and the capabilities/effects those
  functions require. See [Visibility](03-visibility.md).

```tel
# File layout drives module structure:
#
#   pricing/            <- module `pricing`
#     rules.tel         <- contributes to `pricing`
#     rounding.tel      <- contributes to `pricing`
#     fx/               <- sub-module `pricing.fx`
#       rates.tel       <- contributes to `pricing.fx`
```

## Why directory-mirrored structure

Tel deliberately does not let a file declare an arbitrary module path
(`module foo.bar.baz` at the top of a file living anywhere). The reasons:

- **One obvious mapping.** A reader who sees `pricing.fx.rates` knows exactly
  which file to open. There is no indirection to chase.
- **No drift.** A declared path can disagree with the filesystem; a derived
  path cannot.
- **Faster, simpler tooling.** Resolving a name is a directory walk, not a
  whole-project scan for module declarations.

This follows *one good way over many clever ones* and *readability over
writability*.

## Modules have parents; the visibility flows both ways

Unlike [crates and workspaces, which have no parents](04-packages.md#crates-have-no-parents),
**modules do have parent modules**, and the nesting is meaningful in *both*
directions:

- A super-module can refer to its sub-modules without importing them.
- **A sub-module can see items from its parents** too — a child reaches its
  ancestors' items directly. This is the same reach that the
  [`private` level](03-visibility.md#what-visibility-is) describes from the other
  side ("visible to a module and its children").

So the natural shape — a parent module that wires together a handful of child
modules, and children that lean on shared parent definitions — works without
ceremony in either direction. (This is more permissive than Rust, where the
child→parent direction needs an explicit path.)

TODO(open): confirm whether child→parent visibility includes the parent's
`private` items or only its crate-visible ones. Lean: a child sees its parent's
`private` items (that is exactly what "private to a module *and its children*"
means), but not a *sibling's* privates.

## Module-level APIs

The unit at which Tel describes "what is exposed" is the **module**, not the
individual type or function. A module's API is:

- its public functions, with their signatures;
- its public types — which may be **opaque**: the importer sees the type name
  and can pass values around, but cannot see or depend on its internals. The
  defining module keeps full local detail without boxing or type-erasing it.
  This is the clean answer to the Rust pattern of wrapping everything in
  `Box[dyn …]` just to hide a concrete type.
- the **capabilities / effects** its public functions require (filesystem,
  network, clock, randomness — see [embedding Tel in a host](../16-ffi-and-interop/04-embedding-tel-in-a-host.md)).
  TODO(open): there is no dedicated capabilities/effects chapter yet; this and
  the references below point at the embedding chapter as the nearest home.
  Capabilities are part of the API surface because an importer must know what a
  module can do.

This is the description used at the **host/script boundary**. A host states the
API it expects a script to satisfy (inputs, required result type, allowed
capabilities, host functions it may call); the Tel compiler checks a submitted
script against that API and rejects it otherwise.

A module is also the home for its own **business goal**: the module's `##` doc
comment states *what this unit is for and why*, which is the right scope for a
human-language purpose that no tool can check. See
[where goals and invariants live](../20-appendix/06-deferred-features.md#business-goals-and-system-invariants).

TODO(open): can a module API be expressed purely with traits + function
signatures, or does it need a dedicated module-level API construct? Inputs
raise this and do not settle it.

TODO(open): the host/script boundary plausibly needs *two* module APIs — one
describing what the host expects of the script, and one describing what the
script may call back on the host (an injected argument, possibly with a
Kotlin-style receiver, or an effect). Needs a home, possibly in
[embedding Tel in a host](../16-ffi-and-interop/04-embedding-tel-in-a-host.md). pre-pivot
note: re-justify the "expose to many languages" framing against embedding —
this part holds up well.

## Re-exports

By default a module's structure mirrors its directory layout, but a module may
**re-export** names from its sub-modules, so a parent can present a curated,
flat API without forcing importers to know the internal folder tree. The
**parent always decides** what its children expose upward — a child cannot
unilaterally promote itself.

A plain re-export adds an additional public path to the same item: if
`pricing.fx.rates.Rate` is re-exported by `pricing`, importers may write either
`pricing.fx.rates.Rate` or `pricing.Rate`, and both names refer to the **same
type** (aliases for one item, not two distinct types).

The [`export` block](03-visibility.md#crate-export-block) goes one step
further at the publication boundary: it may **re-home** an item, binding it to a
public path that differs from its internal one, so the crate's public API need
not mirror the code layout. This is the [one sanctioned place renaming is
allowed](02-imports.md#renaming-only-at-the-export-boundary); it is what lets
code move internally without breaking consumers.

TODO(open): when the same item is reachable under two paths (e.g. for backwards
compatibility — `old.Foo` and `new.Foo` for one type), confirm the two names
are *exactly the same type*, not nominally distinct aliases. Treating them as
the same type is the only consistent reading given
Tel's [versioning](06-versioning.md) story; cf. the related Rust discussion.

TODO(open): re-export *granularity* — can a parent re-export a whole child
module ("expose all of `pricing.fx` under `pricing`"), only specific items, or
both? Lean: both, with item-level being the precise tool and module-level
being a shorthand. Philosophy does not yet cover this.

## Encapsulation without publishing: an unpublished crate

Earlier drafts proposed a distinct fourth level — a *sub-project* — between a
module and a published crate, for code that wants its own private boundary and
dependency edges without a registry name.
[TIP-0003](../tips/0003-module-levels-and-dependency-direction.md#why-the-sub-project-level-is-dropped)
**drops that level.** Tel has exactly three: **module**, **crate**, and
**workspace**. The sub-project's two jobs are re-homed:

- **Its dependency edge** moves to the **crate** — the crate is where
  dependencies apply.
- **Encapsulation without release** becomes an **unpublished
  [workspace](10-workspaces.md) member**: an ordinary crate the workspace
  builds and depends on internally but never publishes.

This still answers the Rust frustration that motivated a sub-project — workspace
members were used purely to *get* limited visibility without intent to publish.
In Tel that is just *a crate you do not publish*, with no new concept to learn:
the [crate-visible default](03-visibility.md#what-visibility-is) already draws
the encapsulation boundary, and leaving it out of the workspace's published set
keeps it private to the project.

## Compilation units are not user-visible

Tel deliberately does **not** define a "compilation unit" the way Java defines
a `.class` file or Rust defines a crate-as-compilation-unit. The compiler is
free to choose its own units — per-file, per-module, per-crate, or whole
project — based on what is fast. Source code never names a compilation unit,
and observable behaviour does not depend on the choice.

The constraint runs the other way: the language should be designed so that
**local-only knowledge is enough to type-check a module**, which keeps compile
times manageable as projects grow. This is a design pressure on later type
chapters, not a user-facing concept here.

TODO(open): re-justify the local-only-compilation design pressure against
embedding — for small scripts compile speed is already a non-issue, but for the
larger Tel projects the opt-in module system targets it matters. Philosophy
does not yet state this. *(pre-pivot — verify against embedding focus.)*

## Own code vs external code

Tel cleanly separates **own code** (this project's modules) from **external
code** (modules pulled in via [crates](04-packages.md)). The split shows up
in several places later in the chapter:

- Compile-time options (warnings, lint levels, strictness) apply to own code
  by default and not to external code.
- A project may contain more than one unit of "own code" — several
  [crates](04-packages.md) in a [workspace](10-workspaces.md), including
  unpublished ones — and they share that own-code treatment.
- Dependency [versioning](06-versioning.md) and capability grants only apply
  to external code; an in-tree own-code dependency edge needs no version (see
  [Versioning: no in-tree version](06-versioning.md#version-bumping-discipline)).

TODO(open): pin down whether *all* own-code crates in a workspace always share
one view of external dependencies, or whether a member may pull in its own.
Inputs raise both as desirable; the friction with "specify dependency versions
only once" (see [Versioning](06-versioning.md)) needs resolving. Lean: the
workspace supplies a shared default, a member may override
([`10-workspaces.md`](10-workspaces.md#declaring-version-bounds-once)).

## Top-level logic in an imported file

A file can contain top-level statements that *do work* (not just declarations).
What happens when such a file is imported is **unresolved**:

TODO(open): importing a file with top-level logic — fail, run it once, silently
ignore the logic, or wrap it as an implicit `main`? An option raised in inputs
is a different file extension for *scripts* (which may have top-level logic) vs
*module files* (declarations only). The embedding priority says scripting is
the common case and large module projects are rare, so the script form must
stay first-class. Lean: a module file is declarations-only; running logic at
import time is a script concern, not a module concern. Philosophy does not yet
cover this — flag as a philosophy gap.

## See also

- [Imports](02-imports.md) — how one module refers to another.
- [Visibility](03-visibility.md) — what `pub` means, and whether it is needed.
- [Project Layout](05-project-layout.md) — the directory conventions in full.
- [Crates](04-packages.md) — distributing modules between projects.
