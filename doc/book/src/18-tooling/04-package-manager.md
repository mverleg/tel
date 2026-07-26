# Package Manager

<!-- TODO: review -->

## What

The package manager is the subcommand of the Tel toolchain that **acquires,
resolves, and stores third-party crates** so the compiler can find them. It
reads the [manifest](../11-modules-and-packages/07-package-manifest.md), talks
to the [registry](../11-modules-and-packages/09-package-registry.md), records
the resolved graph in a
[lockfile](../11-modules-and-packages/08-dependency-graph-and-locking.md), and
populates a per-user cache that the compiler then consumes.

It is intentionally a *single* tool, sharing a process and a build cache with
the compiler in the Cargo / `go` mould. There is no separate "downloader" CLI,
no separate "resolver" CLI, no separate "publisher" CLI; the subcommands all
live on `tel`.

## Why one tool

The maxim is **the compiler is the whole toolchain — no separate build step,
no build scripts** ([`../02-philosophy/02-maxims.md`](../02-philosophy/02-maxims.md)).
A build coordinator and a package manager living in different binaries forces
the user to learn two tools' configuration languages and keep them in sync;
the [build system topic](03-build-system.md) and this one are deliberately
the same executable.

`TODO(open): the implementation may reuse or wrap an existing open-source
package manager rather than building one from scratch — the requirements
(immutable versions, lockfile-pinned content hashes, capability-aware
resolution, source-only distribution) matter more than a bespoke tool. The
package manager should be *decent*, but it is not where Tel needs to innovate.
Evaluate candidates once the base language is ready.`

The package manager exists at all because Tel scales beyond single scripts:
the [crate chapter](../11-modules-and-packages/04-packages.md) calls
packaging an *opt-in, project-scale* feature. A 30-line embedded hook never
talks to the package manager.

## Surface

A minimal set of subcommands the toolchain exposes:

- `tel add <pkg>` — record a new dependency in the manifest, resolve, and
  update the lockfile.
- `tel update` — re-resolve within the manifest constraints, refresh the
  lockfile.
- `tel install` (often implicit) — populate the local cache from the lockfile
  so the compiler can build offline next time.
- `tel publish` — push a crate version to the configured registry.
- `tel vendor` — copy the resolved graph into a project-local directory so the
  project can build with no registry access. `TODO(open): vendoring is also
  the host's option — see open question in
  [`../11-modules-and-packages/08-dependency-graph-and-locking.md`](../11-modules-and-packages/08-dependency-graph-and-locking.md).`

`TODO(open): exact subcommand names. Lean toward Cargo's
vocabulary (add, update, publish); confirm.`

## Installing binary utilities

The package manager doubles as a way to **distribute Tel-based command-line
tools**. A crate that is itself a runnable util can be installed so its
entry point lands on the user's `PATH` — the `cargo install` / `go install`
/ `pipx` shape — letting a team ship internal tooling written in Tel without a
separate packaging pipeline.

```text
# Illustrative — subcommand name not pinned down.
tel install tel-fmt-check     # fetch, build, place a launcher on PATH
```

