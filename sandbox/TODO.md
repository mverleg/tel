# Sandbox TODO

Pending items left open after the query-engine implementation series
(2026-07, roadmap Phases 0–2 done — see [plans/roadmap.md](plans/roadmap.md)
for the ordered plan and what already landed). Grouped by what gates them.

## Gated on dependency approval

New dependencies need explicit approval first (project rule).

* **OS file watcher** — needs the `notify` crate. The invalidation machinery
  is complete (`Compiler::invalidate(path)` + `run_watch`); the watcher is
  just the event source that calls `invalidate` on file-change events, then
  triggers a watch run. Until then, callers announce changes manually.
* **xxh3 hashing** — needs `xxhash-rust` (already a qcompiler dependency,
  same rationale). Swap point is the single `StableHasher` in
  `src/keys.rs`, currently `DefaultHasher`-backed: xxh3-128 for content
  keys, xxh3-64 for fingerprints per [../docs/hashing.md](../docs/hashing.md).
  Not urgent while caches are in-memory only, but a prerequisite for
  persistence (DefaultHasher output is not guaranteed stable across
  toolchains) — and it unblocks golden-fingerprint tests, which were
  deliberately deferred for that reason.
* **Persistence** — roadmap Phase 3: LMDB + postcard content store, then
  memory/disk tiering. Keys are already portable-by-construction
  (schema hash included) except for the hasher above, which must land first.

## Not gated, just not done

* **Roadmap Phase 3 remainder** — pluggable source backends (abstract the
  leaf read away from `tokio::fs`), query flavors
  ([plans/flavors.md](plans/flavors.md)).
* **Fast mode vs IDE mode** — roadmap item 11, plan in
  [plans/fast-mode.md](plans/fast-mode.md). Note Step 5 already made
  `PreExpr` location-free for cutoff reasons; the metadata sidecar design
  should build on that split.
* **Roadmap Phase 4 hardening** — prevent ctx leak outside scope, lock-free
  during compile, box deep recursive awaits, parse on a threadpool, minor
  cleanups (`context.rs` visibility).
* **Example binaries carry stale generator copies** — `benchmark_test`,
  `inspect_generated`, `profile_run`, `single_test` each embed a copy of the
  old project generator that emits arity-invalid programs (broken at runtime
  since the function-arity rule; pre-existing, discovered when the same bug
  was fixed in the bench generator). Point them at the shared, fixed
  `benches/project_gen/` instead.
* **`Pending` node state** — deliberately unrepresented in the binding layer
  (single-flight lives in the async-lazy parse cache; runs are serialized by
  `run(&mut self)`). Becomes real work only if concurrent query waves are
  introduced — see the note on `BindingRecord.dirty` in `src/store.rs`.

## Deliberate non-goals (recorded so they aren't re-litigated)

* Exec results stay uncached — their value is their side effects.
* Cycle errors stay uncached — their content key is not well-founded; the
  ancestor-path check re-fires cheaply on every demand.
* File reads stay fused with parse; the read itself re-runs each compile to
  derive the lookup digest (batch stance) or is skipped for clean cones
  (watch stance).
