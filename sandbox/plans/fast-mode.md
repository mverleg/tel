# Fast mode vs IDE mode — design draft

Status: **draft / not implemented.** Roadmap: Phase 2, step 11 (see
[roadmap.md](roadmap.md)). Closely related to [flavors.md](flavors.md) — "mode"
is the first concrete flavor.

## Goal

Two views of the same compilation:

- **Fast mode** — minimal metadata. No source spans, no reference tables, no
  rendered messages. Smallest/fastest path to "does it compile, and what does it
  produce". This is the batch-compiler default.
- **IDE / meta mode** — full metadata: source spans (line/col), resolved
  reference maps (go-to-def), original identifiers, rendered diagnostics. Slower
  and larger, needed for good error messages and editor features.

The compiler runs **fast first**; only if it hits an error (or an editor asks
for rich info) does it **upgrade** the relevant part to IDE mode.

Where the sandbox is today: the AST has essentially *no* metadata already
(`PreExpr`/`Expr` carry no spans; only `Panic`/`Unreachable` have a
`source_location: String`). So the current behaviour ≈ fast mode. This plan is
mostly about *adding* the IDE-mode layer, not about stripping anything.

## Are they separate targets?

**No.** They are not separate build targets/artifacts and not two independent
compilations. It is **one logical compilation of one target, observed at two
metadata resolutions.** Concretely:

- **Mode is a *flavor* on query keys**, not a separate query graph. `parse(F)`
  becomes `parse(F, mode)`; fast and IDE results are separate *cache entries*
  under the same logical query, not separate *targets*. (See
  [flavors.md](flavors.md) for the general flavor mechanism.)
- **Semantics must be identical across modes.** Mode may only change *metadata
  richness*, never *decisions*: the set of errors, name resolution, and produced
  values must be the same in both modes. If IDE mode could resolve or fail
  differently, the upgrade-retry path below is unsound (the retry might not
  reproduce the error, or hide it). This is the load-bearing invariant.

Why not the alternatives:

- **Fully separate parallel queries** (`parse_fast`/`parse_ide` as unrelated
  kinds): double-caches, doubles the graph, and — worst — lets the two pipelines
  *diverge*, breaking the invariant above. Rejected.
- **Always compute IDE, derive fast by stripping**: defeats the point; fast mode
  exists to be *cheaper*, not to be a projection of the expensive one.

## Representation: shared core + metadata sidecar

To avoid caching two near-identical copies and to make upgrade cheap, prefer a
**layered** shape over two wholesale representations:

- Each step produces a **mode-independent core result** (the AST structure, the
  resolution *decisions*, the types/values).
- IDE mode *additionally* produces a **metadata sidecar** keyed by the same node
  ids (a span table, a reference map, rendered-message inputs).

So `parse_ide(F)` = `parse core` + `span sidecar`; `resolve_ide(F)` =
`resolve core` + `reference sidecar`. Fast mode simply omits the sidecars.

Caveat that keeps this honest: fast mode may *drop info during computation*
(e.g. never track spans through resolve), so the IDE sidecar generally **cannot
be reconstructed from the fast output** — it needs a re-run *with tracking on*.
That is fine: the re-run reuses all mode-independent inputs (see next section),
so its cost is the metadata-tracking overhead over the affected subtree, not a
cold rebuild.

## Which steps are mode-sensitive

| Step | Mode-sensitive? | Notes |
|---|---|---|
| Read / content-hash (`parse.rs:300`) | **No** | Same bytes → shared cache entry across modes. Big reuse win; do **not** put `mode` in the read key. |
| Parse | Partly | Core AST identical; IDE adds a span sidecar. |
| Resolve | Partly | Binding *decisions* identical; IDE adds a reference-location map. |
| Type-check / exec | Partly | Pass/fail and values identical; IDE messages need the parse span sidecar. |

Rule of thumb: mode goes in the key of **derived** steps that carry metadata,
never in the key of mode-independent inputs.

## The upgrade-retry path (batch compiler)

