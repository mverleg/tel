# Cache Invalidation Problem

## Summary

The current caching implementation in the sandbox compiler has a critical flaw: it caches results permanently without checking transitive dependencies, leading to stale data when upstream inputs change.

## The Problem

### Current Behavior

The system tracks dependencies in a `Graph` but doesn't use them for cache invalidation:

1. **Parse results are cached permanently** (`context.rs:48-55`)
   - `parse_cache.get(id)` returns the same result forever for a given `ParseId`
   - No mechanism to invalidate when source files change

2. **Dependencies are tracked but unused for validation** (`graph.rs:51-54`)
   - `Graph::register_dependency()` records relationships
   - These dependencies are never consulted when retrieving cached values

3. **No transitive dependency checking**
   - When retrieving a cached value, we don't verify upstream dependencies are still valid

### Concrete Example

Given this dependency chain:
```
exec A → resolve B → parse B
```

**Scenario:**
1. Initial execution:
   - parse B reads `file.tel` (version 1)
   - resolve B processes parse B's result
   - exec A uses resolve B's result
   - All results cached

2. Source file changes:
   - User modifies `file.tel` (version 2)

3. Re-execution of exec A:
   - exec A checks dependencies, sees resolve B
   - resolve B recalculates, calls `ctx.parse(B)`
   - **parse B cache hit** → returns stale v1 data!
   - resolve B uses old parse result
   - exec A uses old resolve result

**Result:** The system silently uses stale data despite tracking all dependencies.

## Root Cause Analysis

### Cache Design
The `Cache` (`async-lazy/src/cache.rs`) is designed to be:
- Append-only (no removal)
- Permanent (no invalidation)
- Simple (no staleness checking)

This works fine for immutable data, but compilation inputs change.

### Missing Validation
When serving a cached result, we need to verify:
1. Direct dependencies haven't changed
2. **Transitive dependencies** haven't changed (the critical missing piece)

Example:
```rust
// Current: Only checks immediate cache hit
let result = self.parse_cache.get(id, move || async move {
    crate::parse::parse(&ctx, id_for_init).await
}).await;

// Missing: Should check if any upstream dependencies changed
// (In parse's case: file content. In resolve's case: parse results)
```

## Current Assumptions

The system currently assumes:
- All inputs are immutable during a session
- Once computed, results never become stale
- The `Graph` is for analysis/debugging only, not cache validation

This makes the cache **correct but not useful** for incremental compilation across changes.

## Potential Solutions

### Option 1: Transitive Dependency Validation

When retrieving cached values, recursively check all upstream dependencies:

```rust
async fn is_valid(&self, step: StepId) -> bool {
    if let Some(deps) = self.graph.get_dependencies(&step) {
        for dep in deps {
            if !self.is_valid(dep).await {
                return false;
            }
        }
    }
    // Check if this step's inputs changed
    self.check_inputs_unchanged(step).await
}
```

**Pros:**
- Correctly handles transitive dependencies
- Works with existing dependency graph

**Cons:**
- Requires tracking input "versions" (file hashes, timestamps)
- Recursive checking could be expensive
- Complex to implement correctly

### Option 2: Invalidation Propagation

When inputs change, walk the dependency graph and invalidate all dependent steps:

```rust
fn invalidate(&self, step: StepId) {
    self.cache.remove(step);
    // Find all steps that depend on this one
    for dependent in self.graph.find_dependents(&step) {
        self.invalidate(dependent); // Recursive
    }
}
```

**Pros:**
- Eager invalidation is easier to reason about
- No runtime validation overhead

**Cons:**
- Requires maintaining reverse dependency edges
- Needs mutable cache (breaks append-only design)
- May invalidate more than necessary

### Option 3: Content-Addressed Caching

Include input "fingerprints" in cache keys:

```rust
pub struct ParseId {
    pub file_path: Path,
    pub content_hash: Hash,  // NEW
}
```

**Pros:**
- Automatically handles changes (different hash = different key)
- No explicit invalidation needed
- Simple and correct

**Cons:**
- Need to hash inputs (files, etc.)
- Cache grows without bound (no reuse of same key)
- Doesn't work well for mutable operations

### Option 4: Source Tracking + Validation

Store input metadata with cached results and validate on retrieval:

