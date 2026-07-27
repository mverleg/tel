# The Query Graph

<!-- TODO: review -->

## What

Every compilation step is a **query** — a named, deterministic function from
its inputs to an answer. A query never reaches for state directly; it asks the
**context** for what it needs, and the context records the edge:

```rust
// inside the body of `resolve(file)`
let ast = ctx.parse(file).await?;          // records: resolve(file) -> parse(file)
for import in ast.imports() {
    let _ = ctx.resolve(import).await?;    // records: resolve(file) -> resolve(import)
}
```

Routing every dependency through one choke point is what makes the graph
trustworthy. There is no way to consult an input without the engine learning
about it, so the recorded edge set is complete by construction rather than by
discipline.

## Query kinds and their order

Queries are grouped into **kinds** — parse, resolve, type-check, codegen — and
the kinds are totally ordered. A query may only call **downward** (an earlier
kind) or **sideways** (its own kind). It may never call upward.

This buys two things:

- **No cross-kind wait cycles.** A resolve can never end up waiting on a
  type-check that is waiting on that resolve. The only cycles possible are
  sideways ones (`resolve A → resolve B → resolve A`), which is a much smaller
  problem to solve — see [Cycle detection](08-cycle-detection.md).
- **Disjoint keyspaces.** The kind tags every identifier, so a parse key can
  never alias a type-check key. Each kind can then have its own typed store (no
  dynamic downcasting, the answer type is fixed per kind), its own eviction
  policy — parse answers are large and cheap to redo, type answers are small
  and expensive — and its own on-disk layout.

The ordering constraint is checkable exactly where it matters: at the boundary
between two keyspaces.

## Both edge directions

The graph stores each dependency **twice**: forward (`dependent → dependency`)
and reverse (`dependency → dependents`). Both directions are load-bearing and
neither is derivable cheaply from the other.

```rust
fn register_dependency(&self, dependent: StepId, dependency: StepId) {
    self.dependencies.entry(dependent).or_default().insert(dependency);
    self.dependents.entry(dependency).or_default().insert(dependent);
}
```

**Forward edges** answer *"what did this query consult?"* They are how a dirty
query is cleaned: rebuild its key from the current fingerprints of its recorded
dependencies. They are also what makes skipping a body possible at all — see
[Invalidation](04-invalidation.md#dependencies-come-from-recorded-edges).

**Reverse edges** answer *"who depends on this?"* They are the prerequisite for
everything leaf-driven:

- **Invalidation propagation** — a source file changed; mark its transitive
  dependents dirty. Impossible without them.
- **Watch mode** — the same traversal, driven by a file watcher.
- **Tooling** — "what breaks if I change this file?" is a natural editor query.

Without the reverse map, "who depends on X?" means scanning every entry of the
forward map and testing set membership: `O(V + E)` per question, repeated for
every changed file. With it, the answer costs `O(dependents)`.

The price is one extra insert and one extra id per edge — proportional to work
already being done, since every edge already passes through
`register_dependency`.

### Transitive queries must be cycle-safe

Any walk over the reverse edges needs a `visited` set, independently of whether
cycle detection exists:

```rust
/// All transitive dependents, deduplicated. Cycle-safe: a malformed graph
/// (e.g. circular imports) would otherwise loop forever.
fn transitive_dependents(&self, step: &StepId) -> HashSet<StepId>;
```

The guarantee that a *finished* graph is acyclic — either compilation succeeded
or it failed with a cycle error — is what makes such traversals terminate. The
`visited` set is defence in depth, not an optimisation.

## Consistency of the two maps

The forward and reverse inserts are two separate operations, so a concurrent
observer can momentarily see one without the other. That is acceptable under
the phase split described in
[Execution and recovery](07-execution-and-recovery.md#mutation-vs-query-phases):
edges are written during the *query* phase and traversed during the *mutation*
phase, and the two never overlap.

Making the pair atomic would not help much anyway: `register_dependency(a, b)`
touches two different keys regardless, so a single lock would require a coarser
structure than a sharded concurrent map. If invalidation is ever wanted
*concurrently with* an in-flight compile, this is the assumption to revisit
first.

> `TODO(open):` a graph library would give richer traversals for free, at the
> cost of a dependency and of fitting an external node/edge model to
> `StepId`-keyed concurrent maps. Left out while two maps suffice.

## Why not derive dependencies up front

A tempting alternative shape is to make each query declare its dependencies
*before* running: a small "gather inputs" phase that returns a dependency list,
then a restricted body that computes from exactly those. It looks like it would
make skipping the body easier.

It does not, and it costs a lot. Dependencies in a real compiler are
**dynamic** — which files `resolve` consults depends on the imports it finds by
parsing, and which functions a type-check consults depends on what the body
calls. A gather phase either duplicates that discovery (running most of the
body twice) or forces every query into a shape where its inputs are knowable
without looking at them, which is not true.

The flat form — one body that pulls dependencies inline — works because the
*recorded edges from the previous run*, not a re-run of the body, are what the
next run reuses. [Invalidation](04-invalidation.md#dependencies-come-from-recorded-edges)
shows why that is sound.
