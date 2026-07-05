//! Incremental-compile-from-main tests (plans/roadmap.md item 9).
//!
//! A `Compiler` is a persistent process: each `run` is a demand-driven pull
//! from the entry point that re-derives content keys top-down and stops at
//! cache hits. These tests pin down the three properties that mode adds on
//! top of the per-phase caching tests in `cache_invalidation.rs`: a reverted
//! edit costs nothing (Scenario A, across all phases), only what main
//! transitively demands is touched at all, and nothing survives the compiler
//! itself (no hidden `'static` state).

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

fn counts(compiler: &Compiler) -> (usize, usize, usize) {
    (
        compiler.computed_parse_count(),
        compiler.computed_resolve_count(),
        compiler.computed_mono_count(),
    )
}

/// Scenario A from docs/cache-invalidation-problem.md, across every cached
/// phase: edit, compile, revert, compile — the post-revert compile recomputes
/// *nothing* (zero parse answers, zero resolves, zero mono checks); every key
/// it re-derives was already seen.
#[tokio::test]
async fn scenario_a_revert_recomputes_nothing() {
    let dir = TempDir::new().unwrap();
    let main = dir.path().join("main.telsb");
    let dep = dir.path().join("dep.telsb");
    let path = main.to_str().unwrap();
    fs::write(&main, "(import dep)\n(print (call dep 20))\n").unwrap();
    fs::write(&dep, "(+ (arg 1) 1)\n").unwrap();

    let (mut compiler, out) = recording_compiler();
    compiler.run(path, false).await.unwrap();
    assert_eq!(last_output(&out), "21");

    // A semantic edit recomputes its chain (and must take effect)...
    fs::write(&dep, "(+ (arg 1) 2)\n").unwrap();
    compiler.run(path, false).await.unwrap();
    assert_eq!(last_output(&out), "22");
    let after_edit = counts(&compiler);

    // ...and reverting to the original bytes is pure cache: the leaf is read
    // (to derive its digest — that is the unavoidable input probe), but no
    // phase computes anything, and the original program runs again.
    fs::write(&dep, "(+ (arg 1) 1)\n").unwrap();
    compiler.run(path, false).await.unwrap();
    assert_eq!(last_output(&out), "21");
    assert_eq!(
        counts(&compiler),
        after_edit,
        "a reverted project must be a full cache hit: zero parse/resolve/mono recompute"
    );
}

/// Pull means *demand*-driven: a file that main no longer (transitively)
/// imports is not touched on the next run — not even re-parsed, although its
/// bytes changed while it was out of the graph.
#[tokio::test]
async fn file_dropped_from_import_graph_is_not_demanded() {
    let dir = TempDir::new().unwrap();
    let main = dir.path().join("main.telsb");
    let helper = dir.path().join("helper.telsb");
    let path = main.to_str().unwrap();
    fs::write(&main, "(import helper)\n(print (call helper 1))\n").unwrap();
    fs::write(&helper, "(+ (arg 1) 41)\n").unwrap();

    let (mut compiler, out) = recording_compiler();
    compiler.run(path, false).await.unwrap();
    assert_eq!(last_output(&out), "42");
    let (parses, _, _) = counts(&compiler);

    // Drop the import AND change the helper. If the second run demanded the
    // helper at all, its changed bytes would force a parse recompute.
    fs::write(&main, "(print 7)\n").unwrap();
    fs::write(&helper, "(+ (arg 1) 999)\n").unwrap();
    compiler.run(path, false).await.unwrap();
    assert_eq!(last_output(&out), "7");
    assert_eq!(
        compiler.computed_parse_count(),
        parses + 1,
        "only the edited main may be demanded; the dropped helper must not be re-parsed"
    );
}

/// Nothing outlives the `Compiler`: a fresh one starts genuinely cold (its
/// counters and caches at zero) and recomputes the same project from scratch.
/// Guards against hidden `'static`/leaked state now that `Box::leak` is gone.
#[tokio::test]
async fn dropped_compiler_leaves_no_shared_state_behind() {
    let dir = TempDir::new().unwrap();
    let main = dir.path().join("main.telsb");
    let dep = dir.path().join("dep.telsb");
    let path = main.to_str().unwrap();
    fs::write(&main, "(import dep)\n(print (call dep 6))\n").unwrap();
    fs::write(&dep, "(* (arg 1) 7)\n").unwrap();

    let (mut first, out1) = recording_compiler();
    first.run(path, false).await.unwrap();
    assert_eq!(last_output(&out1), "42");
    let first_counts = counts(&first);
    assert!(first_counts.0 > 0, "the cold run must actually compute");
    drop(first);

    let (mut second, out2) = recording_compiler();
    assert_eq!(counts(&second), (0, 0, 0), "a fresh compiler must start cold");
    second.run(path, false).await.unwrap();
    assert_eq!(last_output(&out2), "42");
    assert_eq!(
        counts(&second),
        first_counts,
        "an unchanged project must cost a fresh compiler exactly what it cost the first"
    );
}

/// Restructuring the import graph across runs must not leave zombie edges
/// behind: run 1 has a -> b, run 2 inverts the relationship to b -> a. If the
/// stale a -> b edge survived, the session graph would contain a phantom
/// cycle a -> b -> a and the (debug-asserted) acyclicity check on a
/// *successful* compile would fail. Each re-resolve instead replaces its dep
/// edges with the set re-derived from current content.
#[tokio::test]
async fn import_restructure_leaves_no_zombie_edges() {
    let dir = TempDir::new().unwrap();
    let main = dir.path().join("main.telsb");
    let a = dir.path().join("a.telsb");
    let b = dir.path().join("b.telsb");
    let path = main.to_str().unwrap();

    // Run 1: main -> a -> b.
    fs::write(&main, "(import a)\n(print (call a 1))\n").unwrap();
    fs::write(&a, "(import b)\n(+ (call b (arg 1)) 1)\n").unwrap();
    fs::write(&b, "(* (arg 1) 10)\n").unwrap();

    let (mut compiler, out) = recording_compiler();
    compiler.run(path, false).await.unwrap();
    assert_eq!(last_output(&out), "11");

    // Run 2: main -> b -> a; a is now the leaf. With stale edges this graph
    // would read a -> b -> a.
    fs::write(&main, "(import b)\n(print (call b 1))\n").unwrap();
    fs::write(&b, "(import a)\n(+ (call a (arg 1)) 1)\n").unwrap();
    fs::write(&a, "(* (arg 1) 10)\n").unwrap();
    compiler.run(path, false).await
        .expect("an inverted (acyclic) import graph must compile — a phantom cycle means stale edges survived");
    assert_eq!(last_output(&out), "11");
}
