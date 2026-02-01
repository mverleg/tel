use crate::lazy::ALazy;
use append_only_vec::AppendOnlyVec;
use scc::HashMap;
use std::future::Future;
use std::hash::Hash;

/// Trait for types that can provide a unique identifier for caching.
///
/// The id must exactly identify the data:
/// - Using too little will give mismatched cache hits, causing strange bugs
/// - Using too much would lead to cache misses
/// - This is used often, so both `id()` and the result's hash should be cheap
pub trait HasId {
    type Uid: Eq + Hash;

    fn id(&self) -> Self::Uid;
}

impl HasId for i32 {
    type Uid = i32;

    fn id(&self) -> Self::Uid {
        *self
    }
}

impl HasId for usize {
    type Uid = usize;

    fn id(&self) -> Self::Uid {
        *self
    }
}


/// A concurrent cache with lazy async initialization.
///
/// Properties:
/// - Can only grow (to shrink, must replace by a shrunken version)
/// - Elements get initialized once; subsequent initializations can wait (async) for completion
/// - Can borrow any number of elements, including repeats, because data never moves
/// - Thread-safe and lock-free
///
/// # Type Parameters
/// - `K`: The cache key type (must be Eq + Hash)
/// - `V`: The cached value type
/// - `E`: The error type for initialization failures
///
/// # Performance
/// - First access to a key: Initializes the value
/// - Concurrent access during initialization: Waits for the initializer to complete
/// - Subsequent access: Very fast (single relaxed atomic load + array index)
pub struct Cache<K, V, E> {
    lookup: HashMap<K, usize>,
    data: AppendOnlyVec<ALazy<V, E>>,
}

impl<K: Eq + Hash, V, E> Cache<K, V, E> {
    /// Create a new empty cache.
    pub fn new() -> Self {
        Cache {
            lookup: HashMap::new(),
            data: AppendOnlyVec::new(),
        }
    }

    /// Get the number of entries in the cache.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.data.len() == 0
    }
}

impl<K: Eq + Hash, V, E> Cache<K, V, E> {
    /// Get a cached value, or initialize it if not present.
    ///
    /// This method:
    /// - Returns the cached value if it has already been computed
    /// - Waits (async) if it is currently being computed by another task
    /// - Calls `init` to compute it if not yet started
    ///
    /// The `init` function is only called once per unique key, even if multiple
    /// tasks call `get` concurrently.
    ///
    /// # Arguments
    /// - `key`: The cache key to look up
    /// - `init`: Function to initialize the value if not cached
    ///
    /// # Returns
    /// A reference to the cached result (Ok or Err).
    ///
    /// # Performance
    /// Uses a two-phase lookup to avoid cloning keys on cache hits:
    /// - Fast path: Check existence with borrowed key (no allocation)
    /// - Slow path: Insert new entry (consumes key)
    pub async fn get<F, Fut>(&self, key: K, init: F) -> &Result<V, E>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<V, E>>,
    {
        // Fast path: check if key exists without cloning
        if let Some(ix) = self.lookup.read_async(&key, |_, &ix| ix).await {
            return self.data[ix].get_or_init(init).await;
        }

        // Slow path: insert new entry (key is moved here)
        let ix = match self.lookup.entry_async(key).await {
            scc::hash_map::Entry::Occupied(occupied) => *occupied.get(),
            scc::hash_map::Entry::Vacant(vacant) => {
                let new_ix = self.data.push(ALazy::new());
                vacant.insert_entry(new_ix);
                new_ix
            }
        };

        // Initialize the value at this index (or wait if another task is doing it)
        self.data[ix].get_or_init(init).await
    }

    /// Get a cached value with arguments that implement HasId.
    ///
    /// This is a convenience wrapper around `get` that extracts the key from
    /// the arguments using the `HasId` trait.
    pub async fn get_with_args<A, F, Fut>(&self, args: A, init: F) -> &Result<V, E>
    where
        A: HasId<Uid = K>,
        F: FnOnce(A) -> Fut,
        Fut: Future<Output = Result<V, E>>,
    {
        let key = args.id();
        self.get(key, || init(args)).await
    }
}

