# Cycle Detection Design for Tel Sandbox

## Problem Statement

The Tel sandbox compiler needs to detect cyclic dependencies during the resolve phase. Currently, circular imports (e.g., module A imports B, B imports A) are not explicitly detected, which can lead to:
- Infinite resolution loops (currently prevented by async-lazy caching, but not diagnosed)
- Confusing error messages when cycles occur
- Difficulty debugging dependency issues

### Requirements

1. **Minimal Happy Path Overhead**: The cycle detection mechanism should add negligible performance cost when no cycles exist (the common case)
2. **Complete Cycle Information**: When a cycle is detected, provide the full cycle path with all involved files/functions
3. **Concurrency Safe**: Must work correctly with parallel resolution of imports
4. **Async Compatible**: Must work with the async resolution architecture
5. **Early Detection**: Ideally detect cycles as early as possible rather than waiting for timeout or cache issues

## Background: Current Architecture

### Dependency Graph Structure
```rust
pub struct Graph {
    dependencies: DashMap<StepId, HashSet<StepId>>,
}

pub enum StepId {
    Root,
    Parse(ParseId),
    Resolve(ResolveId),
    Exec(ExecId),
}
```

### Resolution Flow
```
resolve(ResolveId)
  → ctx.parse(ParseId)                     [registers Resolve → Parse]
  → resolve_internal()
    → process_imports()                    [async/parallel]
      → ctx.resolve_all([import_ids...])   [registers Resolve → Resolve]
    → process_local_functions()
    → resolve_body()
  → register to func_registry
```

### Current Cycle Prevention
- `async_lazy::Cache` prevents redundant work
- If A→B→A, the second attempt to resolve A will wait on the same Future
- This prevents infinite recursion but doesn't provide diagnostic information

## Brainstormed Approaches

### Approach 1: Resolution State Machine

**Concept**: Track the state of each resolution in a shared map.

```rust
enum ResolutionState {
    NotStarted,
    InProgress { started_at: Instant, chain: Vec<FQ> },
    Completed { ast: Expr },
    Failed { error: ResolveError },
}

// In Global
resolution_states: DashMap<FQ, ResolutionState>
```

**Flow**:
1. Before resolving, check state:
   - `NotStarted` → Mark as `InProgress`, proceed
   - `InProgress` → **Cycle detected!** Extract chain from state
   - `Completed` → Return cached result
   - `Failed` → Return cached error

2. During resolution, maintain chain in state
3. On completion, transition to `Completed` or `Failed`

**Happy Path Overhead**:
- One DashMap insert at start: `O(1)` with lock-free read
- One DashMap update at end: `O(1)`
- **Very minimal** - just two hash lookups

**Cycle Information Quality**:
- Can reconstruct full chain from `InProgress` state
- Each state carries its resolution chain
- **Excellent** - complete path available

**Concurrency**:
- DashMap is thread-safe
- State transitions are atomic
- Works with async/parallel resolution
- **Excellent**

**Pros**:
- Simple to implement
- Minimal overhead
- Works naturally with async
- Early detection
- Complete error information

**Cons**:
- Need to carefully handle state cleanup on panics
- Chain storage adds memory overhead
- State transitions need to be correct in all error paths

**Variants**:
- **Minimal state**: Only track `InProgress` set, reconstruct chain on demand
- **Timestamp tracking**: Track how long each resolution has been in progress for debugging

---

### Approach 2: Thread-Local Resolution Chain

**Concept**: Use task-local storage to track the current resolution chain.

```rust
tokio::task_local! {
    static RESOLUTION_CHAIN: RefCell<Vec<FQ>>;
}

async fn resolve_with_cycle_check(id: ResolveId) -> Result<Expr, ResolveError> {
    RESOLUTION_CHAIN.scope(RefCell::new(vec![]), async {
        RESOLUTION_CHAIN.with(|chain| {
            if chain.borrow().contains(&id.func_loc) {
                // Cycle detected!
                return Err(ResolveError::CyclicDependency(chain.borrow().clone()));
            }
            chain.borrow_mut().push(id.func_loc.clone());
        });

        // ... actual resolution ...
    }).await
}
```

