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

## Implementation Priority

1. **Document the current limitation** (this file)
2. **Add tests that demonstrate the problem**
3. **Implement content-addressed parse caching** (low risk, high value)
4. **Add transitive validation for resolve/exec** (more complex)
5. **Add benchmarks to ensure performance is acceptable**

## Impact

**Current state:**
- Caching works correctly within a single execution
- NOT safe for incremental compilation or watch mode
- Changes require full rebuild (discard Global and start fresh)

**After fix:**
- Safe incremental compilation
- Watch mode becomes practical
- Better developer experience
