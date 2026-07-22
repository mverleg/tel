use std::cell::UnsafeCell;
use std::future::Future;
use std::sync::atomic::{AtomicU8, Ordering};
use tokio::sync::Notify;

const EMPTY: u8 = 0;
const INITIALIZING: u8 = 1;
const FILLED: u8 = 2;
const FAILED: u8 = 3;

/// A highly efficient async lazy initialization structure.
///
/// Features:
/// 1. Initialized once, then never changed
/// 2. Can detect concurrent initialization attempts
/// 3. Allows async tasks to wait for initialization to complete
/// 4. Very efficient fast path - uses Relaxed ordering when already filled (no memory barrier)
///
/// # Safety
/// This type is safe to use across threads. The UnsafeCell is protected by the state atomic.
/// Once state is FILLED or FAILED, the value never changes, so it's safe to read without synchronization.
pub struct ALazy<T, E> {
    state: AtomicU8,
    value: UnsafeCell<Option<Result<T, E>>>,
    notify: Notify,
}

unsafe impl<T: Send, E: Send> Send for ALazy<T, E> {}
unsafe impl<T: Send, E: Send> Sync for ALazy<T, E> {}

impl<T, E> ALazy<T, E> {
    /// Create a new uninitialized ALazy.
    pub const fn new() -> Self {
        ALazy {
            state: AtomicU8::new(EMPTY),
            value: UnsafeCell::new(None),
            notify: Notify::const_new(),
        }
    }

    /// Very fast path when already filled - just a relaxed atomic load.
    /// Returns None if not yet initialized or if initialization failed.
    #[inline]
    pub fn get(&self) -> Option<&Result<T, E>> {
        if self.state.load(Ordering::Relaxed) == FILLED {
            // SAFETY: State is FILLED, so value is initialized and will never change
            unsafe { (*self.value.get()).as_ref() }
        } else {
            None
        }
    }

    /// Check if the value is currently being initialized by another task.
    #[inline]
    pub fn is_initializing(&self) -> bool {
        self.state.load(Ordering::Acquire) == INITIALIZING
    }

    /// True once a terminal value (a success or a cached error) is stored.
    /// Distinguishes a real cached entry from a slot that was claimed and then
    /// released — a cancelled, panicked, or aborted initializer leaves the slot
    /// allocated but empty, and such a slot is *not* a cached answer.
    #[inline]
    pub fn is_present(&self) -> bool {
        matches!(self.state.load(Ordering::Acquire), FILLED | FAILED)
    }

    /// Get or initialize the value.
    ///
    /// If the value is already initialized (success or failure), returns it immediately.
    /// If another task is initializing, waits for completion.
    /// If not initialized, calls the provided function to initialize.
    ///
    /// A returned `Err` is terminal and cached like a success. Cancellation (the returned
    /// future being dropped mid-init) or a panic in `init` is *not* terminal: the claim
    /// reverts to empty and a waiting task takes over with its own `init`, so `init` may
    /// run more than once across tasks in those cases.
    pub async fn get_or_init<F, Fut>(&self, init: F) -> &Result<T, E>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T, E>>,
    {
        let mut init = Some(init);
        loop {
            // Fast path: already initialized (success or failure)
            match self.state.load(Ordering::Acquire) {
                FILLED | FAILED => {
                    // SAFETY: State is terminal, value is set and will never change
                    return unsafe { (*self.value.get()).as_ref().unwrap() };
                }
                _ => {}
            }

            // Try to claim initialization
            match self
                .state
                .compare_exchange(EMPTY, INITIALIZING, Ordering::Acquire, Ordering::Acquire)
            {
                Ok(_) => {
                    // If we are cancelled (dropped at the await below) or `init` panics,
                    // the claim must not stay INITIALIZING forever: the guard reverts it
                    // to EMPTY and wakes waiters so one of them can take over.
                    let guard = RevertClaimOnDrop { lazy: self };
                    let result = (init.take().expect("claim happens at most once"))().await;
                    std::mem::forget(guard);

                    let final_state = match &result {
                        Ok(_) => FILLED,
                        Err(_) => FAILED,
                    };
                    unsafe {
                        *self.value.get() = Some(result);
                    }
                    self.state.store(final_state, Ordering::Release);
                    self.notify.notify_waiters();

                    // SAFETY: We just filled it
                    return unsafe { (*self.value.get()).as_ref().unwrap() };
                }
                Err(_) => {
                    // Someone else is initializing (or it just became terminal). Register
                    // for notification *before* re-checking state: notify_waiters() only
                    // wakes already-registered waiters, so checking first could miss a
                    // notification sent in between and hang.
                    let notified = self.notify.notified();
                    tokio::pin!(notified);
                    notified.as_mut().enable();
                    match self.state.load(Ordering::Acquire) {
                        INITIALIZING => notified.await,
                        // Terminal, or reverted to EMPTY by a cancelled/panicked
                        // initializer: retry immediately (possibly claiming it ourselves).
                        _ => {}
                    }
                }
            }
        }
    }