impl<K: Eq + Hash, V, E> Default for Cache<K, V, E> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[tokio::test]
    async fn test_cache_basic() {
        let cache = Cache::new();
        let result = cache.get(1, || async { Ok::<_, ()>(42) }).await;
        assert_eq!(*result, Ok(42));

        // Second call should return cached value
        let result2 = cache.get(1, || async { Ok::<_, ()>(99) }).await;
        assert_eq!(*result2, Ok(42));
    }

    #[tokio::test]
    async fn test_cache_different_keys() {
        let cache = Cache::new();

        let result1 = cache.get(1, || async { Ok::<_, ()>(42) }).await;
        let result2 = cache.get(2, || async { Ok::<_, ()>(99) }).await;

        assert_eq!(*result1, Ok(42));
        assert_eq!(*result2, Ok(99));
        assert_eq!(cache.len(), 2);
    }

    #[tokio::test]
    async fn test_cache_error_caching() {
        let cache = Cache::new();
        let result = cache.get(1, || async { Err::<i32, _>("error") }).await;
        assert_eq!(*result, Err("error"));

        // Error should be cached
        let result2 = cache.get(1, || async { Ok::<_, &str>(42) }).await;
        assert_eq!(*result2, Err("error"));
    }

    async fn spawn_cache_task<K: Eq + Hash + Send + Sync + 'static>(
        cache: Arc<Cache<K, i32, ()>>,
        counter: Arc<AtomicUsize>,
        key: K,
        value: i32,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            cache
                .get(key, || async {
                    counter.fetch_add(1, Ordering::SeqCst);
                    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                    Ok::<_, ()>(value)
                })
                .await;
        })
    }

    #[tokio::test]
    async fn test_cache_concurrent_access() {
        let cache = Arc::new(Cache::new());
        let counter = Arc::new(AtomicUsize::new(0));

        // Spawn multiple tasks accessing the same key
        let mut handles = vec![];
        for _ in 0..10 {
            handles.push(spawn_cache_task(cache.clone(), counter.clone(), 1, 42).await);
        }

        // Wait for all tasks
        for handle in handles {
            handle.await.unwrap();
        }

        // Initialization should only happen once
        assert_eq!(counter.load(Ordering::SeqCst), 1);
        assert_eq!(cache.len(), 1);
    }

    #[tokio::test]
    async fn test_cache_multiple_keys_concurrent() {
        let cache = Arc::new(Cache::new());
        let counter = Arc::new(AtomicUsize::new(0));

        // Spawn tasks for different keys
        let mut handles = vec![];
        for i in 0..5 {
            handles.push(spawn_cache_task(cache.clone(), counter.clone(), i, i * 10).await);
        }

        // Wait for all tasks
        for handle in handles {
            handle.await.unwrap();
        }

        // Each key should be initialized once
        assert_eq!(counter.load(Ordering::SeqCst), 5);
        assert_eq!(cache.len(), 5);
    }

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    struct TestArgs {
        id: i32,
        data: String,
    }

    impl HasId for TestArgs {
        type Uid = i32;

        fn id(&self) -> Self::Uid {
            self.id
        }
    }

    #[tokio::test]
    async fn test_cache_with_args() {
        let cache = Cache::new();
        let args = TestArgs {
            id: 1,
            data: "test".to_string(),
        };

        let result = cache
            .get_with_args(args.clone(), |a| async move {
                Ok::<_, ()>(a.data.len())
            })
            .await;

        assert_eq!(*result, Ok(4));

        // Second call with same id should return cached value
        let args2 = TestArgs {
            id: 1,
            data: "different".to_string(),
        };

        let result2 = cache
            .get_with_args(args2, |a| async move { Ok::<_, ()>(a.data.len()) })
            .await;

        assert_eq!(*result2, Ok(4)); // Still 4, not 9
    }
}