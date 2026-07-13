# Sandbox TODO

Pending items left open after the query-engine implementation series
(2026-07, roadmap Phases 0–2 done — see [plans/roadmap.md](plans/roadmap.md)
for the ordered plan and what already landed). Grouped by what gates them.

## Gated on dependency approval

New dependencies need explicit approval first (project rule).

* ~~**OS file watcher**~~ — done 2026-07-05 (`notify` approved with the
  work): `src/monitor.rs` (`FileMonitor` trait, `DiskMonitor` + `MockMonitor`
  backends, batching `ChangeStream`) plus the `Compiler::run_watch_loop`
  driver. Design notes in [plans/daemon.md](plans/daemon.md).
* ~~**xxh3 hashing**~~ — done 2026-07-06 (`xxhash-rust` approved): the
  single `StableHasher` in `src/keys.rs` is xxh3 now (xxh3-128 content
  keys/digests, xxh3-64 fingerprints), `SCHEMA_VERSION` bumped to 2. Stable
  output unblocked the golden-fingerprint tests (now checked in).
* ~~**Persistence**~~ — done 2026-07-06 (`heed` + `postcard` approved),
  roadmap Phase 3 item 12: LMDB write-through content store (`src/disk.rs`)
  holding Sym-free portable entries (`src/portable.rs`), opened by the
  daemon via `Compiler::with_disk_cache` under `<root>/out/cache`. Still
  open: memory/disk tiering + eviction (item 13).

## Not gated, just not done

* **Cache eviction + parallel queries** — memory/disk cache tiering + eviction
  (item 13; the disk store is append-only, no eviction yet), now unified with
  parallel queries, `Pending` state, and input dedup. **Direction decided
  2026-07-10**, plan in
  [plans/concurrency-and-eviction.md](plans/concurrency-and-eviction.md);
  unimplemented. (Pluggable source backends and further query flavors are
  dropped from scope for now.)
* **External dependencies as sealed leaves** (item 13b) — external deps are
  immutable by contract, so they skip per-compile read+hash, the watcher, and
  dirty tracking; keyed on the lockfile-pinned release hash, gated by a
  provenance bit, and cross-project shareable. **Direction decided 2026-07-10**,
  plan in [plans/external-deps.md](plans/external-deps.md); unimplemented.
* **Fast mode vs IDE mode** — roadmap item 11, plan in
  [plans/fast-mode.md](plans/fast-mode.md). **Landed**: the span sidecar
  (`QueryKind::Spans`, memory-only, keyed on the source digest); a structural
  `(frame, node)` locator on **every** core AST node (a `{loc, kind}` wrapper,
  not just `panic`/`unreachable`); cached resolve/mono errors wrapped in
  `Located<E>`; and the in-session upgrade of a runtime `panic` **and** of
  type/resolve compile errors to `path:line:col`, at the exact offending
  sub-expression, via the sidecar on the error path (`tests/spans.rs`). A
  whitespace edit above an error shifts its reported line without re-checking
  types (early cutoff holds). Out-of-range literals now locate precisely too:
  `Loc` is threaded through `TExpr::Number`, so `LiteralOutOfRange` (raised in
  the lowering pass) is pinned to the offending literal like the check-phase
  errors. `SCHEMA_VERSION` 3→4. Still open: `refmap`/`typemap`/`docs`
  sidecars, the always-on error-recovering parser, and the multi-output
  *record* with per-output fingerprints (this slice uses the
  recompute-on-demand sidecar policy).
* **Roadmap Phase 4 hardening** — prevent ctx leak outside scope, lock-free
  during compile, box deep recursive awaits, parse on a threadpool, minor
  cleanups (`context.rs` visibility).
* **Example binaries carry stale generator copies** — `benchmark_test`,
  `inspect_generated`, `profile_run`, `single_test` each embed a copy of the
  old project generator that emits arity-invalid programs (broken at runtime
  since the function-arity rule; pre-existing, discovered when the same bug
  was fixed in the bench generator). Point them at the shared, fixed
  `benches/project_gen/` instead.
* **`Pending` node state** — currently unrepresented in the binding layer
  (single-flight lives in the async-lazy parse cache; runs are serialized by
  `run(&mut self)`). Now scheduled: parallel query waves are wanted, so
  `Pending` becomes represented as `ALazy::is_initializing` in the *content
  store* (keyed by content key, not `StepId`), per
  [plans/concurrency-and-eviction.md](plans/concurrency-and-eviction.md)
  Decision 4 — not in `BindingRecord`.

## Deliberate non-goals (recorded so they aren't re-litigated)

* Exec results stay uncached — their value is their side effects.
* Cycle errors stay uncached — their content key is not well-founded; the
  ancestor-path check re-fires cheaply on every demand.
* File reads stay fused with parse; the read itself re-runs each compile to
  derive the lookup digest (batch stance) or is skipped for clean cones
  (watch stance).
