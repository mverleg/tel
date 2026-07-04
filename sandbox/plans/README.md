# Sandbox pending plans

Working plans for the sandbox query engine. Design *analysis* lives in
`../../docs/`; this directory holds the *pending plans and ordered todos*.

- **[roadmap.md](roadmap.md)** — the consolidated, ordered execution plan
  (Phases 0–4), merging the README feature checklist, the caching design
  priorities, in-code `TODO @mark` markers, and the gap analysis vs `qcompiler`.
  Start here.
- **[fast-mode.md](fast-mode.md)** — fast mode vs IDE mode: whether they are
  separate targets (no — a flavor), the shared-core + metadata-sidecar shape,
  and the fast→ide upgrade-retry path. Roadmap Phase 2 / step 11.
- **[flavors.md](flavors.md)** — query "flavors" (mode, opt-level, …) as a
  cache-key dimension: representation options, pros & cons, and a recommendation
  (per-query declared subset, starting with `mode`). Roadmap Phase 3 / step 15.

Related analysis (not plans, but referenced throughout):
- `../../docs/cache-invalidation-problem.md` — content-addressed caching, the
  two-layer model, early cutoff, and consistency under partial failure.
- `../../docs/inverse-dependency-graph.md`, `../../docs/cycle-detection.md`.
