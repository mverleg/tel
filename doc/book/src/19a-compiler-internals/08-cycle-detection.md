# Cycle Detection

<!-- TODO: review -->

## The problem

The [kind-ordering rule](02-query-graph.md#query-kinds-and-their-order)
eliminates cross-kind cycles: a resolve can never wait on a type-check that is
waiting on it. But **sideways** calls within one kind can still cycle, and one
of them is a program the user can easily write — an import cycle,
`resolve A → resolve B → resolve A`.

Under single-flight this is a **silent hang**: task A awaits B's `Pending`
claim, whose owner transitively awaits A's. That is worse than the
single-threaded symptom (a stack overflow, which at least reports itself), so it
must be detected rather than tolerated.

Crucially, detection has to happen **before a task parks**. A post-hoc walk over
the dependency graph cannot help: by the time it could run, both tasks are
already blocked and nothing will run it.

## The mechanism: carry the ancestor path

Each in-flight query knows the chain of queries that led to it. Before awaiting
or spawning a dependency, it checks whether that dependency is already on its
own ancestor path. If so, *that is the cycle* — report it immediately, before
awaiting, so the wait-for cycle is never entered.

```rust
pub struct QueryContext {
    current: StepId,
    /// Ordered ancestor chain, root-most first; `current` is implied at the
    /// tail. Cheaply shared; each child extends it by one.
    ancestors: Arc<AncestorPath>,
}
```

```rust
if self.ancestors.contains(&next) || next == self.current {
    return Err(Error::Cycle(self.ancestors.to_cycle(&next)));
}
let child = self.extend(next.clone()); // shares the prefix
```

The properties that matter:

- **Deadlock-free.** The check happens before the await, so no wait-for cycle
  can form.
- **Deterministic.** No timeouts. Detection is exact and independent of timing
  and of how tasks happen to be scheduled.
- **No false positives on diamonds.** The ancestor path is per-task immutable
  state, not shared mutable state. Two parallel branches with a *shared*
  dependency (`A → B`, `A → C`, `B → D`, `C → D`) have different ancestor
  chains, and the shared node simply resolves once through the cache.
- **Cheap.** The path is a persistent structure — a shared cons-list, or a
  copy-on-extend slice — and its depth is bounded by import nesting.
  Containment is `O(depth)`; a small shadowing hash set handles pathologically
  deep graphs.

The technique is not resolve-specific. Over a uniform step-keyed execution path
the ancestor chain is just a chain of `StepId`, so cycles in any kind are caught
by the same code.

## Cost

Detection sits on the slow path by construction:

- **Same-task re-entry** — one call chain recursing into itself, the common case
  — is `O(1)`: each `Pending` records its owning task, and re-entering a node
  you own is an immediate cycle error.
- **Cross-task cycles** are checked only immediately before *parking* on someone
  else's `Pending`, by walking the owner chain — bounded by the depth of blocked
  tasks, typically single digits. Parking already costs a waker registration and
  a scheduler round trip; the walk is noise next to it.
- **Memo hits, cache hits, and uncontended claims pay nothing at all.**

If contention ever shows up in a profile, the fallback is rustc's parallel-mode
approach: no check on await, and a cycle scan run only by a deadlock watchdog
(all workers parked, no progress). Zero cost until an actual deadlock, at the
price of delayed diagnostics.

## Reporting

The error carries the offending chain from the first repeated node back to
itself, which is what a user-facing diagnostic needs to print.

Diagnosis follows the fast/editor mode split described in
[Tooling](../18-tooling/01-compiler.md): in fast mode, detection only flags
"cycle involving query Q" and aborts the wave; the editor-mode retry
re-encounters the cycle with full metadata and produces the real diagnostic —
the cycle path with source locations. Detection is cheap and always on;
explanation is expensive and on demand.

## Rejected alternatives

1. **Post-hoc depth-first search over the finished graph.** Useful as a
   diagnostic or a debug assertion that the finished graph is acyclic, but
   useless as the primary mechanism: it cannot prevent the parallel deadlock,
   and it is only meaningful once the graph is fully built — which never happens
   if the build deadlocks first. It is also racy to snapshot while other tasks
   are still inserting edges.
2. **A global wait-for graph plus a deadlock detector.** A watcher inspects
   blocked tasks for a cycle. Correct, but complex, racy to snapshot, and fires
   late. The ancestor check is strictly simpler and earlier.
3. **Timeouts** — "assume a cycle if it takes too long". False positives on
   slow-but-legal builds, and slow to detect real cycles.

## Relationship to the reverse edges

Cycle detection needs *forward* ancestry, so it does not depend on the reverse
edge map at all. The two intersect in one place: any traversal over the reverse
edges — invalidation, transitive dependents — must itself be cycle-safe with a
`visited` set. The guarantee here (a finished graph is acyclic, or compilation
failed with a cycle error) is what makes those traversals terminate; the
`visited` set is defence in depth.
