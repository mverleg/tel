# Context Fork Design for Async Support

## Problem

The current stack-based approach with `enter_step`/`exit_step` doesn't work with async:
- Call stack isn't preserved across `.await` points
- Multiple async tasks run concurrently - no single "top of stack"
- Easy to forget cleanup (`exit_step`) especially with early returns in async code

## Solution: Fork-Based Context

Each method call creates a new `Context` fork with explicit parent tracking.

## Design

### Core Types

```rust
use std::sync::{Arc, Mutex};

// Shared mutable state - all forks write here
struct ExecutionLog {
    dependencies: Vec<Dependency>,
}

// Immutable context that forks on each step
pub struct Context {
    log: Arc<Mutex<ExecutionLog>>,
    from_step: Option<StepId>,
}
```

**Name choice: `ExecutionLog`** - describes what it stores, leaves room for adding more execution metadata beyond dependencies.

### Key Properties

1. **Shared log**: `Arc<Mutex<ExecutionLog>>` - thread-safe, works with async runtimes
2. **Immutable context**: Each `Context` instance is immutable (except shared log)
3. **Explicit parent**: `from_step` field tracks parent step
4. **Fork semantics**: Each `in_*` method creates a new context

### Implementation

```rust
impl Context {
    pub fn new() -> Self {
        Self {
            log: Arc::new(Mutex::new(ExecutionLog {
                dependencies: Vec::new(),
            })),
            from_step: None,
        }
    }

    pub async fn in_read<T, Fut>(
        &self,
        file_path: impl Into<String>,
        f: impl FnOnce(Context) -> Fut,
    ) -> T
    where
        Fut: std::future::Future<Output = T>,
    {
        let my_id = StepId::Read(ReadId {
            file_path: file_path.into(),
        });

        // Log entry
        println!("[qcompiler2] Enter: {}", serde_json::to_string(&my_id).unwrap());

        // Record dependency from parent to this step
        if let Some(ref my_parent) = self.from_step {
            let my_dep = Dependency {
                from: my_parent.clone(),
                to: my_id.clone(),
            };
            println!("[qcompiler2] Dependency: {} -> {}",
                serde_json::to_string(&my_dep.from).unwrap(),
                serde_json::to_string(&my_dep.to).unwrap());
            self.log.lock().unwrap().dependencies.push(my_dep);
        }

        // Fork: create new context with updated from_step
        let my_child = Context {
            log: self.log.clone(),  // Arc::clone is cheap
            from_step: Some(my_id.clone()),
        };

        let my_result = f(my_child).await;

        // Log exit
        println!("[qcompiler2] Exit: {}", serde_json::to_string(&my_id).unwrap());

        my_result
    }

    // Similar for in_parse, in_resolve, in_exec...
}
```

### Synchronous Compatibility

For non-async code, provide sync versions:

```rust
pub fn in_read_sync<T>(
    &self,
    file_path: impl Into<String>,
    f: impl FnOnce(Context) -> T,
) -> T {
    let my_id = StepId::Read(ReadId {
        file_path: file_path.into(),
    });

    if let Some(ref my_parent) = self.from_step {
        self.log.lock().unwrap().dependencies.push(Dependency {
            from: my_parent.clone(),
            to: my_id.clone(),
        });
    }

    let my_child = Context {
        log: self.log.clone(),
        from_step: Some(my_id),
    };

    f(my_child)
}
```

Or use the same method with a generic future that might be immediate.

## Mutability Strategy

**Use Arc<Mutex<ExecutionLog>>** because:
- Thread-safe: works with multi-threaded async runtimes (tokio)
- Simple: straightforward lock/unlock semantics
- Compatible: works with both sync and async code
- Low contention: dependency recording is fast

**Alternatives considered:**
- `Rc<RefCell<>>`: Single-threaded only, breaks with `Send` futures
- `Arc<RwLock<>>`: Overkill, dependencies are write-heavy
- Pure functional: Impractical API, would need to thread dependencies through everything

## Migration Path

1. Add `ExecutionLog` struct
2. Change `Context` to fork-based with `Arc<Mutex<>>`
3. Update all `in_*` methods to fork instead of push/pop
4. Remove `enter_step`/`exit_step` private methods
5. Keep sync versions for now, add async versions when needed
6. Tests should pass unchanged (same dependency graph output)

## Benefits

1. **Async-ready**: Works naturally with async/await
2. **Concurrent-safe**: Multiple tasks can run in parallel
3. **No cleanup bugs**: No `exit_step` to forget
4. **Explicit parent**: `from_step` makes relationships clear
5. **Immutable context**: Easier to reason about, fewer bugs

## Usage Example (Future Async)

```rust
async fn compile_async(ctx: &Context, file: &str) -> Result<()> {
    ctx.in_read(file, |ctx| async move {
        let content = read_file_async(file).await?;

        ctx.in_parse(file, |ctx| async move {
            let ast = parse_async(&content).await?;

            // Concurrent resolution of multiple functions
            let futures: Vec<_> = ast.functions.iter()
                .map(|f| {
                    let func_ctx = ctx.clone(); // cheap Arc clone
                    async move {
                        func_ctx.in_resolve(&f.name, |ctx| async move {
                            resolve_async(ctx, f).await
                        }).await
                    }
                })
                .collect();

            futures::future::join_all(futures).await;
            Ok(())
        }).await
    }).await
}
```

## Performance

- `Arc::clone()`: ~2 atomic increments (negligible)
- `Mutex::lock()`: Uncontended locks are fast (~10ns)
- No contention expected: dependency recording is microseconds
- Overall: Unmeasurable impact on compilation time
