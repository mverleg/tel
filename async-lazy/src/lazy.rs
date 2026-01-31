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

    /// Get or initialize the value.
    ///
    /// If the value is already initialized (success or failure), returns it immediately.
    /// If another task is initializing, waits for completion.
    /// If not initialized, calls the provided function to initialize.
    ///
    /// The initialization function is only called once across all tasks.
    pub async fn get_or_init<F, Fut>(&self, init: F) -> &Result<T, E>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T, E>>,
    {
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
                // We claimed it - do the initialization
                let result = init().await;
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
                unsafe { (*self.value.get()).as_ref().unwrap() }
            }
            Err(_) => {
                // Someone else is initializing or it got initialized - wait for completion
                self.wait().await
            }
        }
    }

    /// Wait for initialization to complete, then return the value.
    /// Returns the value once initialization succeeds or fails.
    async fn wait(&self) -> &Result<T, E> {
        loop {
            match self.state.load(Ordering::Acquire) {
                FILLED | FAILED => {
                    // SAFETY: State is terminal, value is set
                    return unsafe { (*self.value.get()).as_ref().unwrap() };
                }
                INITIALIZING => {
                    // Wait for notification
                    self.notify.notified().await;
                    // Loop to check state again after waking
                }
                EMPTY => {
                    // Initialization was never started or was reset - this shouldn't happen
                    // in normal usage but we handle it by waiting
                    self.notify.notified().await;
                }
                _ => unreachable!("Invalid state"),
            }
        }
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