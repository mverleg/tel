//! Persistent-cache tests (plans/roadmap.md Phase 3 item 12): a
//! `Compiler::with_disk_cache` warms the *next* process over the same cache
//! directory. This is Scenario A of docs/cache-invalidation-problem.md
//! (revert recomputes nothing) lifted across a process boundary — the whole
//! point of the disk tier — plus the failure modes: version mismatch and
//! corruption degrade to a cold-but-correct compile, and `Compiler::new`
//! stays hermetic.

use sandbox::{Compiler, Printer};
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

struct RecordingPrinter {
    out: Arc<Mutex<Vec<String>>>,
}

impl Printer for RecordingPrinter {
    fn print(&self, message: &str) {
        self.out.lock().unwrap().push(message.to_string());
    }
}

fn recording_compiler_with_cache(cache_dir: &Path) -> (Compiler, Arc<Mutex<Vec<String>>>) {
    let out = Arc::new(Mutex::new(Vec::new()));
    let printer: Arc<dyn Printer> = Arc::new(RecordingPrinter { out: out.clone() });
    (Compiler::with_disk_cache(printer, cache_dir).unwrap(), out)
}

fn last_output(out: &Arc<Mutex<Vec<String>>>) -> String {
    out.lock().unwrap().last().cloned().unwrap_or_default()
}

/// (computed parses, resolves, monos) — work actually done, cache hits
/// (memory *or* disk) excluded.
fn computed(compiler: &Compiler) -> (usize, usize, usize) {
    (
        compiler.computed_parse_count(),
        compiler.computed_resolve_count(),
        compiler.computed_mono_count(),
    )
}

fn write_project(dir: &Path) -> String {
    let main = dir.join("main.telsb");
    let dep = dir.join("dep.telsb");
    fs::write(&main, "(import dep)\n(print (call dep 20))\n").unwrap();
    fs::write(&dep, "(+ (arg 1) 1)\n").unwrap();
    main.to_str().unwrap().to_string()
}

/// The core payoff: a fresh compiler over an existing cache directory serves
/// every phase from disk. Compile in one `Compiler`, drop it (joining the
/// writer thread so all entries are flushed), then a *second* `Compiler`
/// over the same cache dir recomputes nothing — while still producing the
/// right answer. `leaf_read_count > 0` confirms the batch stance still
/// probes inputs; only *computation* is skipped.
#[tokio::test]
async fn warm_restart_recomputes_nothing() {
    let project = TempDir::new().unwrap();
    let cache = TempDir::new().unwrap();
    let path = write_project(project.path());

    {
        let (mut compiler, out) = recording_compiler_with_cache(cache.path());
        compiler.run(&path, false).await.unwrap();
        assert_eq!(last_output(&out), "21");
        // drop flushes the disk writer
    }

    let (mut compiler, out) = recording_compiler_with_cache(cache.path());
    compiler.run(&path, false).await.unwrap();
    assert_eq!(last_output(&out), "21");
    assert_eq!(
        computed(&compiler),
        (0, 0, 0),
        "a warm restart over the same cache must recompute nothing",
    );
    assert!(compiler.leaf_read_count() > 0, "the batch stance still probes leaf inputs");
}

/// Deterministic errors are answers too, so they persist and warm the next
/// process just like successes.
#[tokio::test]
async fn warm_restart_serves_cached_errors() {
    let project = TempDir::new().unwrap();
    let cache = TempDir::new().unwrap();
    let main = project.path().join("main.telsb");
    // Undefined function: a terminal resolve error.
    fs::write(&main, "(print (call nowhere 1))\n").unwrap();
    let path = main.to_str().unwrap();

    {
        let (mut compiler, _out) = recording_compiler_with_cache(cache.path());
        assert!(compiler.run(path, false).await.is_err());
    }

    let (mut compiler, _out) = recording_compiler_with_cache(cache.path());
    assert!(compiler.run(path, false).await.is_err(), "the cached error must still be an error");
    assert_eq!(
        computed(&compiler).0,
        0,
        "the parse answer behind the cached error must be served from disk",
    );
}

/// An unusable cache directory (here: a data file corrupted past the point
/// LMDB can open it) surfaces as a clean `Err` from `with_disk_cache`, never
/// a panic. This is exactly the signal the daemon degrades on — it falls
/// back to a memory-only `Compiler::new` (see sandbox-daemon). Value-level
/// corruption (a decodable env with garbage in one entry) is the softer
/// decode-as-miss path, unit-tested in src/disk.rs.
#[tokio::test]
async fn unusable_cache_errors_cleanly_rather_than_panicking() {
    let project = TempDir::new().unwrap();
    let cache = TempDir::new().unwrap();
    let path = write_project(project.path());

    {
        let (mut compiler, out) = recording_compiler_with_cache(cache.path());
        compiler.run(&path, false).await.unwrap();
        assert_eq!(last_output(&out), "21");
    }

    // Truncate the data file to junk: LMDB can no longer recognize it.
    fs::write(cache.path().join("data.mdb"), b"not an lmdb file").unwrap();

    let printer: Arc<dyn Printer> = Arc::new(RecordingPrinter { out: Arc::new(Mutex::new(Vec::new())) });
    let opened = Compiler::with_disk_cache(printer, cache.path());
    assert!(opened.is_err(), "an unopenable cache must be a clean Err, letting the caller degrade");
}

/// The hermetic contract: `Compiler::new` never persists, so two of them
/// over the same sources both compile cold.
#[tokio::test]
async fn plain_compiler_stays_cold() {
    let project = TempDir::new().unwrap();
    let path = write_project(project.path());

    let out = Arc::new(Mutex::new(Vec::new()));
    let printer: Arc<dyn Printer> = Arc::new(RecordingPrinter { out: out.clone() });
    let mut first = Compiler::new(printer);
    first.run(&path, false).await.unwrap();
    let first_computed = computed(&first);
    assert!(first_computed.0 > 0);

    let out2 = Arc::new(Mutex::new(Vec::new()));
    let printer2: Arc<dyn Printer> = Arc::new(RecordingPrinter { out: out2.clone() });
    let mut second = Compiler::new(printer2);
    second.run(&path, false).await.unwrap();
    assert_eq!(
        computed(&second),
        first_computed,
        "a fresh in-memory compiler shares nothing with the previous one",
    );
    assert_eq!(last_output(&out2), "21");
}
