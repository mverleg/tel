# Sandbox Query-Engine Roadmap

Ordered execution plan for the sandbox query compiler. This consolidates three
existing sources plus a gap analysis against `qcompiler`:

- The **feature checklist** in `README.md` ("Query engine features")
- The **caching design + priorities** in `../../docs/cache-invalidation-problem.md`
- **In-code `TODO @mark`** markers
- **Capabilities `qcompiler` has that the sandbox lacks** (see bottom section)

The README checklist stays the raw status list; this file is the *ordered* plan
and the rationale for the order. Items are tagged with their source:
`[readme]`, `[cache-doc]`, `[code]`, `[qcompiler-gap]`.

Ordering principle: correctness foundations before caching; in-memory caching
before persistence; incremental/watch before scale/perf polish. Each phase
unblocks the next.

---

## Phase 0 — Correctness foundations

These gate everything: no point caching results that can be stale or come from a
cyclic graph.

1. **Finish cycle detection** `[readme]` `[qcompiler-gap]`
   - Partially implemented: `graph.rs::find_resolve_cycle` (DFS over resolve
     edges) exists but the README item is still unchecked — it is not wired into
     the compile flow or surfaced as a user error.
   - Wire it into resolve/exec and report the cycle path.
   - Consider qcompiler's **query-kind ordering** (kinds ordered, may only call
     "down") to make *cross-kind* cycles impossible *by construction*
     (exec→resolve→parse can only go down).
   - **Ordering is not sufficient on its own.** Imports are a *same-kind* edge
     (`parse A` → `parse B` → `parse A`), and same-level calls are allowed, so
     ordering can never rule them out. Runtime cycle detection therefore stays
     **permanently required** for the import graph — it is the primary
     mechanism there, not a fallback. Ordering just shrinks what runtime
     detection must cover to same-kind edges. (qcompiler's README notes the same:
     "and same level, so it is not sufficient".)

2. **Content-addressed parse keys** `[cache-doc #3]`
   - Add a content hash to `ParseId` (currently `{ file_path }` only).
   - Delivers Scenario A (revert / branch-switch hits the cache; use content
     hash, **not** mtime).
   - Foundational: the whole caching approach is content-based.

3. **Schema hash in the file cache** `[readme]`
   - Include a hash of the cache/data-format schema in keys so a compiler change
     invalidates stale on-disk entries. Pairs naturally with step 2 before any
     persistence exists to be poisoned.

## Phase 1 — Cache the pipeline (in-memory)

Make caching correct and useful within a live process, per the committed
two-layer model (immutable content store vs mutable binding layer).

4. **Separate content store from binding layer** `[cache-doc #6]`
   - Immutable `digest → result` store (errors included) vs mutable
     `position → current digest` memo. Only the latter carries dirty state.
   - Prerequisite for correct invalidation and partial-failure safety.

5. **Cache computation steps** `[readme]` — memoize resolve/exec through the
   binding layer (parse is already cached; extend to derived results).

6. **Cache IO steps + selective caching** `[readme]`
   - Note: we committed to keeping **read fused with parse** (`parse.rs:300`),
     so "cache IO" here means the parse-level source result, and "selective
     caching (e.g. not file read)" is the policy knob for what is worth storing.

7. **Transitive validation for resolve/exec** `[cache-doc #4]` — before serving
   a cached derived result, verify upstream steps are still valid.

8. **Output-digest comparison / early cutoff** `[cache-doc #5]` — **done**
   - Recompute, compare result to cached, stop propagation when unchanged; key
     downstream results on the input's *output* digest, not source bytes.
   - Delivers Scenario B (whitespace-only edit re-parses one file, tree above
     stays cached) — asserted by test, plus a semantic-edit control and
     function-level cutoff within a file. Errors carry (tagged) fingerprints
     too, so dependents of stably-erroring steps are cached answers as well.

## Phase 2 — Incremental & watch mode

Requires a persistent process (~~the in-code TODO at `lib.rs:86` about getting
rid of the `Box::leak` for a continuous shared-cache process~~ — done:
`Compiler` owns its `Global` via `Arc`, runs serialized by `&mut self`,
dropping it reclaims everything).

9. **Incremental compile starting from main** `[readme]` — **done** —
   demand-driven top-down re-derivation reusing the caches from Phase 1.
   Scenario A asserted across all phases (revert recomputes nothing); a file
   dropped from the import graph is not even demanded; a re-resolved step
   *replaces* its dependency edges, so cross-run restructures cannot
   accumulate zombie edges (which could otherwise fabricate a phantom cycle).

