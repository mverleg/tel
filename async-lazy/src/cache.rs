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

    /// The number of cached terminal entries.
    ///
    /// Counts only slots holding an actual value (success or cached error), not
    /// slots that were claimed and then released — a cancelled, panicked, or
    /// aborted [`get_or_init_abortable`](Cache::get_or_init_abortable) leaves an
    /// allocated but empty slot in the append-only backing, and that is not a
    /// cached answer. O(n) in the slot count; not a hot path.
    pub fn len(&self) -> usize {
        self.data.iter().filter(|slot| slot.is_present()).count()
    }

    /// Check if the cache holds no terminal entries.
    pub fn is_empty(&self) -> bool {
        !self.data.iter().any(|slot| slot.is_present())
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

    /// Single-flight get where `init` may **abort** rather than cache a value —
    /// the keyed analogue of [`ALazy::get_or_init_abortable`]. `Ok(result)` is
    /// stored terminally; `Err(abort)` reverts the claim (waiters re-claim),
    /// caches nothing, and returns the abort. Lets a content store single-flight
    /// terminal answers while leaving non-terminal failures uncached
    /// (plans/concurrency-and-eviction.md Decision 4).
    pub async fn get_or_init_abortable<F, Fut, A>(&self, key: K, init: F) -> Result<&Result<V, E>, A>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<Result<V, E>, A>>,
    {
        if let Some(ix) = self.lookup.read_async(&key, |_, &ix| ix).await {
            return self.data[ix].get_or_init_abortable(init).await;
        }
        let ix = match self.lookup.entry_async(key).await {
            scc::hash_map::Entry::Occupied(occupied) => *occupied.get(),
            scc::hash_map::Entry::Vacant(vacant) => {
                let new_ix = self.data.push(ALazy::new());
                vacant.insert_entry(new_ix);
                new_ix
            }
        };
        self.data[ix].get_or_init_abortable(init).await
    }

    /// Peek at a key without claiming or initializing it: returns the cached
    /// terminal value if one is present, else `None` (including while another
    /// task is mid-initialization). A lock-free fast-path read — the same one
    /// `get` uses on a hit — so a caller can serve an existing answer without
    /// entering the single-flight claim.
    pub fn peek(&self, key: &K) -> Option<&Result<V, E>> {
        let ix = self.lookup.read(key, |_, &ix| ix)?;
        self.data[ix].get()
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

impl<K: Eq + Hash + Clone, V, E> Cache<K, V, E> {
    /// Shrink the cache in place to the entries whose key satisfies `keep`,
    /// rebuilding the append-only backing so evicted entries actually release
    /// their memory (the cache is otherwise grow-only — "to shrink, replace by
    /// a shrunken version").
    ///
    /// Takes `&mut self` on purpose: with exclusive access no entry can be
    /// mid-initialization, so every slot is terminal or empty and may be moved
    /// or dropped soundly. This is the compaction window
    /// plans/concurrency-and-eviction.md (Decision 2) relies on — reached only
    /// between waves, once `run(&mut self)` has joined every spawned task.
    ///
    /// A key dropped here is never lost to correctness: the content store is
    /// content-addressed with a disk tier, so a re-demand re-loads or recomputes
    /// an equal answer. Eviction affects warmth only.
    pub fn retain(&mut self, mut keep: impl FnMut(&K) -> bool) {
        let old_lookup = std::mem::replace(&mut self.lookup, HashMap::new());
        let old_data = std::mem::replace(&mut self.data, AppendOnlyVec::new());
        // Move each slot into an `Option` so a survivor can be taken out by its
        // old index without disturbing the others.
        let mut slots: Vec<Option<ALazy<V, E>>> = old_data.into_iter().map(Some).collect();
        // First pass (read-only scan): gather survivors with their old index.
        let mut survivors: Vec<(K, usize)> = Vec::new();
        old_lookup.scan(|k, &ix| {
            if keep(k) {
                survivors.push((k.clone(), ix));
            }
        });
        // Second pass: move each survivor's value into the fresh backing.
        for (k, ix) in survivors {
            if let Some(slot) = slots.get_mut(ix).and_then(Option::take) {
                let new_ix = self.data.push(slot);
                let _ = self.lookup.insert(k, new_ix);
            }
        }
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

    #[tokio::test]
    async fn test_cache_retain_shrinks_and_keeps_survivors() {
        let mut cache = Cache::new();
        for i in 0..6 {
            cache.get(i, || async move { Ok::<_, ()>(i * 10) }).await;
        }
        assert_eq!(cache.len(), 6);

        // Keep the even keys; the odd ones are compacted away.
        cache.retain(|k| k % 2 == 0);
        assert_eq!(cache.len(), 3);

        // A survivor keeps its value and does *not* re-run init.
        let survivor = cache.get(4, || async { Ok::<_, ()>(-1) }).await;
        assert_eq!(*survivor, Ok(40));

        // An evicted key is simply absent — it recomputes on next demand, and
        // the fresh slot upholds init-once from here on.
        let recomputed = cache.get(3, || async { Ok::<_, ()>(999) }).await;
        assert_eq!(*recomputed, Ok(999));
        assert_eq!(*cache.get(3, || async { Ok::<_, ()>(-1) }).await, Ok(999));
        assert_eq!(cache.len(), 4);
    }

    #[tokio::test]
    async fn test_cache_retain_all_and_none() {
        let mut cache = Cache::new();
        for i in 0..4 {
            cache.get(i, || async move { Ok::<_, ()>(i) }).await;
        }
        cache.retain(|_| true);
        assert_eq!(cache.len(), 4);
        cache.retain(|_| false);
        assert_eq!(cache.len(), 0);
        assert!(cache.is_empty());
        // Still usable after being emptied.
        assert_eq!(*cache.get(7, || async { Ok::<_, ()>(7) }).await, Ok(7));
        assert_eq!(cache.len(), 1);
    }

    #[tokio::test]
    async fn test_abortable_caches_terminal_reverts_abort() {
        let cache: Cache<i32, i32, ()> = Cache::new();

        // A terminal answer is cached like `get`.
        let ok: Result<_, &str> = cache
            .get_or_init_abortable(1, || async { Ok(Ok::<_, ()>(42)) })
            .await;
        assert_eq!(*ok.unwrap(), Ok(42));
        assert_eq!(cache.len(), 1);

        // An abort caches nothing and hands the abort back.
        let aborted: Result<_, &str> = cache
            .get_or_init_abortable(2, || async { Err("nope") })
            .await;
        assert_eq!(aborted.err(), Some("nope"));
        assert!(cache.peek(&2).is_none(), "aborted key left uncached");

        // The same key can succeed on a later attempt (the claim was released).
        let retry: Result<_, &str> = cache
            .get_or_init_abortable(2, || async { Ok(Ok::<_, ()>(7)) })
            .await;
        assert_eq!(*retry.unwrap(), Ok(7));
        assert_eq!(cache.peek(&2).map(|r| *r), Some(Ok(7)));
    }

    #[tokio::test]
    async fn test_abortable_coalesces_concurrent_demands() {
        // Two concurrent demands for one key run init exactly once.
        let cache: Arc<Cache<i32, i32, ()>> = Arc::new(Cache::new());
        let counter = Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::new();
        for _ in 0..8 {
            let cache = cache.clone();
            let counter = counter.clone();
            handles.push(tokio::spawn(async move {
                let r: Result<_, ()> = cache
                    .get_or_init_abortable(1, || async {
                        counter.fetch_add(1, Ordering::SeqCst);
                        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                        Ok(Ok::<_, ()>(99))
                    })
                    .await;
                *r.unwrap()
            }));
        }
        for h in handles {
            assert_eq!(h.await.unwrap(), Ok(99));
        }
        assert_eq!(counter.load(Ordering::SeqCst), 1, "init ran once for all demands");
    }

    #[tokio::test]
    async fn test_abortable_waiter_takes_over_after_abort() {
        // A waiter parked behind an aborting claimer re-claims and succeeds.
        let cache: Arc<Cache<i32, i32, ()>> = Arc::new(Cache::new());

        let leader_cache = cache.clone();
        let leader = tokio::spawn(async move {
            let r: Result<_, &str> = leader_cache
                .get_or_init_abortable(1, || async {
                    tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
                    Err("leader aborts")
                })
                .await;
            r.err()
        });

        // Give the leader time to claim, then park a waiter behind it.
        tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
        let waiter_cache = cache.clone();
        let waiter = tokio::spawn(async move {
            let r: Result<_, &str> = waiter_cache
                .get_or_init_abortable(1, || async { Ok(Ok::<_, ()>(123)) })
                .await;
            *r.unwrap()
        });

        assert_eq!(leader.await.unwrap(), Some("leader aborts"));
        let got = tokio::time::timeout(tokio::time::Duration::from_secs(1), waiter)
            .await
            .expect("waiter hung after leader aborted")
            .unwrap();
        assert_eq!(got, Ok(123));
    }

    #[tokio::test]
    async fn test_peek_sees_only_terminal_values() {
        let cache: Cache<i32, i32, ()> = Cache::new();
        assert!(cache.peek(&1).is_none());
        cache.get(1, || async { Ok::<_, ()>(5) }).await;
        assert_eq!(cache.peek(&1).map(|r| *r), Some(Ok(5)));
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