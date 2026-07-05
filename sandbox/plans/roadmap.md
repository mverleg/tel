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
    `[cache-doc #7, #8]`
    - File watcher + reverse-edge (`transitive_dependents`) cone invalidation.
    - **Two-pass invalidation**: Pass 1 marks the whole cone dirty (infallible);
      Pass 2 recomputes bottom-up, clearing dirty only on successful commit.
    - **Panic-safe recompute**: `catch_unwind` at the recompute boundary marks
      the node dirty (never clean); "commit + mark clean" is the atomic last
      step. Guarantees consistency under partial failure without a root-down
      walk. (Note: this is strictly better than qcompiler's current stance of
      "trash the whole cache on panic".)

11. **Fast mode vs IDE mode** `[qcompiler-gap]` — detailed plan:
    [fast-mode.md](fast-mode.md)
    - Two versions of queries: metadata-light for fast compile, full
      source-locations for IDE / error messages. Try fast first, retry in IDE
      mode when an error occurs, to get good diagnostics.
    - Not separate targets — a *flavor* on query keys, with a shared
      mode-independent core + metadata sidecar; upgrade re-runs only the failing
      subtree and reuses the shared read cache.

## Phase 3 — Persistence & scale

12. **Store cache in LMDB (with postcard)** `[readme]` — persist the content
    store across runs (types already derive serde).

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

- **Tests for both change scenarios** `[cache-doc #9]` — revert hits parse
  cache; blank-line edit re-parses one file, zero resolve/exec recompute.
- **Partial-failure test** `[cache-doc #10]` — inject error/panic partway up an
  incremental leaf→root pass; assert a later change still recomputes the full
  affected chain (no falsely-clean node survives). Land with Phase 2.
- **Benchmarks** `[cache-doc #11]` — ensure caching/invalidation stays fast.

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
