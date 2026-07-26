# Build System

<!-- TODO: review -->

## What

Tel has, intentionally, **almost no build system of its own**. The compiler
([`01-compiler.md`](01-compiler.md)) is the toolchain: there is no `Makefile`,
no `build.gradle`, no `build.rs`, no separate "build script" phase. A project
is its sources plus a small manifest
([`../11-modules-and-packages/07-package-manifest.md`](../11-modules-and-packages/07-package-manifest.md)),
and `tel build` / `tel test` / `tel check` are subcommands of the compiler.

The build *system*, such as it is, is the **host application's** problem
when Tel is being embedded. The host knows when to compile a script, when to
recompile, where the output goes, and what other host artefacts depend on
it. Tel's job is to make that fast and predictable.

## Why so little

Embedding is the point. A host that already has its own build system (Bazel,
CMake, Gradle, Cargo, npm, …) does not want a second one to coordinate with.
The maxim is **the compiler is the whole toolchain — no separate build step,
no build scripts** ([`../02-philosophy/02-maxims.md`](../02-philosophy/02-maxims.md)).

A fair amount of build-system machinery is conceivable — declaring
"plugins" that compile JSON schema to types, plugging Sass into a Tel
pipeline, build-time external script execution, build coordinator daemons.
**None of this lands in Tel.** A host that needs to generate code from a JSON
schema runs that step itself, then hands the generated Tel sources to the
compiler like any other source. The compiler does not call out to user
programs as part of a build.

This is the same call as **no proc-macros and no `build.rs`-style scripts**
([`../02-philosophy/04-antifeatures.md`](../02-philosophy/04-antifeatures.md)):
once user code runs at compile time, compile speed is unpredictable, caching
becomes fragile, and the host loses control over what its scripts can do.

**The one extension point** is that `tel build` can run a small, fixed set of
known tools at defined steps — notably the [linter](07-linter.md) and
[formatter](06-formatter.md). These are invoked as whole commands at fixed
points; they are *not* a plugin API, do not load user code into the compiler,
and cannot inspect or rewrite the program mid-compilation. A richer
compiler-plugin model is **deferred until after the base language is ready**
(see [Crates: crate kinds](../11-modules-and-packages/04-packages.md#crate-kinds-no-plugin-kind-for-now)
and [Deferred Features](../20-appendix/06-deferred-features.md#compiler-plugin-model-and-crate-distributed-lints)).

## What `tel build` actually does

A minimal, predictable pipeline:

1. Read the project manifest.
2. Resolve dependencies (see [Package Manager](04-package-manager.md)).
3. Compile, in **clean (bulk) mode** by default; in incremental mode under
   the LSP and under `tel test --watch`.
4. Emit either the runnable artefact (for AOT) or the cached IR (for
   interpreter embedding). See
   [`02-compile-targets.md`](02-compile-targets.md).

There is no separate codegen phase, no link phase, no plugin discovery, no
templated configuration. A project either compiles or it does not.

## Targets

A single Tel project can produce more than one output — a library plus a
test runner, two binaries with different entry points, a per-host variant.
This is described in the manifest, not in a build script:

```tel
# manifest sketch — exact spelling TBD
project "my-tool" {
    target lib { entry = "lib.tel" }
    target cli { entry = "cli.tel" depends = ["lib"] }
    target test { discover_tests = true }
}
```

`TODO(open): manifest syntax. The dependency / project chapter is the
authoritative source; this is just a placeholder.`

## What does *not* live in the build system

- **Codegen from external schema files** (JSON Schema, Protobuf, OpenAPI).
  Run the generator separately; check in or generate-then-build at the
  *host* level. The compiler does not invoke external programs.
- **Conditional compilation by build flag.** Tel deliberately does not have
  `#cfg`-style boolean flags that include or exclude lines. Variants are
  expressed as host-implemented capabilities or as platform crates, never
  as preprocessor switches. Feature flags are rejected
  on the same grounds the C preprocessor was rejected: combinations
  multiply, untested branches rot, errors hide in disabled code.
- **Deployment, packaging into installers, signing, container builds.**
  Host territory.

## Cross-cutting concerns

The compiler does retain a few build-shaped behaviours because they are
needed for *correctness*, not flexibility:

- **Dependency resolution and locking.** See
  [`../11-modules-and-packages/08-dependency-graph-and-locking.md`](../11-modules-and-packages/08-dependency-graph-and-locking.md).
- **Capability declarations per crate.** A dependency declares what
  capabilities it needs; `tel build` surfaces these so the host can grant or
  deny them. See [Package Manager](04-package-manager.md).
- **Reproducible output.** Given the same inputs and the same compiler
  version, `tel build` produces byte-identical output. This is part of the
  stability commitment and a precondition for caching at the host level.

## See also

- [Compiler](01-compiler.md)
- [Compile Targets](02-compile-targets.md)
- [Package Manager](04-package-manager.md)
- [Antifeatures](../02-philosophy/04-antifeatures.md) — why there is no
  proc-macro / build-script phase
