//! File monitor tests (src/monitor.rs): the mock backend driving
//! `Compiler::run_watch_loop`, the batching/dedup semantics of
//! `ChangeStream`, and the disk backend observing real edits end-to-end.
//!
//! The compiler-facing assertions mirror tests/invalidation.rs: after a
//! monitored edit, the watch run touches exactly the affected cone.

use sandbox::{Compiler, Error, Printer};
use sandbox::monitor::{ChangeStream, DiskMonitor, FileMonitor, MockMonitor};
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tokio::time::timeout;

struct RecordingPrinter {
    out: Arc<Mutex<Vec<String>>>,
}

impl Printer for RecordingPrinter {
    fn print(&self, message: &str) {
        self.out.lock().unwrap().push(message.to_string());
    }
}

fn recording_compiler() -> (Compiler, Arc<Mutex<Vec<String>>>) {
    let out = Arc::new(Mutex::new(Vec::new()));
    let printer: Arc<dyn Printer> = Arc::new(RecordingPrinter { out: out.clone() });
    (Compiler::new(printer), out)
}

fn last_output(out: &Arc<Mutex<Vec<String>>>) -> String {
    out.lock().unwrap().last().cloned().unwrap_or_default()
}

/// (leaf reads, computed parses, computed resolves, computed monos)
fn counts(compiler: &Compiler) -> (usize, usize, usize, usize) {
    (
        compiler.leaf_read_count(),
        compiler.computed_parse_count(),
        compiler.computed_resolve_count(),
        compiler.computed_mono_count(),
    )
}

fn delta(before: (usize, usize, usize, usize), after: (usize, usize, usize, usize)) -> (usize, usize, usize, usize) {
    (after.0 - before.0, after.1 - before.1, after.2 - before.2, after.3 - before.3)
}

/// Wait (cooperatively) until `results` holds `n` entries; panics on a
/// deadline instead of hanging the test run forever.
async fn await_results(results: &Arc<Mutex<Vec<Result<(), Error>>>>, n: usize) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while results.lock().unwrap().len() < n {
        assert!(Instant::now() < deadline, "timed out waiting for {} watch-loop results", n);
        tokio::task::yield_now().await;
    }
}

/// A three-file project: main imports a "hot" and a "cold" dependency, so a
/// monitored edit to hot must leave cold untouched at every phase.
fn hot_cold_project(dir: &Path) -> (String, std::path::PathBuf) {
    let main = dir.join("main.telsb");
    let hot = dir.join("hot.telsb");
    let cold = dir.join("cold.telsb");
    fs::write(&main, "(import /hot)\n(import /cold)\n(print (+ (call hot 1) (call cold 1)))\n").unwrap();
    fs::write(&hot, "(+ (arg 1) 10)\n").unwrap();
    fs::write(&cold, "(+ (arg 1) 100)\n").unwrap();
    (main.to_str().unwrap().to_string(), hot)
}

/// The full loop against the mock backend: initial run, then an injected
/// edit event triggers exactly one cone-shaped recompute wave, and dropping
/// the monitor ends the loop.
#[tokio::test]
async fn mock_events_drive_watch_loop() {
    let dir = TempDir::new().unwrap();
    let (path, hot) = hot_cold_project(dir.path());

    let (mut monitor, mut events) = MockMonitor::new();
    monitor.watch(dir.path()).unwrap();
    assert_eq!(monitor.watched_roots(), &[dir.path().to_path_buf()]);
    let handle = monitor.handle();

    let (mut compiler, out) = recording_compiler();
    let results: Arc<Mutex<Vec<Result<(), Error>>>> = Arc::new(Mutex::new(Vec::new()));

    let loop_results = results.clone();
    let loop_fut = compiler.run_watch_loop(&path, &mut events, move |result| {
        loop_results.lock().unwrap().push(result);
    });

    let out_after_first = Arc::new(Mutex::new(String::new()));
    let driver_snapshot = out_after_first.clone();
    let driver = async {
        await_results(&results, 1).await;
        *driver_snapshot.lock().unwrap() = last_output(&out);

        // Edit the hot dependency and announce it the way a watcher would.
        fs::write(&hot, "(+ (arg 1) 20)\n").unwrap();
        handle.emit(&hot);
        await_results(&results, 2).await;

        // Closing every sender ends the stream, hence the loop.
        drop(handle);
        drop(monitor);
    };

    tokio::join!(loop_fut, driver);

    assert_eq!(*out_after_first.lock().unwrap(), "112");
    assert_eq!(last_output(&out), "122");
    let results = results.lock().unwrap();
    assert_eq!(results.len(), 2, "one initial run plus one per event batch");
    assert!(results.iter().all(|r| r.is_ok()));
}