```rust
struct CachedParse {
    result: PreExpr,
    file_modified_time: SystemTime,
}

// On cache hit:
if cached.file_modified_time != current_modified_time {
    // Invalidate and recompute
}
```

**Pros:**
- Works with existing cache structure
- Only checks immediate inputs (fast)

**Cons:**
- **Doesn't handle transitive dependencies** (same problem as current system)
- Requires file system access on every cache check

## Recommended Approach

**Hybrid: Content-addressing for parse + transitive validation for resolve/exec**

1. **Parse step**: Use content-addressed caching
   - Hash file contents when creating `ParseId`
   - Changed files automatically get new cache keys
   - Old parse results can be garbage collected

2. **Resolve/Exec steps**: Use transitive validation
   - Before returning cached result, check if dependencies changed
   - Walk dependency graph to verify upstream steps
   - Recompute if any dependency is invalid

This combines:
- Simplicity of content-addressing for file inputs
- Correctness of transitive checking for derived results
- Efficiency of caching when nothing changed

## Change Scenarios

The options above are about *correctness*. But two common edit patterns also
determine how *useful* the cache is, and they need mechanisms beyond what the
"Recommended Approach" above spells out. Both are currently unsupported (there
is no cross-run cache at all — `run_file` leaks a fresh `Global` per
invocation, `lib.rs:92`), and neither was previously documented.

### Scenario A: revert / branch-switch (edit, then restore identical content)

> Compile, add a line, compile, revert the line, compile again. Also happens
> constantly when switching git branches back and forth.

This is a hit **iff cache keys are content-addressed** (Option 3), *not*
timestamp-based (Option 4):

- **Content hash** — reverting restores byte-identical content → identical
  hash → identical `ParseId` key → **cache hit**. The intermediate version
  simply lived under a different key.
- **mtime / timestamp (Option 4)** — git checkout and revert rewrite the file,
  so `mtime` changes even when the bytes are identical → the timestamp scheme
  **misses** the cache on exactly the revert/branch-switch case. This is a
  concrete reason to reject Option 4 for file inputs, not just the transitive
  weakness already noted.

Cost: content-addressing still requires reading + hashing the file each compile
(to know which key to look up), so it saves the *parse computation*, not the
*file read*. Reuse of a key that recurs (revert) is the whole point here, which
also tempers Option 3's "cache grows without bound" con — recurring content
reuses its slot.

Note: the retired `qcompiler` prototype raised branch-switching as an open
question ("keep old cache? how to detect which is correct quickly?"), framed as
storing multiple answers per query. Content-addressing dissolves that question for the
parse layer: the correct answer is simply the entry whose key matches the
current content hash; no "detect which is correct" step is needed.

### Scenario B: early cutoff (semantically-irrelevant edit in a deep leaf)

> Add a blank line (or reformat) deep in a leaf dependency. Ideally that file
> re-parses, the result is seen to be the same, and the rest of the tree stays
> cached.

This is the **early-cutoff** optimization (Salsa/Adapton terminology). It does
**not** fall out of content-addressing — in fact naive content-addressing
defeats it:

- Blank line → file bytes change → new content hash → **parse must re-run.**
  Correct and unavoidable; the input changed.
- If the resolve/exec keys (or the transitive-validation check) incorporate the
  parse *input* hash, then a new hash invalidates them → resolve re-runs →
  **cascades all the way up. No cutoff.**

To stop the propagation you need **output-value comparison**, a mechanism
distinct from input keying:

> re-run parse, then compare the *new* result to the *cached* result. If equal,
> declare parse's **output** unchanged at this revision and stop — downstream
> steps keyed/validated on the parse *result* (not its source bytes) get a hit.