**Happy Path Overhead**:
- Stack allocation of Vec: `O(1)` per resolution
- Linear search in chain: `O(depth)` - typically small
- **Low** - stack operations are fast

**Cycle Information Quality**:
- Chain is immediately available
- **Excellent** - complete path

**Concurrency**:
- Task-local means each async task has its own chain
- **Problem**: Parallel imports spawn separate tasks, lose parent chain
- Would need explicit chain passing across task boundaries
- **Poor** without modifications

**Pros**:
- Very fast in single-threaded/single-task case
- No global state contention
- Chain is naturally available

**Cons**:
- Doesn't work well with parallel resolution (tokio::spawn)
- Need to explicitly propagate chain across task boundaries
- More complex with async

**Verdict**: Not suitable for current architecture with parallel imports.

---

### Approach 3: Graph-Based Cycle Detection (Post-Resolution)

**Concept**: After each resolve completes, analyze the dependency graph for cycles.

```rust
impl Graph {
    pub fn detect_cycles_from(&self, start: StepId) -> Option<Vec<StepId>> {
        // Tarjan's algorithm or DFS with visited set
        // Find strongly connected components
    }
}
```

**Flow**:
1. Allow normal resolution with dependency registration
2. After resolution completes, check for cycles in subgraph
3. If cycle found, return error with full cycle path

**Happy Path Overhead**:
- Zero overhead during resolution
- Graph traversal only on completion: `O(V + E)` where V = resolved nodes, E = edges
- Can be skipped if resolution failed for other reasons
- **Very low** - only pays cost when explicitly checking

**Cycle Information Quality**:
- Can find all cycles using SCC algorithms
- Can provide multiple cycles if they exist
- **Excellent** - complete graph analysis possible

**Concurrency**:
- No impact on concurrent resolution
- Graph analysis can be done after resolution
- **Excellent**

**Pros**:
- Zero cost during resolution
- Can find all cycles, not just first one
- Can provide comprehensive dependency analysis
- Works perfectly with existing async architecture

**Cons**:
- **Late detection** - cycles found after resolution completes or times out
- Doesn't prevent wasted work if cycle exists
- Need to handle case where resolution hangs due to cycle

**Variants**:
- **Incremental checking**: Check for cycles periodically during long-running resolutions
- **Hybrid**: Combine with state machine for early detection

---

### Approach 4: Dependency Graph Traversal (Eager)

**Concept**: Before adding each dependency edge, check if it would create a cycle.

```rust
impl Graph {
    pub fn register_dependency_checked(
        &self,
        caller: StepId,
        callee: StepId
    ) -> Result<(), CycleError> {
        // Check if adding this edge would create a cycle
        if self.would_create_cycle(&caller, &callee) {
            let cycle_path = self.find_cycle_path(&caller, &callee);
            return Err(CycleError::new(cycle_path));
        }

        self.dependencies.entry(caller)
            .or_insert_with(HashSet::new)
            .insert(callee);
        Ok(())
    }

    fn would_create_cycle(&self, from: &StepId, to: &StepId) -> bool {
        // Check if there's a path from 'to' to 'from'
        // If yes, adding 'from' → 'to' would create a cycle
        self.has_path(to, from)
    }
}
```

**Happy Path Overhead**:
- Graph traversal on every dependency registration: `O(V + E)` per edge
- With N imports and depth D, this is `O(N * (V + E))`
- **High** - potentially expensive for large dependency graphs

**Cycle Information Quality**:
- Can compute exact cycle path when detected
- **Excellent**

**Concurrency**:
- Need to lock graph during check-and-insert (TOCTOU problem)
- DashMap makes this tricky
- **Challenging** - would need additional synchronization

**Pros**:
- Immediate detection when cycle would be created
- Prevents adding cyclic edge to graph

**Cons**:
- Expensive in happy path
- Requires locking for atomicity
- Redundant checks (same paths checked multiple times)