/// Cone discipline through the loop, not just through `invalidate` directly:
/// the second wave re-reads and re-parses only the edited file, re-resolves
/// it and its importer, and re-checks only the edited function.
#[tokio::test]
async fn watch_loop_recomputes_only_the_affected_cone() {
    let dir = TempDir::new().unwrap();
    let (path, hot) = hot_cold_project(dir.path());

    let (monitor, mut events) = MockMonitor::new();
    let (mut compiler, out) = recording_compiler();

    compiler.run_watch(&path, false).await.unwrap();
    assert_eq!(last_output(&out), "112");
    let before = counts(&compiler);

    fs::write(&hot, "(+ (arg 1) 20)\n").unwrap();
    monitor.emit(&hot);
    drop(monitor);

    // Drive one wave by hand (batch → invalidate → watch run) so the count
    // deltas below measure exactly that wave and nothing else.
    let batch = events.next_batch().await.expect("event was sent before drop");
    for changed in &batch {
        compiler.invalidate(changed.to_str().unwrap());
    }
    compiler.run_watch(&path, false).await.unwrap();

    assert_eq!(last_output(&out), "122");
    let (reads, parses, resolves, monos) = delta(before, counts(&compiler));
    assert_eq!(reads, 1, "only the changed file may be read");
    assert_eq!(parses, 1, "only the changed file re-parses");
    assert_eq!(resolves, 2, "the changed file and its importer re-resolve");
    assert_eq!(monos, 1, "only the changed function re-checks");
}

/// Batching semantics: everything pending is drained into one deduplicated
/// batch, and a fully closed stream yields `None`.
#[tokio::test]
async fn next_batch_coalesces_and_dedups() {
    let (monitor, mut events) = MockMonitor::new();
    monitor.emit("/w/a.telsb");
    monitor.emit("/w/b.telsb");
    monitor.emit("/w/a.telsb");
    monitor.emit("/w/a.telsb");
    drop(monitor);

    let batch = events.next_batch().await.unwrap();
    assert_eq!(batch, vec![
        std::path::PathBuf::from("/w/a.telsb"),
        std::path::PathBuf::from("/w/b.telsb"),
    ]);
    assert!(events.next_batch().await.is_none(), "closed and drained stream must end the loop");
}

#[tokio::test]
async fn mock_unwatch_requires_watched_root() {
    let (mut monitor, _events) = MockMonitor::new();
    monitor.watch(Path::new("/w")).unwrap();
    monitor.unwatch(Path::new("/w")).unwrap();
    assert!(monitor.unwatch(Path::new("/w")).is_err());
    assert!(monitor.watched_roots().is_empty());
}

/// Grace period for real OS event delivery; inotify is typically sub-ms,
/// the margin is for loaded CI machines.
const DISK_EVENT_TIMEOUT: Duration = Duration::from_secs(10);

async fn next_batch_with_timeout(events: &mut ChangeStream) -> Vec<std::path::PathBuf> {
    timeout(DISK_EVENT_TIMEOUT, events.next_batch()).await
        .expect("no disk event within the grace period")
        .expect("monitor is still alive, stream cannot be closed")
}

/// The disk backend observes a real edit. Canonicalized paths throughout —
/// the path identity contract of the module docs.
#[tokio::test]
async fn disk_monitor_reports_real_edits() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let file = root.join("watched.telsb");
    fs::write(&file, "(print 1)\n").unwrap();

    let (mut monitor, mut events) = DiskMonitor::new().unwrap();
    monitor.watch(&root).unwrap();

    fs::write(&file, "(print 2)\n").unwrap();
    let batch = next_batch_with_timeout(&mut events).await;
    assert!(
        batch.iter().any(|p| p == &file),
        "edit to {} must be reported; got {:?}", file.display(), batch,
    );

    monitor.unwatch(&root).unwrap();
}

/// End-to-end on real disk: compile, edit the file on disk, feed the OS
/// events through invalidate, recompile — the output changes and only the
/// affected cone recomputed. This is the TODO.md watcher wired up for real.
#[tokio::test]
async fn disk_events_invalidate_the_compiler() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let (path, hot) = hot_cold_project(&root);

    let (mut monitor, mut events) = DiskMonitor::new().unwrap();
    monitor.watch(&root).unwrap();

    let (mut compiler, out) = recording_compiler();
    compiler.run_watch(&path, false).await.unwrap();
    assert_eq!(last_output(&out), "112");
    let before = counts(&compiler);

    fs::write(&hot, "(+ (arg 1) 20)\n").unwrap();
    // One edit may surface as several OS events (create/data/metadata);
    // keep draining batches until the edited file has been announced —
    // extra events are hints and cost nothing below.
    let deadline = Instant::now() + DISK_EVENT_TIMEOUT;
    let mut announced = Vec::new();
    while !announced.contains(&hot) {
        assert!(Instant::now() < deadline, "edit never announced; got {:?}", announced);
        announced.extend(next_batch_with_timeout(&mut events).await);
    }
    for changed in &announced {
        compiler.invalidate(changed.to_str().unwrap());
    }

    compiler.run_watch(&path, false).await.unwrap();
    assert_eq!(last_output(&out), "122");
    let (reads, parses, resolves, monos) = delta(before, counts(&compiler));
    assert_eq!(reads, 1, "only the changed file may be read");
    assert_eq!(parses, 1, "only the changed file re-parses");
    assert_eq!(resolves, 2, "the changed file and its importer re-resolve");
    assert_eq!(monos, 1, "only the changed function re-checks");
}