The "Recommended Approach" under-specifies this: its transitive validation
(Option 1) checks whether a dependency *step* is valid, but with
content-addressed parse keys the step's key changes on a blank-line edit, so
that check alone yields no cutoff. Early cutoff specifically requires the
Salsa-style "recompute → compare output → mark unchanged-at-revision" step,
plus keying downstream results on the *output digest* of their inputs rather
than the input source. The retired `qcompiler` prototype stated this behavior as
a goal ("even if a source leaf changed, if that doesn't change the answer... then
dependencies of that aren't executed") but not the mechanism.

**Feasibility for Tel specifically:** the parse AST carries no source spans or
line numbers (`PreExpr`/`Expr` in `types.rs`), so a blank line genuinely
produces an equal AST — early cutoff is achievable here, which is not true for
compilers whose AST nodes carry spans. **Caveat:** `Panic`/`Unreachable` nodes
do carry a `source_location: String` (`types.rs:25-26`); if that string encodes
a line number, an edit *above* such a node changes that function's AST and
defeats cutoff for it. Confirm what goes into that string before relying on
whitespace-insensitivity.

### Summary

| Scenario | Solved by content-addressed keys? | Extra piece needed |
|---|---|---|
| A: revert / branch-switch | Yes — use content hash, **not** mtime | — |
| B: blank line in leaf (early cutoff) | No — still cascades | output-digest comparison; downstream keyed on parse *result*, not source bytes |

## Committed Approach

We commit to **content-based caching** as the general direction (Option 3 for
inputs + output-digest comparison for derived results). Two decisions and one
mental model pin this down.

### Read and parse stay fused

Read (`tokio::fs::read_to_string`, `parse.rs:300`) stays *inside* parse; we do
**not** split IO into its own cache layer. Consequence: there is no separate
"stop after read" cutoff — an identical file with a new mtime is re-read and
re-parsed. That is fine and cheap; the meaningful cutoff is **after parse**
(whitespace-only edit → equal AST → downstream cut off). The earlier idea of a
read-layer cutoff is dropped.

### Two layers — only one is fragile

"Content-based cache" is really two layers, and it matters which is which:

1. **Content store** — `digest → result`. Immutable, append-only. Never
   invalidated: a digest always maps to the same correct result, or is absent →
   recompute. This layer is **panic- and error-safe by construction** — it
   cannot be corrupted into returning a wrong answer. Cached *errors* live here
   too (see below).
2. **Binding / memo layer** — `logical position → the digest current for it`
   (e.g. "what digest does `ExecId(main)` resolve to right now?"). This is the
   mutable, incremental, walked-from-leaves layer. **This is the only state that
   can be left torn.**

This reconciles the two commitments that look like they conflict:
content-addressing makes the *store* safe; "invalidate from the leaves" is an
operation on the *binding layer*. Everything about invalidation and partial
failure below is about the binding layer only.

### Two mechanisms, different jobs

- **File watcher + reverse-dependency walk** answers *where to look*: it hands
  you the changed leaf set and bounds the recheck to that leaf's cone via
  `graph.transitive_dependents` (`graph.rs:78`) — so watch mode never
  re-demands the root or re-hashes every file to find the change.
- **Content / output digests** answer *how far the change actually travels* —
  they are the per-layer stop conditions (Scenario B).

The reverse walk **scopes** the work to the affected cone; the digests **cut it
short** within that cone. Pure content-addressing without a watcher is also
correct (stale entries just stop being looked up), but you'd pay a full
top-down re-derivation to discover the change. The watcher is the efficiency
half; the digests are the correctness-plus-cutoff half.

## Consistency Under Partial Failure

Incremental compilation starts at a changed leaf and works *up* toward the root
(the whole point — never walk the whole tree down from root). So we must answer:
if an **error** or **panic** interrupts that upward pass partway, is the system
left in a state where the *next* change still re-triggers the whole affected
chain — without falling back to a root-down walk?

### The invariant

> A position is marked **clean/resolved** only as the atomic final step of a
> *successful* recompute — and never while any of its inputs is
> dirty/unknown/errored.

Hold this and any failure can only ever leave a position **dirty**, never
falsely-clean. Falsely-clean is the one dangerous state: it is what makes a
future *leaf-local* walk skip a stale node and never reach the root. Dirty is
always safe — it just means "recheck me."

### Guaranteeing it: front-load the invalidation (two passes)

Split the upward pass:

1. **Pass 1 — invalidate (infallible).** Walk reverse edges from the changed
   leaf and mark the **entire** affected cone dirty
   (`graph.transitive_dependents`, `graph.rs:78`). Pure flag-setting, no user
   computation — it cannot raise a resolve error and is extremely unlikely to
   panic.
2. **Pass 2 — recompute (fallible).** Recompute dirty nodes bottom-up, clearing
   dirty **only on successful commit**, with early cutoff on digest match.

An error or panic mid-recompute leaves every not-yet-recomputed node **still
dirty**, because Pass 1 already marked the whole cone and Pass 2 only ever
*removes* dirt. The next demand resumes on whatever is still dirty; no
root-down walk is needed.

This is the clean form of "invalidate the remainder on error": by doing all
invalidation up front, unconditionally, there is **no fragile error path** to
get wrong. If instead you *fuse* invalidate+recompute into a single upward walk,
then you genuinely must invalidate the remaining cone on failure — otherwise
nodes above the failure stay falsely-clean and a later leaf-local walk skips
them, silently serving a stale root.

Properties worth noting:
- **Still incremental.** Pass 1 touches only reverse *edges* of the cone
  (cheap); Pass 2 is still cut short by digests. Even when the cone is large (a
  widely-imported leaf), no expensive work runs over the whole tree and nothing
  walks down from root.
- **Dirtiness is monotonic.** A second leaf change unions its cone into whatever
  is still dirty, so leftover dirt from a failed pass persists safely until
  something recomputes it.

### Errors vs panics

- **Errors (the common case)** are *values* (`Result::Err`). Cache them in the
  content store like any result: `digest → Err(...)`. Same input → same error,
  so a failed resolve is content-addressable and participates in early cutoff
  (unchanged error output → cut off, don't re-derive dependents). The error
  propagates *up as an error-valued result*; the chain continues carrying `Err`.
  Errors therefore never tear state — treat `Err` as a first-class cacheable
  output, not as an "abort."
- **Panics** are *not* values — they unwind and can interrupt mid-mutation. Two
  cheap defenses, both upholding the invariant:
  1. `catch_unwind` at the recompute boundary → convert a panic into "this node
     = dirty/unknown," never clean.
  2. Make "commit result + mark clean" the single atomic last step of a
     successful recompute, so a panic *before* commit is indistinguishable from
     "never ran" and leaves the node dirty.

  With the two-pass split, that is all that's required: Pass 1's marking stands,
  and the panicking node stays dirty.

**One-line answer:** mark the whole cone dirty *before* any fallible work, and
clear dirty *only* on successful commit. Then neither an error nor a panic can
produce a falsely-clean node, so there is nothing to repair with a root-down
walk.

## Implementation Priority

1. **Document the current limitation** (this file)
2. **Add tests that demonstrate the problem**
3. **Implement content-addressed parse caching** (low risk, high value) — also
   delivers Scenario A (revert/branch-switch); ensure keys are content hashes,
   not mtime
4. **Add transitive validation for resolve/exec** (more complex)
5. **Add output-digest comparison for early cutoff** (Scenario B) — recompute,
   compare result to cached, stop propagation when unchanged; key downstream
   results on input *output* digest, not source bytes
6. **Separate the content store from the binding layer** — immutable
   `digest → result` store (errors included) vs mutable
   `position → current digest` memo; only the latter carries dirty state
7. **Two-pass invalidation for watch mode** — Pass 1 marks the whole
   `transitive_dependents` cone dirty (infallible); Pass 2 recomputes bottom-up
   and clears dirty only on successful commit. Guarantees consistency under
   partial failure without a root-down walk
8. **Make recompute panic-safe** — `catch_unwind` at the recompute boundary
   marks the node dirty (never clean); "commit result + mark clean" is the
   single atomic last step
9. **Add tests for both change scenarios** — Scenario A asserts a revert hits
   the parse cache; Scenario B asserts a blank-line edit re-parses one file but
   performs zero resolve/exec recomputation
10. **Add a partial-failure test** — inject an error/panic partway up an
    incremental leaf→root pass, then assert a subsequent change still recomputes
    the full affected chain (no falsely-clean node survives)
11. **Add benchmarks to ensure performance is acceptable**

## Impact

**Current state:**
- Caching works correctly within a single execution
- NOT safe for incremental compilation or watch mode
- Changes require full rebuild (discard Global and start fresh)

**After fix:**
- Safe incremental compilation
- Watch mode becomes practical (file watcher + leaf→root cone invalidation)
- Revert / branch-switch and whitespace-only edits reuse the cache
- Consistent across partial failures — an error or panic mid-invalidation never
  leaves a falsely-clean node, so no root-down rebuild is needed
- Better developer experience
