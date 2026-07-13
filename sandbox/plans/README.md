# Sandbox pending plans

Working plans for the sandbox query engine. Design *analysis* lives in
`../../docs/`; this directory holds the *pending plans and ordered todos*.

- **[roadmap.md](roadmap.md)** — the consolidated, ordered execution plan
  (Phases 0–4), merging the README feature checklist, the caching design
  priorities, in-code `TODO @mark` markers, and the gap analysis vs `qcompiler`.
  Start here.
- **[fast-mode.md](fast-mode.md)** — fast mode vs detail (IDE) mode: whether
  they are separate targets (no — same code path; likely not even a key
  dimension: detail = sidecar queries + driver demand policy), the
  core + sidecar shape, NodeId locators, the upgrade-retry path, and the
  runtime/AOT line-number story. Roadmap Phase 2 / step 11.
- **[flavors.md](flavors.md)** — query "flavors" (opt-level, …) as a
  cache-key dimension: representation options, pros & cons, and a
  recommendation (per-query declared subset). Mode was the candidate first
  flavor but likely dissolves into demand policy (see fast-mode.md); the
  first real customer is then opt-level. Roadmap Phase 3 / step 15.
- **[concurrency-and-eviction.md](concurrency-and-eviction.md)** — cache
  eviction (roadmap item 13) unified with parallel queries, `Pending` state,
  and input dedup: never hand out `Arc<V>` of an entry (scope safety = item 16),
  but keep `Arc<Global>` + `tokio::spawn` + owned results for parallelism;
  compaction between waves under `&mut self` (via `Arc::get_mut` after the run
  joins its tasks), admission control (enqueue but don't start over budget),
  single-flight per kind, size-aware LRU on a byte budget, and barrier-free
  (`Relaxed`) recency stamping. Direction decided 2026-07-10.
- **[external-deps.md](external-deps.md)** — external dependencies as *sealed
  leaves*: immutable by contract, so they skip per-compile read+hash, the
  watcher, and dirty tracking. Keyed on the lockfile-pinned release hash (not
  bare semver); a provenance bit gates which cones may seal; entries are
  shareable across projects. Direction decided 2026-07-10.

Related analysis (not plans, but referenced throughout):
- `../../docs/cache-invalidation-problem.md` — content-addressed caching, the
  two-layer model, early cutoff, and consistency under partial failure.
- `../../docs/inverse-dependency-graph.md`, `../../docs/cycle-detection.md`.
