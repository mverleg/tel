//! File-change monitoring: the event source that drives watch-stance
//! compiles (TODO.md "OS file watcher", plans/daemon.md).
//!
//! The split: a [`FileMonitor`] backend only *produces* raw change events;
//! delivery — batching, dedup, closed-stream detection — lives once in
//! [`ChangeStream`], shared by every backend. Two backends exist:
//! [`DiskMonitor`] (the `notify` crate, i.e. inotify/FSEvents/etc.) and
//! [`MockMonitor`] (tests inject events by hand). Future backends (editor
//! overlays, a polling fallback, a virtual FS for the pluggable source
//! backends of roadmap Phase 3) implement [`FileMonitor`] and feed the same
//! stream type, so drivers never care where events come from.
//!
//! Two contracts carry over from the watch stance
//! (doc/book/src/19a-compiler-internals/07-execution-and-recovery.md, [`crate::Compiler::run_watch`]):
//!
//! * **Extra events are hints.** A spurious or duplicate event costs one
//!   re-derive that early cutoff stops immediately, so backends should
//!   over-report rather than filter cleverly.
//! * **Missed events are stale serves.** The watch contract requires every
//!   change to be announced, so a backend must never drop an event it saw.
//!   (When in doubt, [`crate::Compiler::run`] remains always-correct.)
//!
//! **Path identity.** The engine interns paths textually: `invalidate(p)`
//! only hits the leaf that was parsed as exactly `p`. Events must therefore
//! carry the same form of path the compiler was given — in practice, run the
//! compiler on a canonical absolute entry path (imports inherit its
//! directory) and watch the canonical root. [`DiskMonitor::watch`]
//! canonicalizes the root before registering it so the OS reports canonical
//! paths; callers are responsible for the entry path they compile.

use std::fmt;
use std::path::{Path, PathBuf};
use log::warn;
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;

/// Error from a monitor backend (root missing, OS watch limit reached, …).
/// Backend-specific error types are rendered to a message here so backend
/// crates never leak into the public API.
#[derive(Debug)]
pub struct MonitorError {
    message: String,
}

impl fmt::Display for MonitorError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "monitor error: {}", self.message)
    }
}

impl std::error::Error for MonitorError {}

/// A source of file-change events. Implementations control *which* trees are
/// observed; the events themselves arrive on the [`ChangeStream`] returned by
/// the implementation's constructor.
pub trait FileMonitor: Send {
    /// Start observing `root` recursively. Watching the same root twice is
    /// backend-defined but must not duplicate the stream's semantics (extra
    /// events are hints anyway).
    fn watch(&mut self, root: &Path) -> Result<(), MonitorError>;

    /// Stop observing `root`. Events already emitted stay in the stream.
    fn unwatch(&mut self, root: &Path) -> Result<(), MonitorError>;
}

/// The receiving end of a monitor: changed paths, delivered in coalesced
/// batches. One stream per monitor, created by the backend's constructor.
pub struct ChangeStream {
    rx: mpsc::UnboundedReceiver<PathBuf>,
}

impl ChangeStream {
    fn new() -> (mpsc::UnboundedSender<PathBuf>, ChangeStream) {
        let (tx, rx) = mpsc::unbounded_channel();
        (tx, ChangeStream { rx })
    }

    /// Wait for at least one change, then drain everything already pending
    /// into one deduplicated batch — a save-all touching twenty files becomes
    /// one invalidate wave and one compile, not twenty.
    ///
    /// Returns `None` once the monitor (and any handles) are dropped and all
    /// buffered events have been consumed — the loop-termination signal.
    pub async fn next_batch(&mut self) -> Option<Vec<PathBuf>> {
        let first = self.rx.recv().await?;
        let mut batch = vec![first];
        while let Ok(path) = self.rx.try_recv() {
            if !batch.contains(&path) {
                batch.push(path);
            }
        }
        Some(batch)
    }
}

