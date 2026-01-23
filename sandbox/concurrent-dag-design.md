# Concurrent Bidirectional DAG Design

## Goal

Replace the current mutex-protected `Vec<Dependency>` with a concurrent bidirectional graph structure that supports:
1. Lock-free node lookups
2. Efficient parent/child queries
3. Thread-safe concurrent insertions
4. Grow-only semantics (no deletions)

## Current Implementation

```rust
struct ExecutionLog {
    dependencies: Vec<Dependency>,
}

pub struct Context {
    log: Arc<Mutex<ExecutionLog>>,
    from_step: StepId,
}
```

Problems:
- Mutex contention on every dependency insert
- No efficient parent/child queries (requires building `DagIndex` from scratch)
- `DagIndex::from_dependencies()` rebuilds HashMaps on every query

## Proposed Structure

```rust
enum EdgeSet {
    Small(AppendOnlyVec<u32>),  // Linear search, no lock needed
    Large(DashMap<u32, ()>),    // Lock-free concurrent set
}

struct ConcurrentDag {
    // Node bimap: StepId <-> index
    step_to_ix: DashMap<StepId, u32>,
    ix_to_step: AppendOnlyVec<StepId>,
    next_ix: AtomicUsize,

    // Edge maps: index -> set of indices (hybrid representation)
    children: DashMap<u32, EdgeSet>,    // forward edges (from -> to)
    parents: DashMap<u32, EdgeSet>,     // backward edges (to -> from)
}
```

### Design Decisions

1. **`u32` indices instead of `usize`**
   - 4 billion nodes is more than sufficient
   - Saves memory (especially on 64-bit systems)
   - Use `debug_assert!` to check overflow in debug builds

2. **`DashMap<StepId, u32>` for step lookup**
   - Lock-free concurrent HashMap
   - Get-or-insert pattern for node allocation

3. **`AppendOnlyVec<StepId>` for reverse lookup**
   - Sequential index assignment makes Vec natural
   - Lock-free append
   - Compact memory layout

4. **Hybrid edge sets: `enum EdgeSet`**
   - Small sets: `ArrayVec<u32, 6>` with inline storage and linear search
   - Large sets: Regular `HashSet<u32>` (outer `DashMap` provides synchronization)
   - Both variants same size (~24 bytes): optimal memory layout
   - No nested concurrency overhead - single level of locking at node granularity
   - Typical case (≤6 edges): inline storage, zero allocations, fast linear search
   - Rare large nodes: automatically upgrade to HashSet with O(1) lookups

5. **Bidirectional storage**
   - Store both `children` and `parents` for O(1) queries in both directions
   - Small memory cost, large query speedup

## Implementation Plan

### 1. Dependencies

Add to `Cargo.toml`:
```toml
dashmap = "6.1"
append-only-vec = "0.1"
arrayvec = "0.7"
```

### 2. Core Structure

```rust
use append_only_vec::AppendOnlyVec;
use arrayvec::ArrayVec;
use dashmap::DashMap;
use std::collections::HashSet;
use std::sync::atomic::{AtomicUsize, Ordering};

// Threshold: Match HashSet size (~24 bytes = 3 pointers on 64-bit)
// ArrayVec with 6 u32s = 24 bytes, same as HashSet's inline size
const EDGE_SET_THRESHOLD: usize = 6;

enum EdgeSet {
    Small(ArrayVec<u32, EDGE_SET_THRESHOLD>),  // Inline storage, bounded capacity
    Large(HashSet<u32>),                       // Regular HashSet, outer DashMap provides sync
}

impl EdgeSet {
    fn new() -> Self {
        EdgeSet::Small(ArrayVec::new())
    }

    fn insert(&mut self, ix: u32) {
        match self {
            EdgeSet::Small(vec) => {
                // Check if already exists (linear search, but fast for small N)
                if vec.contains(&ix) {
                    return;
                }

                // Check if we need to upgrade before inserting
                if vec.len() >= EDGE_SET_THRESHOLD {
                    // Upgrade to Large (regular HashSet, outer DashMap provides sync)
                    let mut set = HashSet::new();
                    for &item in vec.iter() {
                        set.insert(item);
                    }
                    set.insert(ix);
                    *self = EdgeSet::Large(set);
                } else {
                    vec.push(ix);
                }
            }
            EdgeSet::Large(set) => {
                set.insert(ix);
            }
        }
    }

    fn contains(&self, ix: u32) -> bool {
        match self {
            EdgeSet::Small(vec) => vec.contains(&ix),
            EdgeSet::Large(set) => set.contains(&ix),
        }
    }

    fn iter(&self) -> Vec<u32> {
        match self {
            EdgeSet::Small(vec) => vec.to_vec(),
            EdgeSet::Large(set) => set.iter().copied().collect(),
        }
    }

    fn len(&self) -> usize {
        match self {
            EdgeSet::Small(vec) => vec.len(),
            EdgeSet::Large(set) => set.len(),
        }
    }
}

struct ConcurrentDag {
    step_to_ix: DashMap<StepId, u32>,
    ix_to_step: AppendOnlyVec<StepId>,
    next_ix: AtomicUsize,
    children: DashMap<u32, EdgeSet>,
    parents: DashMap<u32, EdgeSet>,
}

impl ConcurrentDag {
    fn new() -> Self {
        Self {
            step_to_ix: DashMap::new(),
            ix_to_step: AppendOnlyVec::new(),
            next_ix: AtomicUsize::new(0),
            children: DashMap::new(),
            parents: DashMap::new(),
        }
    }

    fn get_or_insert_node(&self, step: StepId) -> u32 {
        if let Some(ix) = self.step_to_ix.get(&step) {
            return *ix;
        }

        let ix_usize = self.next_ix.fetch_add(1, Ordering::SeqCst);
        debug_assert!(ix_usize <= u32::MAX as usize, "Index overflow: too many nodes");
        let ix = ix_usize as u32;

        self.step_to_ix.insert(step.clone(), ix);
        self.ix_to_step.push(step);

        ix
    }

    fn add_edge(&self, from: StepId, to: StepId) {
        let from_ix = self.get_or_insert_node(from);
        let to_ix = self.get_or_insert_node(to);

        // Add forward edge
        self.children.entry(from_ix)
            .and_modify(|edge_set| edge_set.insert(to_ix))
            .or_insert_with(|| {
                let mut edge_set = EdgeSet::new();
                edge_set.insert(to_ix);
                edge_set
            });

        // Add backward edge
        self.parents.entry(to_ix)
            .and_modify(|edge_set| edge_set.insert(from_ix))
            .or_insert_with(|| {
                let mut edge_set = EdgeSet::new();
                edge_set.insert(from_ix);
                edge_set
            });
    }

    fn get_children(&self, step: &StepId) -> Option<Vec<u32>> {
        let ix = *self.step_to_ix.get(step)?;
        Some(self.children.get(&ix)?.iter())
    }

    fn get_parents(&self, step: &StepId) -> Option<Vec<u32>> {
        let ix = *self.step_to_ix.get(step)?;
        Some(self.parents.get(&ix)?.iter())
    }
}
```

