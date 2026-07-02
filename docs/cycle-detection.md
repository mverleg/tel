# Cycle Detection

## Summary

The sandbox has a **partial, resolve-only** cycle detector today. It works for
reporting a cycle after the fact, but it is not integrated with the concurrent
resolve pipeline and does not prevent the deadlock that a real import cycle can
cause under parallel resolution. This document proposes a general, deadlock-safe
approach based on carrying the in-progress ancestor path through the context.

## Current State

Two half-mechanisms exist:

1. **On-demand DFS over forward edges** (`graph.rs:63-117`):
   `find_resolve_cycle(target)` runs `dfs_find_cycle`, which walks
   `dependencies`, keeping a `visited` set and a `stack`; if it re-encounters a
   node already on the stack it reports the cycle. It only considers
   `StepId::Resolve` nodes and is invoked explicitly, not automatically.

2. **Runtime resolution state** (`context.rs:10-15, 22`):
   ```rust
   pub enum ResolutionState {
       InProgress { started_at: Instant },
       Completed,
   }
   // Global.resolution_states: DashMap<FQ, ResolutionState>
   ```
   A per-`FQ` marker intended to notice re-entry into a resolution already in
   progress. The `started_at: Instant` hints at a timeout-based fallback.

### Problems

- **Resolve-only.** `dfs_find_cycle` filters to `Resolve` nodes; cycles that
  route through other step kinds are ignored. It is also a separate, manually
  triggered pass rather than part of normal execution.
- **Deadlock under parallelism.** `resolve_all_impl` spawns tokio tasks
  (`context.rs:83-94`). A genuine import cycle `A → B → A` becomes: task(A)
  awaits resolve(B), task(B) awaits resolve(A) — a wait-for cycle. By the time a
  post-hoc DFS could run, both tasks are already parked. Detection must happen
  **before** a task awaits a dependency that is one of its own ancestors.
- **Racy DFS.** `dfs_find_cycle` reads `self.dependencies` while other tasks are
  still inserting edges, so a snapshot taken mid-build can miss or misreport
  edges.
- **Timeout smell.** `started_at: Instant` suggests "assume a cycle if it takes
  too long," which is both false-positive prone and slow to fire.

## Design

### Core idea: carry the ancestor path

Each resolution knows the chain of resolutions that led to it. Before a context
awaits (or spawns) a dependency, it checks whether that dependency is already on
its own ancestor path. If so, that *is* the cycle — report it immediately,
before awaiting, so no deadlock can form.

```rust
pub struct ResolveContext {
    current: ResolveId,
    core: &'static Global,
    // Ordered ancestor chain: Root-most first, `current` implied at the tail.
    // Cheap immutable share; each child extends by one.
    ancestors: Arc<AncestorPath>, // persistent cons-list / Arc<[FQ]>
}
```

When `resolve_all` is asked for `next`:

```rust
if self.ancestors.contains(&next.func_loc) || next.func_loc == self.current.func_loc {
    return Err(ResolveError::Cycle(self.ancestors.to_cycle(&next.func_loc)));
}
let child = self.extend(next.func_loc.clone()); // Arc-shares the prefix
```

Properties:

- **Deadlock-free**: the check happens before the await/spawn, so a
  wait-for cycle is never entered.
- **Deterministic**: no timeouts; detection is exact and independent of timing.
- **Concurrency-friendly**: the ancestor path is per-task immutable state, not
  shared mutable global state, so parallel branches with a *shared* dependency
  (diamond, not cycle) don't false-positive — they have different ancestor
  chains but the shared node simply resolves once via the cache.
- **Cheap**: `AncestorPath` is a persistent structure (`Arc` cons-list or
  `Arc<[FQ]>` copy-on-extend); depth is bounded by import nesting. `contains`
  is O(depth); for deep graphs a small `HashSet<FQ>` can shadow the list.

This replaces `resolution_states` for the *detection* role. If we still want a
"currently in progress" registry for other reasons (diagnostics, dedup), it can
stay, but it is no longer the cycle mechanism and the `Instant`/timeout can go.

### Reporting

`ResolveError::Cycle(Vec<FQ>)` carries the offending chain from the first
repeated node back to itself, matching the shape `find_resolve_cycle` already
returns, so error formatting is reusable. The existing `graph.rs` DFS can be
**kept as a diagnostic / verification tool** (e.g. a debug assertion that the
finished graph is acyclic), just demoted from the primary detector.

### Generalising beyond resolve

The ancestor-path technique is not resolve-specific. Once the query engine has a
uniform `StepId`-keyed execution path, the ancestor chain should be `Vec<StepId>`
(or `Arc<AncestorPath<StepId>>`) so parse/exec/monomorph cycles are caught by
the same code. For the current sandbox, resolve is the only phase that can
recurse into itself (imports), so a resolve-scoped version is sufficient first.

### Relationship to the inverse graph

Cycle detection needs **forward** ancestry, so it does not require the inverse
graph. But the two intersect in one place: any traversal over the *inverse*
edges (invalidation, `transitive_dependents` in `inverse-dependency-graph.md`)
must itself be cycle-safe via a `visited` set, because a cyclic dependency graph
would otherwise loop during invalidation. The guarantee here (the finished graph
is acyclic, or compilation failed with `Cycle`) is what makes those traversals
terminate; the `visited` set is defense in depth.

## Alternatives Considered

1. **Keep post-hoc DFS only** (`find_resolve_cycle`). Rejected as the primary
   mechanism: cannot prevent the parallel-resolve deadlock; only useful once the
   graph is fully built, which never happens if we deadlock first.
2. **Global wait-for graph + deadlock detector.** A watcher thread inspects
   `resolution_states` for a cycle of blocked tasks. Correct but complex,
   racy to snapshot, and fires late. The ancestor-path check is strictly simpler
   and earlier.
3. **Timeout-based** (implied by `started_at`). Rejected: false positives on
   slow-but-legal builds, and slow to detect real cycles.

## Implementation Priority

1. Add `ancestors: Arc<AncestorPath>` to `ResolveContext`; thread it through
   `resolve_all` / `resolve_all_impl` (Root starts empty).
2. Pre-await/pre-spawn ancestor check; return `ResolveError::Cycle`.
3. Reuse the existing cycle-vector formatting for the error message.
4. Demote `find_resolve_cycle` to a debug-assert / diagnostic; drop the
   `Instant` timeout from `ResolutionState` (or remove the enum if unused).
5. Tests: direct self-import `A → A`; two-node `A → B → A`; and a **diamond**
   `A → B, A → C, B → D, C → D` that must **not** be flagged.
6. Tick "Cycle detection" in `sandbox/README.md`.

## Impact

- **Before**: real import cycles can deadlock the parallel resolver; detection
  is manual, resolve-only, racy, and possibly timeout-based.
- **After**: cycles are detected deterministically before any task blocks, with
  a precise chain in the error; legitimate shared dependencies (diamonds) are
  unaffected; inverse-graph traversals are safe to write.
