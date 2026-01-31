# async-lazy

Highly efficient async lazy initialization and caching primitives for Rust.

## Features

- **ALazy**: Lazy initialization structure with very efficient read path (no memory barriers on fast path)
- **Cache**: Concurrent cache combining lazy async initialization with HashMap lookup
- Thread-safe and lock-free
- Error caching to prevent repeated failed initialization attempts
- Concurrent initialization protection - only one task initializes each value

## Performance

The key optimization is in the read path for already-initialized values:
- Uses `Relaxed` atomic ordering for the common case
- No memory barriers when checking if value is already initialized
- Direct access to cached value once initialized

## Usage

Add to your `Cargo.toml`:
```toml
[dependencies]
async-lazy = { path = "../async-lazy" }
```

### ALazy Example

```rust
use async_lazy::ALazy;

let lazy = ALazy::new();

// First access initializes
let result = lazy.get_or_init(|| async { Ok::<_, ()>(42) }).await;

// Subsequent access returns cached value (very fast)
let result2 = lazy.get().unwrap();
```

### Cache Example

```rust
use async_lazy::Cache;

let cache = Cache::new();

// Initialize value for key
let result = cache.get(1, || async { Ok::<_, ()>(42) }).await;

// Subsequent access returns cached value
let result2 = cache.get(1, || async { Ok::<_, ()>(99) }).await;
// result2 is still 42
```

## Implementation Details

The `ALazy` type uses a state machine with four states:
- `EMPTY` (0): Not yet initialized
- `INITIALIZING` (1): Currently being initialized by some task
- `FILLED` (2): Successfully initialized
- `FAILED` (3): Initialization failed (error is cached)

State transitions are managed using atomic compare-exchange operations to ensure only one task performs initialization, while others wait asynchronously using `tokio::sync::Notify`.

The `Cache` type combines:
- `scc::HashMap` for concurrent key lookup
- `append_only_vec::AppendOnlyVec` for stable value storage
- `ALazy` for per-entry lazy initialization

## License

See the workspace license.