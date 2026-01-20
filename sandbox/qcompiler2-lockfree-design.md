# Lock-free Dependency Tracking Design

## Problem Statement

The current `qcompiler2.rs` implementation uses `Arc<Mutex<ExecutionLog>>` to track dependencies. The mutex is only used to append to a grow-only `Vec<Dependency>`, which creates unnecessary contention in multi-threaded scenarios.

Lines affected:
- `qcompiler2.rs:80` - `in_read` method
- `qcompiler2.rs:108` - `in_parse` method
- `qcompiler2.rs:136` - `in_resolve` method
- `qcompiler2.rs:164` - `in_exec` method
- `qcompiler2.rs:179` - `dependencies` method

## Current Implementation

```rust
struct ExecutionLog {
    dependencies: Vec<Dependency>,
}

pub struct Context {
    log: Arc<Mutex<ExecutionLog>>,
    from_step: StepId,
}

// Usage:
self.log.lock().unwrap().dependencies.push(my_dep);
```

The `ExecutionLog` wrapper struct serves no purpose other than holding the Vec.

## Proposed Solution

Replace the mutex-protected Vec with a lock-free append-only data structure.

### Option 1: `append-only-vec` crate (Recommended)

**Pros:**
- Purpose-built for this exact use case
- Clean API: maintains Vec-like interface
- Memory-efficient: single contiguous allocation
- Fast reads: can iterate without locks

**Cons:**
- Adds external dependency

**Implementation:**
```rust
use append_only_vec::AppendOnlyVec;

pub struct Context {
    log: Arc<AppendOnlyVec<Dependency>>,
    from_step: StepId,
}

// Usage:
self.log.push(my_dep);
```

### Option 2: `crossbeam::queue::SegQueue`

**Pros:**
- Well-maintained, popular crate
- Lock-free push/pop operations

**Cons:**
- Queue semantics (not Vec-like)
- Iteration is more complex
- May already be a dependency

### Option 3: `lockfree` crate

**Pros:**
- Provides `AppendList<T>`

**Cons:**
- Less actively maintained
- More complex API

## Recommended Approach

Use `append-only-vec::AppendOnlyVec<Dependency>` for the following reasons:

1. **Semantic match** - We never remove items, only append
2. **Simple API** - Minimal changes to existing code
3. **Performance** - No lock contention on appends
4. **Read efficiency** - The `dependencies()` method can iterate without locks

## Implementation Plan

1. Add `append-only-vec` to `Cargo.toml`
2. Remove `ExecutionLog` struct (no longer needed)
3. Change `Context::log` from `Arc<Mutex<ExecutionLog>>` to `Arc<AppendOnlyVec<Dependency>>`
4. Update `Context::root()` to initialize `AppendOnlyVec::new()`
5. Replace all `self.log.lock().unwrap().dependencies.push(my_dep)` with `self.log.push(my_dep)`
6. Update `dependencies()` method to iterate over the append-only vec

## Impact

- **Lines changed**: ~10 lines
- **Breaking changes**: None (public API unchanged)
- **Performance**: Eliminates lock contention on dependency tracking
- **Complexity**: Reduced (removes mutex, removes wrapper struct)