**Verdict**: Too expensive for happy path requirement.

---

### Approach 5: Lazy Detection with Metadata

**Concept**: Track minimal state during resolution, perform detailed analysis only when needed.

```rust
// Minimal state tracking
struct ResolveMetadata {
    state: AtomicU8,  // 0=NotStarted, 1=InProgress, 2=Done
    started_at: Instant,
}

// In Global
resolve_metadata: DashMap<FQ, ResolveMetadata>

async fn resolve(id: ResolveId) -> Result<Expr, ResolveError> {
    let metadata = self.global.resolve_metadata.entry(id.func_loc.clone())
        .or_insert_with(|| ResolveMetadata::new());

    match metadata.state.compare_exchange(0, 1, ...) {
        Ok(_) => {
            // First to resolve this FQ
            // ... actual resolution ...
        }
        Err(1) => {
            // Someone else is resolving - possible cycle!
            // Do expensive cycle detection now
            let cycle = self.graph.find_cycle_involving(&id.func_loc)?;
            return Err(ResolveError::Cycle(cycle));
        }
        Err(2) => {
            // Already resolved, return cached
        }
    }
}
```

**Happy Path Overhead**:
- Atomic compare-exchange: `O(1)`, very fast
- Minimal memory per resolution (just state + timestamp)
- **Minimal** - just atomic operations

**Cycle Information Quality**:
- Lazy computation on suspicion
- Can do full graph analysis when needed
- **Excellent** when triggered

**Concurrency**:
- Atomic operations are thread-safe
- Graph analysis is read-only when detecting
- **Excellent**

**Pros**:
- Best of both worlds: fast happy path, detailed error path
- Natural fit with async
- Minimal memory footprint

**Cons**:
- Slightly more complex implementation
- Need both fast path and slow path code

**Verdict**: Strong candidate - balances all requirements well.

---

### Approach 6: Resolution Timeout with Analysis

**Concept**: Set timeout for resolutions, analyze graph if timeout occurs.

```rust
async fn resolve(id: ResolveId) -> Result<Expr, ResolveError> {
    match timeout(Duration::from_secs(10), resolve_internal(id)).await {
        Ok(result) => result,
        Err(_timeout) => {
            // Timeout - likely a cycle
            let analysis = self.graph.analyze_resolution_deadlock(&id);
            Err(ResolveError::PossibleCycle(analysis))
        }
    }
}
```

**Happy Path Overhead**:
- Timeout wrapping: negligible
- **Minimal**

**Cycle Information Quality**:
- Full graph analysis available on timeout
- Can provide detailed diagnostic
- **Good** but not certain (could be legitimate slow resolution)

**Concurrency**:
- No issues
- **Excellent**

**Pros**:
- Zero overhead in happy path
- Natural fallback for detection
- Good diagnostic information

