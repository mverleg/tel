# Package Manifest

TODO: review

The manifest is the single declarative file at the root of a crate that
states its identity, its dependencies, the capabilities those dependencies
need, and any metadata the registry or build needs. Other tools never have to
parse Tel source to learn these basics — the manifest is enough.

## What

A crate directory has one manifest file at its root. The manifest is the
authoritative answer to:

- What is this crate called, and at which version?
- Which other crates does it depend on, and at which version constraints?
- Which capabilities does each dependency need? See
  [Per-dependency capability declarations](04-packages.md#per-dependency-capability-declarations).
- Which entry-point modules are public (the *export surface*)?
- Optional metadata: license, authors, repository URL, description.

The manifest does *not* contain executable logic — it is data. That keeps
parsing it cheap, keeps tooling outside the compiler workable, and makes the
file diffable.

## Why

Stability and reproducibility. A manifest pinned in source is a contract: any
conforming runtime, on any host, with the same lockfile, gets the same
dependency graph. See
[Dependency Graph and Locking](08-dependency-graph-and-locking.md).

## Open questions

- TODO(open): manifest *format* — Tel-flavoured data, TOML, JSON, or a custom
  declarative form. The standard library's data-formats topic and the
  [`std.tel_ast`](../17-standard-library/18-tel-as-data.md) sublanguage are
  candidates.
- TODO(open): how feature flags are spelled, and whether the manifest carries
  them at all (see `04-packages.md` "Mixed feature flags").
- TODO(open): the schema for the manifest itself — versioned, or frozen along
  with Tel1?

## See also

- [Crates](04-packages.md) — what a crate is and what shape it takes.
- [Dependency Graph and Locking](08-dependency-graph-and-locking.md) — how the
  manifest's constraints are resolved.
- [Package Registry](09-package-registry.md) — where the manifest's named
  dependencies are fetched from.