/// Test/mock backend: events are whatever the test injects, whenever it
/// injects them. `watch`/`unwatch` only record roots for assertions —
/// [`emit`](MockMonitor::emit) is deliberately unfiltered, so a test can
/// also exercise driver behaviour on out-of-tree events.
pub struct MockMonitor {
    tx: mpsc::UnboundedSender<PathBuf>,
    watched: Vec<PathBuf>,
}

/// Cloneable injector for [`MockMonitor`], so a test can move the monitor
/// into one task and emit from another. The stream closes when the monitor
/// *and* every handle are dropped.
#[derive(Clone)]
pub struct MockHandle {
    tx: mpsc::UnboundedSender<PathBuf>,
}

impl MockMonitor {
    pub fn new() -> (MockMonitor, ChangeStream) {
        let (tx, stream) = ChangeStream::new();
        (MockMonitor { tx, watched: Vec::new() }, stream)
    }

    pub fn handle(&self) -> MockHandle {
        MockHandle { tx: self.tx.clone() }
    }

    /// Inject a change event, as if `path` was edited on disk.
    pub fn emit(&self, path: impl Into<PathBuf>) {
        let _ = self.tx.send(path.into());
    }

    /// Roots passed to [`watch`](FileMonitor::watch) and not yet unwatched.
    pub fn watched_roots(&self) -> &[PathBuf] {
        &self.watched
    }
}

impl MockHandle {
    /// Inject a change event, as if `path` was edited on disk.
    pub fn emit(&self, path: impl Into<PathBuf>) {
        let _ = self.tx.send(path.into());
    }
}

impl FileMonitor for MockMonitor {
    fn watch(&mut self, root: &Path) -> Result<(), MonitorError> {
        self.watched.push(root.to_path_buf());
        Ok(())
    }

    fn unwatch(&mut self, root: &Path) -> Result<(), MonitorError> {
        match self.watched.iter().position(|r| r == root) {
            Some(idx) => {
                self.watched.remove(idx);
                Ok(())
            }
            None => Err(MonitorError { message: format!("not watching {}", root.display()) }),
        }
    }
}

/// Real-disk backend over the `notify` crate (inotify on Linux). Dropping it
/// stops the OS watches and closes the stream.
pub struct DiskMonitor {
    watcher: RecommendedWatcher,
}

impl DiskMonitor {
    pub fn new() -> Result<(DiskMonitor, ChangeStream), MonitorError> {
        let (tx, stream) = ChangeStream::new();
        let watcher = notify::recommended_watcher(move |result: Result<notify::Event, notify::Error>| {
            let event = match result {
                Ok(event) => event,
                Err(e) => {
                    // A backend error (overflow, dropped watch) may mean
                    // missed events, which the watch contract can't absorb;
                    // surface it loudly. The daemon-level recovery is a full
                    // `run` (batch stance), which needs no events.
                    warn!("file monitor backend error: {}", e);
                    return;
                }
            };
            // Access events must not be forwarded: the compiler *reads* the
            // watched files while recomputing, so forwarding reads would make
            // every watch run schedule the next one, forever. Everything else
            // (create/modify/remove/rename/unknown) is forwarded — extra
            // events are hints.
            if matches!(event.kind, EventKind::Access(_)) {
                return;
            }
            for path in event.paths {
                let _ = tx.send(path);
            }
        }).map_err(|e| MonitorError { message: e.to_string() })?;
        Ok((DiskMonitor { watcher }, stream))
    }
}

impl FileMonitor for DiskMonitor {
    fn watch(&mut self, root: &Path) -> Result<(), MonitorError> {
        // Canonicalize so the OS reports canonical paths — see the path
        // identity contract in the module docs.
        let root = root.canonicalize()
            .map_err(|e| MonitorError { message: format!("{}: {}", root.display(), e) })?;
        self.watcher.watch(&root, RecursiveMode::Recursive)
            .map_err(|e| MonitorError { message: e.to_string() })
    }

    fn unwatch(&mut self, root: &Path) -> Result<(), MonitorError> {
        let root = root.canonicalize()
            .map_err(|e| MonitorError { message: format!("{}: {}", root.display(), e) })?;
        self.watcher.unwatch(&root)
            .map_err(|e| MonitorError { message: e.to_string() })
    }
}
