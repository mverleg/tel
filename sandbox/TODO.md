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

* ~~**Cache eviction + parallel queries**~~ — done 2026-07-22/23 (item 13),
  all four phases of
  [plans/concurrency-and-eviction.md](plans/concurrency-and-eviction.md):
  the cache primitive + between-wave compaction, admission control,
  single-flight for derived kinds, and the byte budget wired into daemon config
  (`TEL_SANDBOX_CACHE_BUDGET`) with GC on wave completion. (Pluggable source
  backends and further query flavors stay dropped from scope.)
* **External dependencies as sealed leaves** (item 13b) — external deps are
  immutable by contract, so they skip per-compile read+hash, the watcher, and
  dirty tracking; keyed on the lockfile-pinned release hash, gated by a
  provenance bit, and cross-project shareable. **Direction decided 2026-07-10**,
  plan in [plans/external-deps.md](plans/external-deps.md). **Slice 1 landed
  2026-07-26**: `ContentDigest::sealed` in `src/keys.rs` (`SCHEMA_VERSION` 4→5)
  and `src/deps.rs` (`LeafSource`/`SealedCoord`, JSON `Lockfile`, XDG store
  path, temporary hash-at-lock). Slices 2–4 open — import→coordinate wiring,
  the provenance bit, and the shared sealed tier; until Slice 2 the module is
  private and unused outside its own tests. This is the last open item of the
  swap readiness gate
  ([plans/swap-to-real-language.md](plans/swap-to-real-language.md) §4).
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
* **Roadmap Phase 4 hardening** — ~~prevent ctx leak outside scope~~ (done,
  see below), lock-free during compile, box deep recursive awaits, parse on a
  threadpool, minor cleanups (`context.rs` visibility).
  * ~~**Ctx-leak (item 16)**~~ — **done 2026-07-22.** Resolve/mono bodies were
    already leak-safe (their `ResolveContext<'a>`/`MonoContext<'a>` borrow
    `&Global` and are sync, so the borrow *is* the enforcement). Exec/codegen
    were the holdout: the old `ExecContext` merged the driver role (the
    up-front `resolve_all`+`mono` pulls, which spawn and need the `Arc`) with
    the body role (sync interpret/emit, read-only), forcing an `Arc<Global>`
    the body could clone out. Split along that line: `Global::execute_impl`/
    `codegen_impl` gather deps async via the shared
    `Global::gather_backend_inputs` (keeps `&Arc<Global>`), then invoke the
    backend body — `execute::interpret` / `codegen::generate_python` — as a
    bare **`fn` pointer** over a borrowed `BackendCtx<'a> { core: &'a Global }`.
    The `fn` has no environment to capture the context into; the borrow stops
    it outliving the call. Independent of "box deep recursive awaits" (18): the
    resolve→resolve chain inside `resolve_all_impl` still keeps its own `Arc`;
    exec's future is never spawned, so its body needs no `'static` handle. All
    suites green (`tests/codegen.rs`, `tests/spans.rs`).
  * **Answer-seam (swap-plan §4 gate) — audited 2026-07-22, already satisfied:**
    `graph.rs`/`store.rs`/`keys.rs` touch answers only through
    `StableHash::stable_hash`, the stored `fingerprint`, and the free
    `portable::*` fns; no mechanism code pattern-matches answer internals. No
    change needed — recorded so it stops reading as an open blocker.
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