1. **Compile everything in fast mode.** Minimal metadata → fast, small.
2. **No errors → done.** Ship the artifact; never pay for metadata. This is the
   common case and the whole point.
3. **Error(s) → upgrade only the affected subtree to IDE mode.** Fast mode
   already tells you *where* it failed (which query/node), even if it can't
   render a nice message. Re-request `parse_ide(F)`, `resolve_ide(F)`, … for the
   failing `F` and just the dependencies needed to produce the diagnostic — not
   the whole program.
4. **Render diagnostics from the IDE-mode result** (spans + source snippets +
   notes) and report.

Why the retry is cheap:

- The **read / content-hash cache is shared** (step is mode-independent), so no
  IO or hashing is repeated.
- Mode-independent core results can be reused where the pipeline is structured
  as core + sidecar; only the sidecar work is new.
- Scope is the failing subtree, not the tree.

### Error object design

- **Fast-mode error** = cheap token: an error *kind* + a *locator* (query/node
  id) sufficient to (a) conclude compilation failed and (b) drive the upgrade.
  No rendered string, no span.
- **IDE-mode error** = full: spans, source excerpt, notes, suggestions.

Today's errors (`ParseError`/`ResolveError`/`ExecuteError`, some carrying
`source_location: String`) are an awkward middle: they carry a *string* location
but no structured span. The plan splits this into the two levels above.

## IDE always-on variant

In an actual editor you don't wait for an error — you want hover/go-to-def
immediately. So the editor strategy is: **IDE mode for the open working set,
fast mode for the rest of the closure.** Same flavor mechanism, different
selection policy. The fast-first/upgrade-on-error flow is the *batch-compiler*
policy; per-file mode selection is the *IDE* policy. Both share one machinery.

## Interaction with caching & invalidation

Builds on the content-addressed / two-layer model in
`../../docs/cache-invalidation-problem.md`:

- Mode is part of the key for mode-sensitive steps → fast and IDE results are
  distinct content-store entries; no collision, both cacheable.
- **Invalidation is per position, across flavors.** A file change dirties *all*
  flavors of the affected nodes (the reverse-dep walk treats every flavor of a
  position as a dependent). You don't want a stale IDE result surviving a change
  that invalidated the fast one.
- Early cutoff still applies *within* each mode.
- Caching both flavors costs memory; the roadmap's memory/disk tiering (deferred
  for now) is the eventual mitigation. Until then, IDE-mode entries can be
  evicted aggressively since they're recomputable on demand.

## Concrete sandbox changes (when implemented)

- Add `enum Mode { Fast, Ide }`; thread it as a flavor on `ParseId` / `ResolveId`
  / `ExecId` (or via the query key at the cache layer) — **not** on the read key.
- Parse returns `Parsed { ast: PreExpr, meta: Option<ParseMeta> }` where
  `ParseMeta` holds the span table; `meta` is `Some` only in IDE mode.
- Resolve gains an optional reference-map sidecar in IDE mode.
- Split errors into `FastError { kind, locator }` and a rich `IdeError` renderer.

## Invariant + tests

- **Invariant:** for any input, `fast-error ⟺ ide-error` (same failures), and
  resolution/values are identical across modes. Mode changes only metadata.
- **Tests:**
  - Property/fuzz: compile in both modes, assert identical error sets and values.
  - Upgrade path: seed an error, assert fast mode locates it and IDE mode renders
    a span-accurate message for the *same* error.
  - Reuse: assert the upgrade re-run does **zero** extra reads (shared read
    cache) and touches only the failing subtree.

## Open questions

- Is IDE mode a *flavor of the same query* (recommended) or a thin *wrapper
  query* that calls the fast query and computes the sidecar? The wrapper is
  cleaner when the sidecar truly is additive; the flavor is needed when fast
  drops info mid-computation. Likely a mix per step.
- Mode granularity: per-file or per-function? Per-file is simpler; per-function
  gives finer IDE working sets.
- After an upgrade, keep the fast result cached or drop it? Keep both (flavors);
  let tiering/eviction decide.
