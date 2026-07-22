//! Phase C single-flight for resolve (plans/concurrency-and-eviction.md
//! Decision 4). Concurrent demands for one resolve content key coalesce onto a
//! single computation via the `async-lazy` claim/await primitive, instead of
//! racing and discarding the loser's work (first-write-wins). The unit-level
//! coalescing and panic-abort semantics are pinned in `async-lazy`'s
//! `cache::tests`; here we check the property end-to-end through the resolver.

use sandbox::{Compiler, Printer};
use std::fs;
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

fn recording_compiler() -> (Compiler, Arc<Mutex<Vec<String>>>) {
    let out = Arc::new(Mutex::new(Vec::new()));
    let printer: Arc<dyn Printer> = Arc::new(RecordingPrinter { out: out.clone() });
    (Compiler::new(printer), out)
}

fn last_output(out: &Arc<Mutex<Vec<String>>>) -> String {
    out.lock().unwrap().last().cloned().unwrap_or_default()
}

/// A diamond: `main` imports `a` and `b`, both of which import the shared `c`.
/// Resolving `a` and `b` runs on concurrent tasks (`resolve_each` spawns), so
/// both demand `resolve(c)` at once. Single-flight coalesces them: `c` is
/// resolved exactly once, so the total resolve computations equal the four
/// distinct files — never five. Run on a multi-threaded runtime so the demands
/// for `c` can truly overlap.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn diamond_import_resolves_shared_dep_once() {
    let dir = TempDir::new().unwrap();
    let main = dir.path().join("main.telsb");
    let a = dir.path().join("a.telsb");
    let b = dir.path().join("b.telsb");
    let c = dir.path().join("c.telsb");
    let path = main.to_str().unwrap();

    fs::write(&main, "(import a)\n(import b)\n(print (+ (call a 1) (call b 1)))\n").unwrap();
    fs::write(&a, "(import c)\n(+ (call c (arg 1)) 1)\n").unwrap();
    fs::write(&b, "(import c)\n(+ (call c (arg 1)) 2)\n").unwrap();
    fs::write(&c, "(* (arg 1) 10)\n").unwrap();

    let (mut compiler, out) = recording_compiler();
    compiler.run(path, false).await.unwrap();

    // (10+1) + (10+2) = 23
    assert_eq!(last_output(&out), "23");
    assert_eq!(
        compiler.computed_resolve_count(),
        4,
        "the four distinct files resolve once each — the shared `c` is not \
         resolved twice despite two concurrent importers",
    );
}