There is **no separate "executable" crate kind** for this — a tool is an
ordinary [crate](../11-modules-and-packages/04-packages.md#crate-kinds-no-plugin-kind-for-now)
like any other, and `tel install` is just a **separate command** that builds a
designated entry-point module and drops a thin launcher (a Tel runtime invoking
that module) onto `PATH`. The crate kind stays singular; installability is a
property of the *command*, not a new kind.

The mechanism reuses everything already described: the tool is fetched and
resolved like any dependency, its
[capabilities](../11-modules-and-packages/04-packages.md#per-dependency-capability-declarations)
are surfaced and granted explicitly at install time, and the result is recorded
so it can be listed and uninstalled.

`TODO(open): how the entry-point module is designated (a manifest key, a
convention), where the launcher goes on Windows and inside a sandboxed host, and
how an installed tool's capabilities are re-confirmed on upgrade.`

## Per-user cache (the "virtual environment by default")

The package manager should **behave like a virtual
environment by default**. The user-level cache holds *every* resolved version
of every crate, side by side:

```text
~/.cache/tel/crates/
  pricing/
    1.4.2/
    1.5.0/
  csv-tools/
    1.4.0/
```

A given project only *sees* the versions its lockfile actually resolves to —
even if other versions are cached for other projects. No global "active
version" of a crate; no per-machine state that influences which version a
script ends up using.

Consequences:

- Two projects on the same machine can depend on different versions of the
  same crate without colliding. Installing or removing dependencies on
  project A never breaks project B.
- The cache is just a content-addressed store. Wiping it costs only download
  time, never correctness.
- The host's own dependencies (the Tel runtime, capability adapters) are
  *not* in this cache — the host ships those.

`TODO(open): cache location. The above is illustrative; on Windows and inside
a sandboxed host it must follow whatever the host picks. Decide whether the
default is overridable by env var, by manifest, both, or neither.`

## Capability-aware fetching

The package manager carries the
[per-dependency capability declarations](../11-modules-and-packages/04-packages.md#per-dependency-capability-declarations)
through to the build:

- When a dependency is added, the resolver records the capabilities it
  declares. The user is shown the new capability surface and must confirm or
  pin it in the manifest.
- A version bump that *requests new capabilities* is surfaced loudly and
  refuses to compile until a human has reviewed and widened the grant. This is
  the supply-chain failure mode the design is built to catch — a previously
  pure-data crate suddenly asking for filesystem access cannot slip through
  on a routine update.
- A crate marked as *unstable* (0.x) that a *stable* crate tries to
  re-export is rejected at resolve time, not at compile time, so the failure
  is attributed to the package manager — the chapter on
  [crates](../11-modules-and-packages/04-packages.md#stable-depending-on-unstable)
  spells out the rule.

## Approval-server and mirror story

A real-world need: organisations want to **review and
host their own artefacts** rather than fetch directly from a public registry.
The package manager treats mirrors and approval servers as first-class:

- Any URL or registry that speaks the standard protocol can be configured as
  a source. The lockfile pins content hashes, so a mirror that serves the
  same bytes resolves identically to the upstream.
- A *review* mirror can refuse to publish a crate version until a human has
  signed off. The CLI surfaces "this version is pending review" distinctly
  from "this version does not exist."
- Different *trust tiers* per source — e.g. test/dev crates from a looser
  mirror, production dependencies only from the reviewed one — fit naturally
  on top of per-dependency capability grants.

`TODO(open): protocol shape. One option is "just point at GitHub
tags" alongside a content-addressed store. Decide whether the registry
protocol is fully open (any URL endpoint can be a registry) or whether
there is a defined HTTP API. See
[`../11-modules-and-packages/09-package-registry.md`](../11-modules-and-packages/09-package-registry.md).`

## Reproducibility

The package manager is one half of Tel's reproducibility promise. The
contract:

- A fresh checkout with a lockfile resolves to **the same exact set of
  versions**, regardless of when or where it runs.
- A fresh checkout *without* a lockfile re-resolves; the result should match
  the locked one unless the registry has been pruned (the registry's
  "no silent unpublishing" rule, see
  [`../11-modules-and-packages/09-package-registry.md`](../11-modules-and-packages/09-package-registry.md),
  is what makes this true).
- Cached crates are byte-identical between machines that fetched the same
  version. The cache is content-addressed.

This is the same property the [build system topic](03-build-system.md) calls
*reproducible output* — the package manager is what makes it possible upstream.

## What the package manager is *not*

- **Not a build-script runner.** It never executes user code at install time
  (Cargo's `build.rs` is rejected for the same reason as proc-macros — see
  [`../02-philosophy/04-antifeatures.md`](../02-philosophy/04-antifeatures.md)).
  A crate is data the compiler reads, not a program the manager runs.
- **Not a deployment tool.** It does not push artefacts to production, build
  containers, sign installers, or configure runtimes. Host territory.
- **Not a host-resource installer.** The host owns its own libraries,
  plug-ins, and assets; the package manager handles Tel-to-Tel reuse only. A
  graphics engine's shader pack is not a Tel crate.
- **Not interactive in CI.** Every subcommand has a deterministic batch mode;
  prompts are for local development only.

## See also

- [Build System](03-build-system.md) — the same tool, on the build side.
- [Crates](../11-modules-and-packages/04-packages.md) — the unit being
  resolved.
- [Package Manifest](../11-modules-and-packages/07-package-manifest.md) — what
  the user edits.
- [Package Registry](../11-modules-and-packages/09-package-registry.md) —
  what the manager fetches from.
- [Dependency Graph and Locking](../11-modules-and-packages/08-dependency-graph-and-locking.md)
  — what the manager writes out for reproducibility.
- [Antifeatures](../02-philosophy/04-antifeatures.md) — why there is no
  install-time scripting and no bundled web/ORM/REST framework.