10. **Incremental compile starting from leafs + watch mode** `[readme]`
    `[cache-doc #7, #8]` — **done except the OS watcher itself**
    - Explicit `Compiler::invalidate(path)` (what a watcher would call) +
      `run_watch` (trust-clean stance: clean subgraphs are served from their
      bindings without even re-reading their sources); plain `run` remains the
      always-correct batch stance that never trusts events. The `notify`-based
      watcher is deferred (new dependency — needs approval).
    - **Two-pass invalidation**: Pass 1 marks the whole cone dirty via reverse
      edges (bit flips only, infallible); Pass 2 is the next watch run, which
      re-derives exactly the dirty ∩ live cone, clearing dirty only as part of
      a successful whole-record commit — and early cutoff un-dirties a node
      whose fingerprint comes back unchanged.
    - **Panic-safe recompute**: `catch_unwind` at the recompute boundary; a
      panic becomes a non-terminal `Panicked` error that is never cached or
      fingerprinted, and the node's binding stays dirty. Asserted by test:
      caches stay unpoisoned and a later fix recomputes the full chain. (Note:
      this is strictly better than qcompiler's current stance of "trash the
      whole cache on panic".)
    - Mono instances now hang off their defining file's resolve step in the
      graph, so a file's marking cone includes its function instances.

11. **Fast mode vs IDE mode** `[qcompiler-gap]` — detailed plan:
    [fast-mode.md](fast-mode.md)
    - Two versions of queries: metadata-light for fast compile, full
      source-locations for IDE / error messages. Try fast first, retry in IDE
      mode when an error occurs, to get good diagnostics.
    - Not separate targets — a *flavor* on query keys, with a shared
      mode-independent core + metadata sidecar; upgrade re-runs only the failing
      subtree and reuses the shared read cache.

## Phase 3 — Persistence & scale

12. **Store cache in LMDB (with postcard)** `[readme]` — **done 2026-07-06.**
    The content store has an append-only LMDB tier (`src/disk.rs`, via
    `heed`) holding postcard-encoded **Sym-free portable entries**
    (`src/portable.rs`) — stored answers carry interned ids, so they are
    rewritten with a per-entry string table and re-interned on load, which
    makes them valid in any process. Reads are inline (mmap lookups); writes
    go through one batched writer thread with `NO_OVERWRITE` (disk
    first-write-wins). A `(format, schema)` stamp wipes the cache on
    mismatch; corruption is a miss, never a panic. The daemon opens it under
    `<root>/out/cache` via `Compiler::with_disk_cache`; `--no-daemon` stays
    cold and hermetic. Prerequisite (xxh3 stable hashing) landed first.

13. **Memory/disk cache tiering** `[qcompiler-gap]` — keep hottest entries in
    memory, spill everything to disk (LRU or similar).

14. **Pluggable source backends** `[qcompiler-gap]` — abstract the leaf read so
    a source file can come from disk *or* an in-memory / web-IDE buffer, instead
    of `tokio::fs` directly.

15. **Query "flavors"** `[qcompiler-gap]` — detailed plan: [flavors.md](flavors.md)
    (leaning towards adopting). Parameterize keys by environment (mode, opt
    level). Recommended shape: a *per-query declared* flavor subset (not an
    ambient env — that fragments the cache), starting with `mode` only. Note:
    "which filesystem" resolves to content (not a flavor), "which cache" is
    storage-layer, and generics are query *parameters*, not flavors. Sandbox
    keys are currently just `file_path` / `FQ` with no flavor dimension.

## Phase 4 — Hardening & performance

16. **Prevent context leak outside scope** `[readme]` — pure fn pointers so a
    step can't smuggle the context out of its scope.

17. **Lock-free during compile** `[readme]`.

18. **Box recursive awaits** `[qcompiler-gap]` — box specific awaits (deep
    resolve→resolve chains) onto the heap to prevent stack overflow.

19. **Delegate parse to a threadpool** `[code]` — `parse.rs:301`.

20. **Minor code cleanups** `[code]` — e.g. `context.rs:195` visibility.

## Cross-cutting (do alongside the phases they validate)

- **Tests for both change scenarios** `[cache-doc #9]` — **done**, in both
  stances: batch (`tests/incremental.rs` scenario A, `tests/cache_invalidation.rs`
  scenario B) and watch (`tests/invalidation.rs` revert + formatting cutoff).
- **Partial-failure test** `[cache-doc #10]` — **done**
  (`tests/invalidation.rs`): panic mid-wave leaves the node dirty and the
  stores unpoisoned; an edit made *while* the chain was broken by a
  deterministic error is reflected once the chain heals — no falsely-clean
  node survives either failure mode.
- **Benchmarks** `[cache-doc #11]` — **done**; also fixed: the generator had
  emitted arity-invalid programs ever since the "Function arity" rule landed
  (`ArityGap`/`ArityMismatch`), so every bench run since then failed — masked
  because benches are outside `cargo test`. The generator now closes arg gaps
  and emits call sites at the callee's actual arity, it is shared with
  `tests/generated_project.rs` as a regression guard, and the query-engine
  machinery costs nothing on a cold compile vs pre-series 847fed8 (32k funcs:
  1.29s vs 1.34s; 64k: 2.63s vs 2.73s — if anything slightly faster, thanks
  to parallel import resolution).

---

## Deferred / out of scope for the query engine

These are things `qcompiler` covers that the sandbox intentionally does not —
they are compiler/product features, not query-engine infrastructure (the
sandbox exists to test the engine):

- **Type-check query** (`type of X`) — the README's "Type Check" future phase.
- **Monomorphization query** (`monomorph F for (U, T)`).
- **Swappable codegen / backend** — explicitly out of scope in both projects.

## Where the sandbox is already ahead of qcompiler

For the record (no action needed) — qcompiler lists these as open questions the
sandbox has already solved:

- **Concurrent duplicate tasks / pending results** — handled by `async-lazy`
  `Cache` (README items on parallel imports and "same task twice" are checked).
- **Panic handling** — sandbox has a `catch_unwind` + dirty plan (Phase 2 / step
  10); qcompiler's TODO still assumes it must trash the whole cache.