    /// Get or initialize the value, where `init` may **abort** instead of
    /// producing a terminal answer.
    ///
    /// `init` returns `Result<Result<T, E>, A>`:
    /// - `Ok(result)` is a terminal answer — stored and cached exactly as
    ///   [`get_or_init`](ALazy::get_or_init) does (a returned inner `Err` is a
    ///   terminal error and cached like a success).
    /// - `Err(abort)` means *not an answer*: the claim reverts to empty (as if
    ///   the initializer had been cancelled), waiters take over, and the abort
    ///   is handed straight back to this caller. Nothing is cached.
    ///
    /// This is what lets a content store distinguish a terminal answer (cache
    /// it, single-flight it) from a non-terminal failure such as a caught panic
    /// (revert the claim, leave no poisoned `Pending`) — see
    /// plans/concurrency-and-eviction.md Decision 4. As with `get_or_init`, a
    /// panic or cancellation inside `init` also reverts the claim.
    pub async fn get_or_init_abortable<F, Fut, A>(&self, init: F) -> Result<&Result<T, E>, A>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<Result<T, E>, A>>,
    {
        let mut init = Some(init);
        loop {
            // Fast path: already terminal.
            match self.state.load(Ordering::Acquire) {
                FILLED | FAILED => {
                    return Ok(unsafe { (*self.value.get()).as_ref().unwrap() });
                }
                _ => {}
            }

            match self
                .state
                .compare_exchange(EMPTY, INITIALIZING, Ordering::Acquire, Ordering::Acquire)
            {
                Ok(_) => {
                    // Same revert-on-drop discipline as get_or_init: a cancel or
                    // panic below releases the claim. An explicit `Err(abort)`
                    // releases it too, by simply *not* defusing the guard.
                    let guard = RevertClaimOnDrop { lazy: self };
                    let outcome = (init.take().expect("claim happens at most once"))().await;
                    match outcome {
                        Ok(result) => {
                            std::mem::forget(guard);
                            let final_state = match &result {
                                Ok(_) => FILLED,
                                Err(_) => FAILED,
                            };
                            unsafe {
                                *self.value.get() = Some(result);
                            }
                            self.state.store(final_state, Ordering::Release);
                            self.notify.notify_waiters();
                            return Ok(unsafe { (*self.value.get()).as_ref().unwrap() });
                        }
                        // Non-terminal: drop `guard` to revert EMPTY + wake
                        // waiters (one of them re-claims), and hand the abort back.
                        Err(abort) => return Err(abort),
                    }
                }
                Err(_) => {
                    let notified = self.notify.notified();
                    tokio::pin!(notified);
                    notified.as_mut().enable();
                    match self.state.load(Ordering::Acquire) {
                        INITIALIZING => notified.await,
                        _ => {}
                    }
                }
            }
        }
    }
}

/// Reverts an `INITIALIZING` claim back to `EMPTY` and wakes waiters, unless defused
/// with `mem::forget` after successful initialization. Runs when the claiming task is
/// cancelled or its init future panics.
struct RevertClaimOnDrop<'a, T, E> {
    lazy: &'a ALazy<T, E>,
}

impl<T, E> Drop for RevertClaimOnDrop<'_, T, E> {
    fn drop(&mut self) {
        self.lazy.state.store(EMPTY, Ordering::Release);
        self.lazy.notify.notify_waiters();
    }
}

impl<T, E> Default for ALazy<T, E> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_basic_initialization() {
        let lazy = ALazy::new();
        let result = lazy.get_or_init(|| async { Ok::<_, ()>(42) }).await;
        assert_eq!(*result, Ok(42));

