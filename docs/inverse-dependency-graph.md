# Inverse Dependency Graph

## Summary

The `Graph` currently stores only **forward** edges (`caller → callee`). Several
planned features — cache invalidation, watch mode, and incremental compilation
"from the leafs" — need the **reverse** question: *given a step, which steps
depend on it?* This document proposes adding a maintained inverse (reverse) edge
set so that question is answered in O(dependents) instead of scanning the whole
graph.

## Motivation

The reverse edges are a prerequisite for work already sketched elsewhere:

- **Invalidation propagation** (`cache-invalidation-problem.md`, Option 2): when
  a source file changes, walk *up* from the changed leaf and mark every
  transitive dependent dirty. This is impossible without reverse edges.
- **Incremental compile starting from leafs** (`sandbox/README.md` checklist):
  same traversal direction — start at a changed input and push work upward.
- **Debugging / tooling**: "what breaks if I change `Parse(foo.tel)`?" is a
  natural query for editor integration.

Without reverse edges, answering "who depends on X?" requires scanning every
entry of `dependencies` and testing membership in each `HashSet` — O(V + E) per
query, repeated for every changed leaf.

## Current State

`graph.rs` holds a single forward map:

```rust
pub struct Graph {
    dependencies: DashMap<StepId, HashSet<StepId>>,
}

pub fn register_dependency(&self, caller: StepId, callee: StepId) {
    self.dependencies.entry(caller)
        .or_insert_with(HashSet::new)
        .insert(callee);
}
```

Key observations:

- `register_dependency` is the **single choke point** for all edges — every
  dependency in the system flows through `ctx.parse` / `resolve_all` /
  `execute`, which all call it (see `context.rs`). This is the ideal place to
  populate a second map with no changes to callers.
- `StepId::Root` only ever appears as a `caller`, never a `callee`, so it will
  never be a key in the reverse map. That is fine.
- Edges are added concurrently: `resolve_all_impl` spawns tokio tasks that each
  register their own edges (`context.rs:88-92`). Any reverse-edge structure must
  be as thread-safe as the current `DashMap`.

## Design

### Data structure

Add a parallel map storing the reversed edge:

```rust
pub struct Graph {
    dependencies: DashMap<StepId, HashSet<StepId>>, // caller -> callees
    dependents:   DashMap<StepId, HashSet<StepId>>, // callee -> callers
}

pub fn register_dependency(&self, caller: StepId, callee: StepId) {
    self.dependencies.entry(caller.clone())
        .or_insert_with(HashSet::new)
        .insert(callee.clone());
    self.dependents.entry(callee)
        .or_insert_with(HashSet::new)
        .insert(caller);
}
```

This costs one extra `StepId` clone and one extra map insert per edge. `StepId`
is already `Clone` and the edge count is bounded by the dependency fan-out, so
the overhead is proportional to existing work.

### Query API

```rust
/// Direct dependents (one hop up).
pub fn get_dependents(&self, step: &StepId)
    -> Option<dashmap::mapref::one::Ref<StepId, HashSet<StepId>>>;

/// All transitive dependents, e.g. for invalidation. Deduplicated.
/// Cycle-safe: a `visited` set prevents infinite loops on back-edges.
pub fn transitive_dependents(&self, step: &StepId) -> HashSet<StepId>;
```

`transitive_dependents` is a BFS/DFS over `dependents`, guarded by a `visited`
set. Note it **must** be cycle-safe even before dedicated cycle detection lands,
because a malformed graph (e.g. circular imports) would otherwise loop forever
here — see `cycle-detection.md`.

### Consistency model

The two maps are updated in two separate `DashMap` operations, so an observer on
another thread can briefly see a forward edge without its reverse (or vice
versa). This is acceptable because the graph is **built during compilation and
only queried afterward**: invalidation and incremental traversal run once the
current run's steps have completed, by which point both maps are consistent. We
do *not* need the pair of inserts to be atomic.

If we later want to invalidate *concurrently with* an in-progress compile, this
assumption must be revisited (e.g. a single `DashMap<StepId, Edges { deps,
dependents }>` entry updated under one lock). That is out of scope here.

### Alternatives considered

1. **Derive dependents on demand** by scanning `dependencies`. Rejected: O(V+E)
   per query and repeated for every changed leaf in watch mode.
2. **Single combined entry** `DashMap<StepId, Edges>` holding both directions
   under one lock. Gives atomic pair-updates but a `register_dependency(a, b)`
   now touches two *different* keys (`a` and `b`) regardless, so it cannot be a
   single lock anyway without a coarser structure. Deferred until concurrent
   invalidation is actually needed.
3. **petgraph** or another graph crate. Rejected for now: adds a dependency
   (project rule: ask before adding deps), and the current `DashMap` approach is
   already concurrency-friendly and sufficient.

## Implementation Priority

1. Add `dependents` map + populate in `register_dependency` (mechanical).
2. Add `get_dependents` and cycle-safe `transitive_dependents`.
3. Add a test: build a small `Root → Exec → Resolve → Parse` chain and assert
   `transitive_dependents(Parse)` contains `Resolve`, `Exec`, `Root`.
4. Tick "Inverse dependency graph" in `sandbox/README.md`.
5. Consumers (invalidation propagation, incremental-from-leafs) build on top —
   tracked separately.

## Impact

- **Before**: reverse queries require a full graph scan; invalidation and
  leaf-first incremental compilation are not implementable.
- **After**: "who depends on X?" is O(dependents); unblocks
  `cache-invalidation-problem.md` Option 2 and watch mode. Cost is roughly 2×
  edge memory and one extra insert per edge during graph construction.