### 3. Integration with Context

Replace `Arc<Mutex<ExecutionLog>>` with `Arc<ConcurrentDag>`:

```rust
pub struct Context {
    dag: Arc<ConcurrentDag>,
    from_step: StepId,
}

impl Context {
    fn root() -> Self {
        Self {
            dag: Arc::new(ConcurrentDag::new()),
            from_step: StepId::Root,
        }
    }

    pub fn in_read<T>(&mut self, file_path: impl Into<String>, f: impl FnOnce(&mut Self) -> T) -> T {
        let to_step = StepId::Read(ReadId {
            file_path: file_path.into(),
        });

        self.dag.add_edge(self.from_step.clone(), to_step.clone());

        let old_from_step = self.from_step.clone();
        self.from_step = to_step;
        let result = f(self);
        self.from_step = old_from_step;

        result
    }

    // Similar for in_parse, in_resolve, in_exec...
}
```

### 4. Migration Path

1. Add new `ConcurrentDag` struct alongside existing code
2. Update `Context` to use `ConcurrentDag` instead of `ExecutionLog`
3. Remove old `ExecutionLog` struct
4. Update `to_json()` to query `ConcurrentDag` directly (no need to rebuild indices)
5. Update tests

## Performance Characteristics

| Operation | Current | Proposed |
|-----------|---------|----------|
| Add dependency | O(1) with lock | O(1) lock-free for node lookup, O(1) with RwLock for edge insert |
| Get children | O(n) rebuild | O(1) lookup + O(k) copy where k = #children |
| Get parents | O(n) rebuild | O(1) lookup + O(k) copy where k = #parents |
| Find roots | O(n) | O(n) iterate nodes |
| Memory | O(n) edges | O(n) nodes + O(n) edges × 2 (bidirectional) |

## Future Optimizations

1. **Tune threshold**: Profile to find optimal `EDGE_SET_THRESHOLD` (currently 32)
2. **Compact node IDs**: Consider dense packing if memory becomes an issue
3. **Read-optimized views**: Cache common queries (e.g., root nodes, leaf nodes)
4. **Downgrade large sets**: If DashMap becomes mostly empty (rare), could downgrade back to Small

## Notes

- All operations are grow-only; no deletions needed
- `debug_assert!` ensures `u32` index space is not exhausted (debug builds only)
- Hybrid EdgeSet optimizes for common case (small sets) with inline storage via ArrayVec
- **Zero heap allocations** for edge sets with ≤6 items (covers most nodes)
- Linear search on ArrayVec is very fast for small N (6 items = ~6 comparisons worst case)
- Both enum variants same size (~24 bytes): no wasted space, efficient memory layout
- Automatic upgrade to HashSet for large edge sets (>6) ensures O(1) lookups scale
- **Single-level concurrency**: Outer `DashMap<u32, EdgeSet>` provides all synchronization
- No nested locks or concurrent data structures inside EdgeSet
- Mutations happen under exclusive `&mut EdgeSet` access from DashMap::entry()
- The design maintains the same public API, so migration should be straightforward
