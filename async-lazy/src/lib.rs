//! Highly efficient async lazy initialization and caching primitives.
//!
//! This crate provides:
//! - `ALazy`: A lazy initialization structure with very efficient read path (no memory barriers)
//! - `Cache`: A concurrent cache that combines lazy async initialization with a HashMap lookup
//!
//! # Key Features
//!
//! 1. **Very Efficient Fast Path**: When a value is already initialized, reading it requires only
//!    a single relaxed atomic load (no memory barriers).
//!
//! 2. **Concurrent Initialization Protection**: Only one task will initialize each value, even
//!    when multiple tasks try concurrently.
//!
//! 3. **Async Waiting**: Tasks that arrive during initialization will wait asynchronously for
//!    completion rather than blocking or re-initializing.
//!
//! 4. **Error Caching**: Both successful values and errors are cached, preventing repeated
//!    failed initialization attempts.
//!
//! # Example: ALazy
//!
//! ```
//! use async_lazy::ALazy;
//!
//! # tokio_test::block_on(async {
//! let lazy = ALazy::new();
//!
//! // First access initializes
//! let result = lazy.get_or_init(|| async { Ok::<_, ()>(42) }).await;
//! assert_eq!(*result, Ok(42));
//!
//! // Subsequent access returns cached value (very fast)
//! let result2 = lazy.get_or_init(|| async { Ok::<_, ()>(99) }).await;
//! assert_eq!(*result2, Ok(42));
//! # });
//! ```
//!
//! # Example: Cache
//!
//! ```
//! use async_lazy::Cache;
//! use std::sync::Arc;
//!
//! # tokio_test::block_on(async {
//! let cache = Arc::new(Cache::new());
//!
//! // Initialize value for key 1
//! let result = cache.get(1, || async { Ok::<_, ()>(42) }).await;
//! assert_eq!(*result, Ok(42));
//!
//! // Multiple concurrent accesses only initialize once
//! let cache_clone = cache.clone();
//! let handle = tokio::spawn(async move {
//!     cache_clone.get(2, || async { Ok::<_, ()>(99) }).await
//! });
//!
//! let result2 = cache.get(2, || async { Ok::<_, ()>(100) }).await;
//! handle.await.unwrap();
//!
//! // Only one initialization happened, both tasks see same result
//! assert_eq!(*result2, Ok(99));
//! # });
//! ```

pub mod cache;
pub mod lazy;

pub use cache::{Cache, HasId};
pub use lazy::ALazy;