        // Second call should return same value
        let result2 = lazy.get_or_init(|| async { Ok::<_, ()>(99) }).await;
        assert_eq!(*result2, Ok(42));
    }

    #[tokio::test]
    async fn test_initialization_error() {
        let lazy = ALazy::new();
        let result = lazy
            .get_or_init(|| async { Err::<i32, _>("failed") })
            .await;
        assert_eq!(*result, Err("failed"));

        // Error is cached - second call returns same error
        let result2 = lazy.get_or_init(|| async { Ok::<_, &str>(42) }).await;
        assert_eq!(*result2, Err("failed"));
    }

    #[tokio::test]
    async fn test_concurrent_initialization() {
        let lazy = Arc::new(ALazy::new());
        let counter = Arc::new(AtomicUsize::new(0));

        // Spawn multiple tasks that try to initialize
        let mut handles = vec![];
        for _ in 0..10 {
            let lazy_clone = lazy.clone();
            let counter_clone = counter.clone();
            let handle = tokio::spawn(async move {
                lazy_clone
                    .get_or_init(|| async {
                        // Increment counter to track how many times init is called
                        counter_clone.fetch_add(1, Ordering::SeqCst);
                        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                        Ok::<_, ()>(42)
                    })
                    .await;
            });
            handles.push(handle);
        }

        // Wait for all tasks
        for handle in handles {
            handle.await.unwrap();
        }

        // Initialization should only happen once
        assert_eq!(counter.load(Ordering::SeqCst), 1);

        // Value should be correct
        assert_eq!(*lazy.get().unwrap(), Ok(42));
    }

    #[tokio::test]
    async fn test_fast_path_after_init() {
        let lazy = ALazy::new();
        lazy.get_or_init(|| async { Ok::<_, ()>(42) }).await;

        // Fast path should work
        assert_eq!(*lazy.get().unwrap(), Ok(42));
    }

    #[tokio::test]
    async fn test_cancelled_initializer_releases_claim() {
        let lazy = Arc::new(ALazy::<i32, ()>::new());
        let lazy_clone = lazy.clone();
        let leader = tokio::spawn(async move {
            lazy_clone
                .get_or_init(|| async {
                    tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
                    Ok(1)
                })
                .await;
        });
        tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
        assert!(lazy.is_initializing());

        leader.abort();
        let _ = leader.await;

        // The claim must have been released; a new caller must complete, not hang.
        let result = tokio::time::timeout(
            tokio::time::Duration::from_secs(1),
            lazy.get_or_init(|| async { Ok::<_, ()>(2) }),
        )
        .await
        .expect("get_or_init hung after initializer was cancelled");
        assert_eq!(*result, Ok(2));
    }

    #[tokio::test]
    async fn test_waiter_takes_over_after_cancelled_initializer() {
        let lazy = Arc::new(ALazy::<i32, ()>::new());
        let lazy_clone = lazy.clone();
        let leader = tokio::spawn(async move {
            lazy_clone
                .get_or_init(|| async {
                    tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
                    Ok(1)
                })
                .await;
        });
        tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;

        // Park a waiter while the leader holds the claim.
        let lazy_clone = lazy.clone();
        let waiter = tokio::spawn(async move {
            *lazy_clone.get_or_init(|| async { Ok::<_, ()>(2) }).await
        });
        tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;

        leader.abort();
        let _ = leader.await;

        let result = tokio::time::timeout(tokio::time::Duration::from_secs(1), waiter)
            .await
            .expect("waiter hung after initializer was cancelled")
            .unwrap();
        assert_eq!(result, Ok(2));
    }

    #[tokio::test]
    async fn test_panicking_initializer_releases_claim() {
        let lazy = Arc::new(ALazy::<i32, ()>::new());
        let lazy_clone = lazy.clone();
        let leader = tokio::spawn(async move {
            lazy_clone
                .get_or_init(|| async { panic!("init panicked") })
                .await;
        });
        assert!(leader.await.is_err());

        let result = tokio::time::timeout(
            tokio::time::Duration::from_secs(1),
            lazy.get_or_init(|| async { Ok::<_, ()>(7) }),
        )
        .await
        .expect("get_or_init hung after initializer panicked");
        assert_eq!(*result, Ok(7));
    }

    #[tokio::test]
    async fn test_is_initializing() {
        let lazy = Arc::new(ALazy::new());
        let lazy_clone = lazy.clone();

        // Start initialization in background
        let handle = tokio::spawn(async move {
            lazy_clone
                .get_or_init(|| async {
                    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                    Ok::<_, ()>(42)
                })
                .await;
        });

        // Give the task time to start initializing
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Should be initializing
        assert!(lazy.is_initializing());

        // Wait for completion
        handle.await.unwrap();

        // Should no longer be initializing
        assert!(!lazy.is_initializing());
    }
}