# Crates

<!-- TODO: review -->

A **crate** is a unit of reusable Tel code that can be distributed between Tel
*projects*. Crates are how a Tel library reaches another Tel codebase — they
are **not** how a host ships its own resources to a script.

Like the rest of [chapter 11](01-modules.md), packaging is an opt-in,
project-scale feature. A single embedded script has no crate, no dependencies,
and no manifest.

## What a crate is

- A crate bundles one or more [modules](01-modules.md).
- A crate has a **name**, which becomes the root segment of every import path
  into it: a crate `X` is imported as `X.path` (see
  [Imports](02-imports.md#importing-published-modules)).
- A crate may declare **dependencies** on other crates, and the
  capabilities each dependency is permitted to use (see below).

## Packaging format: a zip of a directory

Tel deliberately keeps distribution boring. A crate is **a directory tree
with naming conventions, zipped** — no bespoke archive format, no build
artifact step.

The aim is that the Tel compiler can consume a zip file directly: packaging is
"zip up a directory" and using a crate is "point the compiler at the zip."
This is the [pex](https://github.com/pantsbuild/pex)-style convenience, with one
difference — a host embeds the Tel runtime, so the end user does not need a
separate Tel installation.

Crates **distribute source, not compiled binaries** — this is settled. It
keeps crates readable, serves the
[one-script-many-hosts](../01-overview/02-when-to-use-tel.md) story, and is part
of the minimal supply-chain baseline: a consumer can always read what they run.
A compiled/IR form would simplify host implementations but loses readability, so
it is not the distribution format.

TODO(open): exact directory conventions inside the zip (manifest filename,
where modules sit). The source-only decision above settles *what* ships; the
on-disk layout is still to be documented.

## Static resources (crate-bundled files)

A crate may ship **static data files** alongside its modules — Unicode tables,
locale/currency data, parser tables, a public-suffix list, templates, schemas.
These are what Java ships as *resources*. In Tel they are part of the crate's
source tree, so they are covered by the
[tree hash](09-package-registry.md#crate-identity-the-tree-hash), are immutable,
and ship as source like everything else.

### A dedicated read-only resource capability

Reading a bundled file is **not** the
[`filesystem` capability](#per-dependency-capability-declarations). The
filesystem capability reaches the *host's* ambient filesystem and must be granted
explicitly. A crate's own bundled resources can reach **nothing outside the
crate**, so they get their own, much weaker capability:

- **Read-only.** Resources cannot be written or modified at runtime.
- **Scoped to the crate's own files.** A resource read can only name a path
  *inside the crate that declared it* — no traversal out to the host
  filesystem, another crate, or the network.
- **Fully trusted, always available.** Because it cannot reach anything outside
  the crate, granting it carries no supply-chain risk: it is not a capability a
  consumer must review or can meaningfully deny. It is the one I/O-shaped power
  that is safe by construction, so it sits in the capability taxonomy as the
  member that is always granted.
- **Platform-independent.** Every host can implement it one way or another; in
  the worst case the build **embeds each resource as a byte array** compiled into
  the crate, so "reading a resource" never needs a real filesystem at all. A
  host that has a filesystem may instead map resources to files — the crate
  cannot tell the difference.

```tel
# Illustrative — exact API not pinned down.
let table: Bytes = resource.bytes("data/unicode-tables.bin")
let tmpl:  Text  = resource.text("templates/report.tmpl")
```

Because resources are embedded at compile time, a data-table dependency needs
**no capability grant from its consumer** — unlike ecosystems where shipped data
means runtime file reads. This is why static data is a clean fit for Tel's
[no-ambient-I/O](../02-philosophy/04-antifeatures.md) model.

TODO(open): exact resource API and path grammar; whether resources are declared
in the manifest (an explicit list) or found by directory convention. Lean: an
explicit resource directory plus optional manifest globs, so the published file
set is predictable and the [size limit](#size-limits) is enforceable.

## Size limits

A published version is capped at **10 MB**, measured on the **uncompressed
canonicalised tree** (the same bytes the
[tree hash](09-package-registry.md#crate-identity-the-tree-hash) covers), with
a **warning at 7 MB** and the ability to **request a higher limit** for
legitimate cases. Two reasons: keep crates reasonably sized, and keep hosting
manageable.

The cap is on *uncompressed* size, not the zip, for three reasons: it is what the
tree hash already measures; it bounds decompression-bomb expansion by
construction (a compressed-only cap does not); and since stored bytes are always
≤ uncompressed, capping uncompressed also bounds hosting storage. A
compression-ratio guard rejects implausibly expanding archives as a second line.

For a **source-only** ecosystem 10 MB of text is millions of lines — pure code
never approaches it. The budget exists almost entirely for
[static resources](#static-resources-crate-bundled-files); the 7 MB warning
targets crates quietly accreting large bundled data. Test-only fixtures should
be **excluded from the published artifact** so they do not consume the budget.

TODO(open): the request-an-increase process and ceiling; the compression-ratio
guard threshold.

## The crate is both the distribution unit and the dependency edge

[TIP-0003](../tips/0003-module-levels-and-dependency-direction.md) collapses the
earlier *crate vs sub-project* split: **the crate is both what gets
published and where dependencies apply.** A crate is a name, a version, what
gets published, *and* the unit that owns a dependency edge — what is allowed to
import what, and which version of a shared dependency it expects. There is no
separate internal dependency-unit to learn.

Two recurring patterns still want describing, and both are expressed with
crates plus a [workspace](10-workspaces.md):

- **Wrapper crate.** A small crate re-exports a curated subset of a larger
  external library, so the rest of the project only sees the wrapper and never
  the underlying library directly (e.g. `file_endec` wraps `file_shred`, and the
  rest of the code uses `file_endec`).
- **Coordinated set.** A workspace ships **several crates** that share
  dependencies, so the workspace can guarantee they all use the same version of
  a common crate (e.g. one `regex` across several parser crates). Some of
  those members may be [unpublished](10-workspaces.md#dev-only-members) —
  internal encapsulation boundaries the outside world never sees, which is the
  job the dropped *sub-project* level used to do.

TODO(open): exactly which file owns what — does the workspace manifest own the
shared dependency list while each package manifest only references it, or do
crates restate their deps and the workspace enforces uniformity? Lean:
declare once in the workspace, reference by name from package manifests, a
member may override, fail the build on genuine conflicts. See also
[Workspaces](10-workspaces.md#declaring-version-bounds-once) and
[Project Layout](05-project-layout.md).

## Compile-time vs runtime dependencies

Maven-style separation of compile and runtime dependencies has a real use:
depending on an *interface* at compile time and providing an *implementation*
at runtime (so the implementation is not strongly linked, and can vary).

Tel's capability-based design covers part of this already — the *host* provides
the runtime implementation for any capability — but pure-Tel libraries can also
want it. The simplest case: a library defines a trait, downstream code
implements it, the consumer wires the implementation in at runtime.

TODO(open): does Tel need an explicit *compile-only* dependency kind in the
manifest, or is this fully covered by traits + dependency injection? A third
form raised in inputs is "per-run / swappable at runtime" — almost certainly
out of scope (it implies dynamic loading, which conflicts with
[no reflection / no eval](../02-philosophy/04-antifeatures.md)). Lean: traits
plus a *compile-only* dependency kind for the few cases traits alone cannot
express. Confirm.

## Crate kinds (no plugin kind for now)

A Tel crate is either a **library** (modules importable by other Tel code) or
an **executable** (an application with an entrypoint that nothing imports) —
this is the [`api`/`impl`/`executable` classification](#the-lightweight-apiimplexecutable-flag),
where library is just the non-`executable` case. There is deliberately **no
*compiler plugin* kind**: the toolchain is not extensible by loading third-party
crates into the compiler, and there is no API for crates to inspect or
rewrite the program being compiled.

This keeps the toolchain simple and predictable while the base language is
still being designed. The build is opinionated with minimal customization (see
[Build System](../18-tooling/03-build-system.md)); its only extension point is
that the build may invoke a small set of **fixed external commands** (a linter,
a formatter) at defined steps — not a programmatic compiler API, and not
arbitrary user code running inside the compiler. This is the same call as
[no proc-macros / no build scripts](../02-philosophy/04-antifeatures.md).

A compiler-plugin ecosystem (post-typecheck inspection, doc-generation hooks,
custom lints distributed as crates) is **deferred until after the base language
is ready** — see
[Deferred Features](../20-appendix/06-deferred-features.md#compiler-plugin-model-and-crate-distributed-lints).
If it ever lands, plugins would be distributed through the same package manager
and capability model as libraries, and a plugin's toolchain authority would have
to be declared per plugin and granted per project.

**Executable is a real kind** (decided — it is the top of the
[dependency classification](#the-lightweight-apiimplexecutable-flag)): the home
for Tel-built CLI tools and workspace utilities, with at least one entrypoint and
no importers. This does *not* contradict the embedding focus — when Tel runs as a
guest the **host** is still the executable and the script is an ordinary library
crate; the `executable` kind is for the separate case where a Tel crate is
itself the program entry point (see
[distributing CLI utilities](10-workspaces.md#distributing-tel-based-command-line-utilities)).

The crate is also the **scope of the orphan rule** for trait implementations:
"own the trait or own the type," and same-owner specialisation, are both
crate-level, so the *outward* guarantee — one resolved impl per (trait,
applied-type) in any program — is a property of the crate, not the module. See
[Traits — coherence](../10-data-modelling/03-traits-or-interfaces.md#coherence-the-orphan-rule-and-specialisation)
and [TIP-0005](../tips/0005-trait-coherence-and-the-orphan-rule.md).

## Crates have no parents

A crate can be **nested in a directory hierarchy**, but that nesting is
**lexical organisation only** — a crate inherits *nothing* from the
directories above it. Tel has no Maven-style parent crate: there is no parent
manifest to pull config, versions, or metadata from, and so none of the
"which layer wins?" ambiguity that
[layered POMs cause](06-versioning.md#bugs-the-version-discipline-prevents).
The two real jobs a parent POM does are re-homed:

- **shared config / dependency versions** → the [workspace](10-workspaces.md);
- **grouped naming** → the [namespace](10-workspaces.md#namespaces) axis.

A crate may carry a **hierarchical (dotted) name** like `user.auth.google`,
but the segments are *purely lexical* — `user.auth.google` neither requires nor
inherits from any `user` or `user.auth`. The directory layout and the name *may*
match but are not forced to: the **[workspace](10-workspaces.md#listing-members-and-paths)
decides the member-to-name mapping** (it must list its members and may dictate
their paths), so a crate's on-disk location is not load-bearing. See
[TIP-0003](../tips/0003-module-levels-and-dependency-direction.md#crate-names-hierarchical-but-the-workspace-decides-the-mapping).

`TODO(open):` the exact member-list syntax (a nested block that mirrors
directories vs explicit per-member paths) and the default mapping; tracked in
[Workspaces](10-workspaces.md) and [Project Layout](05-project-layout.md).

## Decoupling distribution from dependency edges

A consequence of the previous sections, worth stating directly: a project may
have **many published crates** but **one shared dependency view**, or a
**single published crate** built from many internal modules with **different
internal dependency boundaries**. Neither shape is awkward.

This serves the [opt-in priority](../02-philosophy/01-priorities.md) — a small
script ignores all of it; large projects reach for whichever combination
matches their shape.

## Transitive dependencies: opt-in only

A crate may import **only the dependencies it declares** in its manifest. A
transitive dependency that ends up in the resolved graph is **not** importable
just because it is present. This closes the *phantom-dependency* hole — npm's
hoisting let code `require` a crate it never declared, which then broke when
the resolved tree reshuffled and the crate was no longer reachable.

Transitivity is therefore **opt-in**. A crate can deliberately **re-export** a
dependency so its own consumers may name that dependency's types directly — the
wrapper pattern from
[The crate is both the distribution unit and the dependency edge](#the-crate-is-both-the-distribution-unit-and-the-dependency-edge).
Re-export is a conscious API decision, not the default. A consumer's reachable
import set is exactly *its own declared dependencies, plus whatever those
re-export* — computed mechanically and recorded in the lock file.

Two related rules live elsewhere:

- A re-exported type becomes part of the re-exporter's public API, so a
  [stable crate may not re-export an unstable one](#stable-depending-on-unstable).
- Because the reachable set is explicit and there is no `eval`, the
  [dependency-graph edges](08-dependency-graph-and-locking.md#visualising-the-graph-and-its-diffs)
  are exact.

### The lightweight `api`/`impl`/`executable` flag

Tel has a **deliberately tiny** dependency-direction feature, decided in
[TIP-0003](../tips/0003-module-levels-and-dependency-direction.md#dependency-direction-the-lightweight-apiimplexecutable-flag).
A crate classifies itself **`executable`**, **`api`**, **`impl`**, or leaves it
**unspecified**, and there are just **two rules**:

> 1. **`api` → `impl` is forbidden** — an API layer may not depend on an
>    implementation layer.
> 2. **Nothing may depend on an `executable`** — it is the top of the stack: it
>    imports anything, but no crate may import it.

An **`executable`** is the application/entrypoint crate: it **must expose at
least one entrypoint**, may depend on every other class, and is depended on by
nothing. Every other combination compiles, and **unspecified is the default that
costs nothing** — a script or early-stage crate writes no classification and
meets neither rule. The feature adds *no* new visibility or re-export machinery;
that already exists (opt-in re-export above,
[opaque types](01-modules.md#module-level-apis), the mandatory
[crate export block](03-visibility.md#crate-export-block)). Its job is the
thing convention and dependency injection cannot give: a **compile-time error**
when an API layer reaches into an implementation detail, or when anything tries
to import an application.

It also lets the [stable-on-unstable](#stable-depending-on-unstable) rule be
*selective*: an `api` dependency on an unstable crate is the hard-error case
(the unstable types would surface in a stable API), while an `impl` dependency
on the same crate is merely risky-but-allowed.

```tel
# Illustrative — syntax not pinned down.
crate "pricing-cli"  { kind = executable }  # has an entrypoint; nobody imports it
crate "pricing-api"  { kind = api }         # may not depend on any `impl` crate
crate "pricing-impl" { kind = impl }        # may depend on anything but executables
crate "pricing-util" { }                    # unspecified — no restriction
```

The classification lives on the **unit**: a crate declares what it *is*, and
the forbidden-edge rules follow from the two endpoints' classes — there is no
per-edge direction to restate.

TODO(open): only the **spelling** of the classification (the `kind = api` shown
above is illustrative) is unsettled.

## Per-dependency capability declarations

When a project takes on a dependency, it must **declare which capabilities that
dependency is allowed** — filesystem, network, other host syscalls. The
declaration lives with the dependency (in the project manifest, possibly in a
lock file).

This is a **supply-chain defence**:

- A dependency that quietly starts doing something new — a compromised release,
  or honest scope creep — needs capabilities it was not granted.
- Because the grant is explicit, that change **fails to compile**. The build
  breaks until a human reviews the new behaviour and widens the grant on
  purpose.
- A dependency can never acquire a capability it was not handed, malicious or
  not.

```tel
# Illustrative manifest fragment — syntax not pinned down.
dependency "csv-tools" {
    version = "1.4"
    capabilities = []          # pure data work; no I/O at all
}

dependency "http-fetch" {
    version = "2.0"
    capabilities = [network]   # network only; no filesystem, no clock
}
```

If `csv-tools` later ships a version that wants `filesystem`, the project no
longer compiles until someone adds `filesystem` to its grant — turning a silent
supply-chain risk into a visible code-review checkpoint.

This sits squarely on the [safety-over-flexibility](../02-philosophy/01-priorities.md)
priority and extends Tel's capability model (see
[embedding Tel in a host](../16-ffi-and-interop/04-embedding-tel-in-a-host.md)) from the
host/script boundary down to every dependency edge.

TODO(open): does the capability grant belong in the manifest, the lock file, or
both? The lock file pins exact resolved versions, so the *effective* grant is
naturally lock-file data; the manifest is where a human writes intent. Lean:
intent in the manifest, resolved/enforced grant in the lock file.

### Required, optional, and unused capabilities

A dependency declares each capability it touches in one of three modes:

- **Required.** The crate will not compile or run without it. The importer
  must grant the capability or pick a different crate.
- **Optional.** The crate can do its work without it, but does *more* with it
  — e.g. an HTTP client that caches to disk when given filesystem access. The
  importer chooses whether to grant it.
- **Unused.** The crate never asks for the capability and so does not list
  it.

An *optional* capability is observed inside the crate through a normal
`Option`-shaped value the host (or wrapper code) hands in — the typical
pattern is `if let Some(fs) = file_system_or_none() { ... }`. There is no
silent runtime probe and no "unwrap if maybe-present"; the absence is just an
ordinary value the code must handle. This makes "the crate degrades cleanly"
a statically-checked property, not a wish.

### Capability arguments

A grant may carry **arguments** that narrow it: filesystem access scoped to
specific directories, network access scoped to specific hostnames or URL
patterns, environment-variable reads restricted to specific names. Wildcards
are allowed where they make sense (`/tmp/myapp/*`, `*.example.com`).

```tel
# Illustrative — syntax not pinned down.
dependency "http-fetch" {
    version = "2.0"
    capabilities = [
        network(hosts = ["*.example.com"]),
    ]
}

dependency "report-writer" {
    version = "0.7"
    capabilities = [
        filesystem(write = ["${OUT_DIR}/*.pdf"]),
    ]
}
```

TODO(open): exact argument grammar (and where it lives — manifest, lock file,
or both). Keep arguments coarse enough to be readable; defer fine-grained
gating to the capability chapter. The base set of capabilities must be small
and fixed in the language so it is recognised everywhere.

### Capability transitivity

If crate `A` depends on `B`, and `B` declares it needs `network`, does `A`'s
manifest have to list `network` too?

**Decided: capabilities propagate, computed by the package manager.** Each
crate declares only its **own direct** capability use and the grants to its
**own direct** dependencies. The package manager computes the transitive closure
and **propagates** the effective capability set up to the top-level project,
which sees and reviews the full union. The lock file is the **authoritative
record** of that transitive grant — a reviewer reads the lock file to answer
"what can this project actually do?" in one place.

This is chosen over strict-transitive declaration (every crate restating every
capability it can transitively reach), which is maximally explicit but verbose in
deep trees and duplicates information the resolver already has. Propagation keeps
each manifest local and legible while still surfacing the complete effective set
where it matters — at the root, in the lock file. It pairs with the
[opt-in re-export rule](#transitive-dependencies-opt-in-only): a capability
travels with the dependency edge that needs it, not by ambient inheritance.

### What capability gating depends on

Capability gating only works because Tel rules out the language features that
would let untrusted code bypass it:

- **No runtime reflection or `eval`** — see
  [antifeatures](../02-philosophy/04-antifeatures.md). If a crate could call a
  function by name from a string, it could reach anything the runtime has.
- **No arbitrary system calls, embedded C, or external-process spawning** from
  Tel itself — these are capabilities the host hands out, not powers the
  language grants.
- **No `unsafe` escape hatch.**

If the host exposes any of these through the FFI, the host has voluntarily
broken the model for that script — see
[embedding Tel in a host](../16-ffi-and-interop/04-embedding-tel-in-a-host.md).
Tel's job is to make sure the model is intact by default.

### Capability gating is compile-time

Capability checking happens at compile time — a crate without the right
capabilities will not compile against the project. There is no second runtime
check at the language level (the host of course remains free to deny a
capability at runtime through its own gates).

TODO(open): granularity of capabilities at the dependency edge — is `network` a
single grant, or split (outbound only, specific hosts)? Keep it coarse enough
to stay readable; defer fine-grained gating to the capability chapter.

TODO(open): is "may mutate its arguments" itself a capability? Inputs raise
this. The data-race / mutability discussion in
[`04-antifeatures.md`](../02-philosophy/04-antifeatures.md) overlaps. Lean:
mutation of arguments is a *type-system* concern, not a capability — but
revisit once the mutability model is settled.

## Stable depending on unstable

A crate whose own version signals stability (e.g. `1.x` and up) is restricted
in how it may depend on a crate that signals instability (e.g. `0.x`):

- A stable crate **may not re-export** types from an unstable dependency. The
  unstable crate can change its public API at any time, so a re-export turns
  the stable crate's API into an unstable one in disguise.
- Even *consuming* an unstable dependency internally is risky for a stable
  crate, because the dependency may break at the next bump. A stable crate
  that wants to use an unstable one should either wait for it to stabilise or
  vendor a frozen copy.

TODO(open): whether re-export of unstable types is a *hard error* in the
toolchain, a warning, or a lint. Lean: hard error — *make the wrong thing
hard*. Whether stable-on-unstable dependency at all is forbidden is more
nuanced; inputs lean "allowed but loudly flagged."

## Shading: not the answer

Other ecosystems sometimes solve version conflicts by **shading** — rewriting a
dependency's crate name at build time so two copies coexist. Tel does not
plan to ship a shading tool, for two reasons:

- Shading breaks identity-based patterns. Two shaded copies of the same
  metrics singleton are *not the same* singleton; subtle bugs follow.
- Shading is a workaround for an ecosystem that did not solve dependency
  resolution. Tel's [versioning](06-versioning.md) story (single resolved
  version where possible, distinct types where not) is meant to remove the
  motivation.

TODO(open): confirm that the [version-compatibility model](06-versioning.md)
genuinely covers the cases shading would otherwise solve — in particular,
plugin-host scenarios where the host already loaded a different version of a
library. Philosophy gap if it does not.

## Crate metadata from code

A crate can read its own **identity metadata** at runtime — at least its
declared name and version, and (where defined) its build identifier. This is
the small, fixed set of fields that maps cleanly to logs, error reports, and
crash diagnostics ("script `pricing 1.4.2` reports …").

```tel
# Illustrative — exact API not pinned down.
let info = crate.info()
log.info("running ${info.name} v${info.version}")
```

TODO(open): exact API surface and the field list. Resist scope creep — this is
*not* a place to expose arbitrary manifest fields. Lean: name, version, and
optional build id only; anything else is the host's job.

## Mixed feature flags

Cargo-style **feature flags** (additive boolean knobs unioned across the
dependency graph) have a well-known footgun: features that select between
*mutually-exclusive backends* miscompile when the union picks both. Tel should
either not have feature flags, or rule out exclusive-backend features by
construction.

TODO(open): commit on feature flags. Lean: no general-purpose feature flags in
the manifest — the few real use cases (optional capabilities, optional
sub-modules) are better served by [optional capabilities](#required-optional-and-unused-capabilities)
and re-exports. If feature flags are admitted, they must be strictly additive
with no "choose one of N" form.

## What crates are not for

- **Not for host resources.** A host exposes data, functions, and capabilities
  to a script directly; that is not a crate.
- **Not a plugin/extension loader.** Crates are a compile-time dependency
  mechanism, resolved before the script runs.

## See also

- [Modules](01-modules.md) — what a crate is made of.
- [Imports](02-imports.md) — how a crate name becomes an import path.
- [Versioning](06-versioning.md) — resolving and mixing dependency versions.
- [Project Layout](05-project-layout.md) — the on-disk shape a crate zips up.
- [Build System](../18-tooling/03-build-system.md) — the opinionated,
  minimal-customization build tool.
- Supply-chain security — capability declarations are the part of the threat
  model Tel commits to today. The broader story (signing, attestation, build
  provenance, maintainer reputation) is **deferred until after the base language
  is ready**; the committed baseline is immutable versions and source-only
  distribution. See [Package Registry](09-package-registry.md).
- [Package Registry](09-package-registry.md) — where published crates are
  fetched from, and the reproducibility/unpublishing story.