**Cons**:
- **Late detection** - must wait for timeout
- False positives possible (slow resolution isn't always a cycle)
- Wasted work before timeout triggers

**Verdict**: Good as a fallback/safety net, not as primary detection.

---

### Approach 7: Hybrid State Machine + Graph Analysis

**Concept**: Combine Approach 1 (state machine) with Approach 3 (graph analysis).

```rust
// Fast path: State machine for immediate cycle detection
enum ResolutionState {
    InProgress { started_at: Instant },
    Completed { ast: Expr },
}

async fn resolve(id: ResolveId) -> Result<Expr, ResolveError> {
    // Quick check: am I already being resolved?
    if let Some(state) = self.global.resolution_states.get(&id.func_loc) {
        if matches!(state.value(), ResolutionState::InProgress { .. }) {
            // Potential cycle - do expensive analysis
            return self.analyze_and_report_cycle(&id).await;
        }
    }

    // Mark as in progress
    self.global.resolution_states.insert(
        id.func_loc.clone(),
        ResolutionState::InProgress { started_at: Instant::now() }
    );

    // ... actual resolution ...
}

async fn analyze_and_report_cycle(&self, id: &ResolveId) -> Result<Expr, ResolveError> {
    // Do full graph traversal to find exact cycle path
    let cycle_path = self.graph.find_cycle_containing(&id.func_loc)?;

    // Collect metadata for each step in cycle
    let detailed_info = cycle_path.iter()
        .map(|fq| {
            let state = self.resolution_states.get(fq).unwrap();
            CycleStepInfo {
                location: fq.clone(),
                in_progress_duration: state.started_at.elapsed(),
            }
        })
        .collect();

    Err(ResolveError::CyclicDependency {
        cycle: cycle_path,
        details: detailed_info,
    })
}
```

**Happy Path Overhead**:
- One DashMap lookup: `O(1)`
- One DashMap insert: `O(1)`
- **Minimal**

**Cycle Information Quality**:
- Full cycle path reconstruction
- Timing information for debugging
- Can show which resolutions are blocked
- **Excellent**

**Concurrency**:
- DashMap handles concurrent access
- Graph analysis is read-only
- **Excellent**

**Pros**:
- Fast detection (immediate on re-entry)
- Complete information (via graph analysis)
- Clean separation of fast and slow paths
- Good error messages

**Cons**:
- Two code paths to maintain
- Slightly more complex

**Verdict**: Excellent balance - recommended approach.

---

## Comparison Matrix

| Approach | Happy Path | Cycle Info | Concurrency | Early Detect | Complexity |
|----------|-----------|------------|-------------|--------------|------------|
| 1. State Machine | ★★★★★ | ★★★★☆ | ★★★★★ | ★★★★★ | ★★★☆☆ |
| 2. Thread-Local | ★★★★☆ | ★★★★★ | ★☆☆☆☆ | ★★★★★ | ★★★★☆ |
| 3. Post-Resolution | ★★★★★ | ★★★★★ | ★★★★★ | ★☆☆☆☆ | ★★★☆☆ |
| 4. Eager Graph Check | ★☆☆☆☆ | ★★★★★ | ★★☆☆☆ | ★★★★★ | ★★★☆☆ |
| 5. Lazy Metadata | ★★★★★ | ★★★★☆ | ★★★★★ | ★★★★☆ | ★★★★☆ |
| 6. Timeout | ★★★★★ | ★★★☆☆ | ★★★★★ | ★☆☆☆☆ | ★★★★★ |
| 7. Hybrid | ★★★★★ | ★★★★★ | ★★★★★ | ★★★★★ | ★★★☆☆ |

---

## Recommended Approach: Hybrid State Machine + Graph Analysis (Approach 7)

### Why This Approach?

1. **Optimal Happy Path Performance**:
   - Single DashMap lookup + insert
   - No traversals, no complex state management
   - Essentially free for non-cyclic code

2. **Complete Cycle Information**:
   - Full graph traversal when cycle suspected
   - Can show exact dependency chain
   - Can include timing and metadata

3. **Concurrency Safe**:
   - DashMap handles concurrent access
   - No task-local state
   - Works with parallel resolution

4. **Early Detection**:
   - Detects cycle on first re-entry attempt
   - No need to wait for timeout
   - Fails fast with clear error

### Implementation Sketch

```rust
// In global.rs
pub struct Global {
    graph: Graph,
    parse_cache: Cache<ParseId, PreExpr, ParseError>,
    func_registry: DashMap<FQ, FuncData>,
    resolution_states: DashMap<FQ, ResolutionState>,  // NEW
}

#[derive(Clone)]
pub enum ResolutionState {
    InProgress { started_at: Instant },
    Completed,
}

// In context.rs
impl ResolveContext {
    pub async fn resolve(&self, id: ResolveId) -> Result<Expr, ResolveError> {
        // Fast path: check if already in progress (potential cycle)
        if let Some(entry) = self.global.resolution_states.get(&id.func_loc) {
            if matches!(entry.value(), ResolutionState::InProgress { .. }) {
                // Suspected cycle - do full analysis
                return self.detect_and_report_cycle(&id);
            }
            // If Completed, fall through to cache check
        }

        // Mark as in progress
        self.global.resolution_states.insert(
            id.func_loc.clone(),
            ResolutionState::InProgress { started_at: Instant::now() }
        );

        // Register dependency
        self.register(id.clone(), ParseId { file_path: id.func_loc.path.clone() });

        // Actual resolution
        let result = resolve_internal(&self, id.clone()).await;

        // Mark as completed (whether success or failure)
        self.global.resolution_states.insert(
            id.func_loc.clone(),
            ResolutionState::Completed
        );

        result
    }

    fn detect_and_report_cycle(&self, id: &ResolveId) -> Result<Expr, ResolveError> {
        // Find cycle path in graph
        let cycle_path = self.global.graph.find_resolve_cycle(&id.func_loc)?;

        // Gather metadata for each step
        let cycle_details: Vec<CycleStepInfo> = cycle_path.iter()
            .filter_map(|fq| {
                self.global.resolution_states.get(fq).map(|state| {
                    match state.value() {
                        ResolutionState::InProgress { started_at } => {
                            CycleStepInfo {
                                location: fq.clone(),
                                state: "in_progress".to_string(),
                                duration: started_at.elapsed(),
                            }
                        }
                        ResolutionState::Completed => {
                            CycleStepInfo {
                                location: fq.clone(),
                                state: "completed".to_string(),
                                duration: Duration::ZERO,
                            }
                        }
                    }
                })
            })
            .collect();

        Err(ResolveError::CyclicDependency {
            cycle: cycle_path,
            details: cycle_details,
        })
    }
}

// In graph.rs
impl Graph {
    /// Find a cycle containing the given FQ in Resolve dependencies
    pub fn find_resolve_cycle(&self, target: &FQ) -> Result<Vec<FQ>, GraphError> {
        // Convert FQ to ResolveId
        let target_id = StepId::Resolve(ResolveId { func_loc: target.clone() });

        // DFS to find cycle
        let mut visited = HashSet::new();
        let mut stack = Vec::new();

        if self.dfs_find_cycle(&target_id, &mut visited, &mut stack) {
            // Extract FQs from cycle
            let cycle_fqs = stack.iter()
                .filter_map(|step| match step {
                    StepId::Resolve(rid) => Some(rid.func_loc.clone()),
                    _ => None,
                })
                .collect();
            Ok(cycle_fqs)
        } else {
            Err(GraphError::NoCycleFound)
        }
    }

    fn dfs_find_cycle(
        &self,
        current: &StepId,
        visited: &mut HashSet<StepId>,
        stack: &mut Vec<StepId>,
    ) -> bool {
        if stack.contains(current) {
            // Found cycle - current is in stack
            stack.push(current.clone());
            return true;
        }

        if visited.contains(current) {
            return false;
        }

        visited.insert(current.clone());
        stack.push(current.clone());

        if let Some(deps) = self.dependencies.get(current) {
            for dep in deps.iter() {
                if self.dfs_find_cycle(dep, visited, stack) {
                    return true;
                }
            }
        }

        stack.pop();
        false
    }
}
```

### Error Message Example

```
Error: Cyclic dependency detected

Cycle:
  1. /path/to/moduleA.telsb:moduleA
     └─ imports moduleB (in progress for 125ms)
  2. /path/to/moduleB.telsb:moduleB
     └─ imports moduleC (in progress for 89ms)
  3. /path/to/moduleC.telsb:moduleC
     └─ imports moduleA (in progress for 45ms) ← cycle completes here

To fix: Remove one of the import dependencies above.
```

---

## Alternative Considerations

### Variation: Minimal State (Trade-off on Error Quality)

If memory is a concern, use even simpler state:

```rust
// Just track what's currently being resolved
in_progress_resolutions: DashSet<FQ>

// Check on entry
if !self.global.in_progress_resolutions.insert(id.func_loc.clone()) {
    // Already in set - cycle!
    return self.detect_and_report_cycle(&id);
}

// Remove on exit (use RAII guard)
```

Pros: Slightly less memory (no timestamp)
Cons: Can't show how long resolutions have been stuck

### Variation: Separate Cycle Cache

For performance-critical applications, separate concerns:

```rust
// Fast lookup for cycle detection
cycle_detection_set: DashSet<FQ>

// Detailed metadata only when needed
resolution_metadata: DashMap<FQ, ResolutionMetadata>
```

Pros: DashSet is faster than DashMap for contains() checks
Cons: Two structures to maintain

---

## Future Enhancements

### 1. Cycle Type Classification

Distinguish between different cycle types:
- **Direct cycles**: A imports B imports A
- **Transitive cycles**: A imports B imports C imports A
- **Self-cycles**: A imports A (if ever possible)

### 2. Suggested Fixes

Analyze cycle and suggest potential fixes:
```
Cycle detected: moduleA → moduleB → moduleC → moduleA

Suggested fixes:
  1. Extract shared functionality to a new module moduleD
  2. Merge moduleB and moduleC if they're tightly coupled
  3. Use dependency injection for one of the imports
```

### 3. Graph Visualization

Generate DOT format for cycle visualization:
```rust
pub fn export_cycle_dot(&self, cycle: &[FQ]) -> String {
    // Generate GraphViz DOT format
}
```

### 4. Cycle Metrics

Track cycle detection statistics:
- Number of cycles encountered
- Most common cycle patterns
- Average cycle depth

### 5. Partial Cycle Information in Cache

When resolution fails mid-way, preserve partial information:
```rust
enum ResolutionState {
    InProgress { started_at: Instant, imports_resolved: Vec<FQ> },
    // ...
}
```

This helps debugging even if the full cycle analysis fails.

---

## Testing Strategy

### Unit Tests

1. **Simple cycle**: A imports B imports A
2. **Transitive cycle**: A imports B imports C imports A
3. **Multiple cycles**: A→B→A and C→D→C
4. **Deep cycle**: A→B→C→D→E→A
5. **Concurrent cycle**: Multiple threads hit same cycle
6. **False positive**: Long but non-cyclic chain

### Integration Tests

1. Create sample .telsb files with cycles
2. Verify error message format
3. Check that cycle path is correct
4. Ensure no false positives on large codebases

### Performance Tests

1. Benchmark overhead on non-cyclic resolutions
2. Measure cycle detection time for various cycle depths
3. Test with parallel imports hitting cycles

---

## Implementation Phases

### Phase 1: Basic State Tracking
- Add `ResolutionState` to `Global`
- Implement state check in `resolve()`
- Simple error on re-entry detection

### Phase 2: Graph Analysis
- Implement `find_resolve_cycle()` in `Graph`
- DFS-based cycle detection
- Basic error message with cycle path

### Phase 3: Rich Diagnostics
- Add timing information
- Improve error message formatting
- Add file location context

### Phase 4: Testing & Refinement
- Comprehensive test suite
- Performance benchmarking
- Error message polish

---

## Open Questions

1. **Should we detect all cycle types or just Resolve cycles?**
   - Could also detect Parse→Parse or Exec→Exec cycles
   - Resolve cycles are most likely and important

2. **Should cycle detection be optional/configurable?**
   - Could add `--no-cycle-check` flag for performance
   - Probably not needed given minimal overhead

3. **How to handle cycles discovered during parallel resolution?**
   - Multiple threads may discover same cycle
   - Should we deduplicate cycle reports?

4. **Should we cache cycle detection results?**
   - Once a cycle is found, cache it to avoid re-detection
   - Useful if user doesn't fix immediately and retries

5. **Integration with parse cache?**
   - Currently parse uses `async_lazy::Cache`
   - Should resolution use similar caching?
   - How do caching and cycle detection interact?

---

## Conclusion

The **Hybrid State Machine + Graph Analysis** approach provides the best balance:
- Minimal overhead in the common (non-cyclic) case
- Complete diagnostic information when cycles occur
- Natural fit with the async architecture
- Early detection for fast failure

Implementation complexity is moderate and well-contained within the existing architecture.
