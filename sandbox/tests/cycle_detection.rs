//! Import-cycle detection tests.
//!
//! Exercises the deadlock-safe detector from `doc/book/src/19a-compiler-internals/08-cycle-detection.md`: each
//! resolution carries the chain of in-progress ancestor resolutions, and a
//! dependency already on that chain is reported as a cycle *before* it is
//! awaited or spawned -- so a genuine cycle errors out deterministically
//! instead of deadlocking the parallel resolver, while legitimate shared
//! dependencies (diamonds) resolve normally.

use sandbox::{Compiler, NoopPrinter};
use std::fs;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;

fn noop_compiler() -> Compiler {
    Compiler::new(Arc::new(NoopPrinter))
}

/// Runs the compile under a timeout: a hang here is precisely the deadlock the
/// ancestor-path detector exists to prevent, so it must fail the test rather
/// than stall the suite.
async fn run_expecting_cycle(path: &str) -> String {
    let mut compiler = noop_compiler();
    let result = tokio::time::timeout(Duration::from_secs(10), compiler.run(path, false))
        .await
        .expect("cycle detection must terminate, not deadlock");
    let err = result.expect_err("an import cycle must fail compilation");
    let msg = err.to_string();
    assert!(
        msg.contains("Cyclic dependency"),
        "cycle must be reported as a cyclic-dependency error, got: {msg}"
    );
    msg
}

#[tokio::test]
async fn self_import_is_reported_as_cycle() {
    let dir = TempDir::new().unwrap();
    let main = dir.path().join("main.telsb");
    fs::write(&main, "(import /main)\n(print 1)\n").unwrap();

    let msg = run_expecting_cycle(main.to_str().unwrap()).await;
    // The chain is main -> main: the path must name the file at both ends.
    assert!(msg.contains("main.telsb"), "cycle path must name the file, got: {msg}");
    assert!(msg.matches("main.telsb").count() >= 2, "self-cycle shows the node twice, got: {msg}");
}

#[tokio::test]
async fn direct_two_node_cycle_is_reported() {
    let dir = TempDir::new().unwrap();
    let main = dir.path().join("main.telsb");
    fs::write(&main, "(import /a)\n(print (call a))\n").unwrap();
    fs::write(dir.path().join("a.telsb"), "(import /b)\n(call b)\n").unwrap();
    fs::write(dir.path().join("b.telsb"), "(import /a)\n(call a)\n").unwrap();

    let msg = run_expecting_cycle(main.to_str().unwrap()).await;
    // The reported chain starts at the first repeated node, so it is exactly
    // a -> b -> a; main (outside the cycle) is not blamed.
    assert!(msg.contains("a.telsb"), "cycle must include a, got: {msg}");
    assert!(msg.contains("b.telsb"), "cycle must include b, got: {msg}");
    assert!(!msg.contains("main.telsb"), "main is not part of the cycle, got: {msg}");
}

#[tokio::test]
async fn transitive_three_node_cycle_is_reported() {
    let dir = TempDir::new().unwrap();
    let main = dir.path().join("main.telsb");
    fs::write(&main, "(import /a)\n(print (call a))\n").unwrap();
    fs::write(dir.path().join("a.telsb"), "(import /b)\n(call b)\n").unwrap();
    fs::write(dir.path().join("b.telsb"), "(import /c)\n(call c)\n").unwrap();
    fs::write(dir.path().join("c.telsb"), "(import /a)\n(call a)\n").unwrap();

    let msg = run_expecting_cycle(main.to_str().unwrap()).await;
    for module in ["a.telsb", "b.telsb", "c.telsb"] {
        assert!(msg.contains(module), "cycle must include {module}, got: {msg}");
    }
}

/// A diamond -- two paths sharing one dependency -- is NOT a cycle. The shared
/// node appears on two different (parallel) ancestor chains but never twice on
/// one chain, so it must resolve and run normally.
#[tokio::test]
async fn diamond_imports_are_not_flagged_as_cycle() {
    let dir = TempDir::new().unwrap();
    let main = dir.path().join("main.telsb");
    fs::write(&main, "(import /b)\n(import /c)\n(print (+ (call b) (call c)))\n").unwrap();
    fs::write(dir.path().join("b.telsb"), "(import /d)\n(call d)\n").unwrap();
    fs::write(dir.path().join("c.telsb"), "(import /d)\n(call d)\n").unwrap();
    fs::write(dir.path().join("d.telsb"), "21\n").unwrap();

    let mut compiler = noop_compiler();
    tokio::time::timeout(Duration::from_secs(10), compiler.run(main.to_str().unwrap(), false))
        .await
        .expect("diamond must not deadlock")
        .expect("diamond imports are legal and must compile");
}

/// Cycles under concurrency: several compiles race over the same cyclic module
/// pair on a multi-threaded runtime. Every one must terminate with a cycle
/// error -- never hang (the old in-progress-marker detector could deadlock or
/// misreport when resolutions overlapped in time).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_compiles_of_a_cycle_all_error_without_hanging() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("x.telsb"), "(import /y)\n(call y)\n").unwrap();
    fs::write(dir.path().join("y.telsb"), "(import /x)\n(call x)\n").unwrap();
    let entry1 = dir.path().join("main1.telsb");
    let entry2 = dir.path().join("main2.telsb");
    fs::write(&entry1, "(import /x)\n(print (call x))\n").unwrap();
    fs::write(&entry2, "(import /y)\n(print (call y))\n").unwrap();

    let (msg1, msg2) = tokio::join!(
        run_expecting_cycle(entry1.to_str().unwrap()),
        run_expecting_cycle(entry2.to_str().unwrap()),
    );
    for msg in [msg1, msg2] {
        assert!(msg.contains("x.telsb"), "cycle must include x, got: {msg}");
        assert!(msg.contains("y.telsb"), "cycle must include y, got: {msg}");
    }
}

/// Diamonds under concurrency: two compiles race over the same shared modules.
/// Both must succeed -- overlapping in-progress resolutions of the same file
/// are deduplicated work at worst, never a cycle.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_diamond_compiles_succeed() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("b.telsb"), "(import /d)\n(call d)\n").unwrap();
    fs::write(dir.path().join("c.telsb"), "(import /d)\n(call d)\n").unwrap();
    fs::write(dir.path().join("d.telsb"), "21\n").unwrap();
    let entry1 = dir.path().join("main1.telsb");
    let entry2 = dir.path().join("main2.telsb");
    fs::write(&entry1, "(import /b)\n(import /c)\n(print (+ (call b) (call c)))\n").unwrap();
    fs::write(&entry2, "(import /c)\n(import /b)\n(print (+ (call c) (call b)))\n").unwrap();

    let mut compiler1 = noop_compiler();
    let mut compiler2 = noop_compiler();
    let (r1, r2) = tokio::join!(
        tokio::time::timeout(Duration::from_secs(10), compiler1.run(entry1.to_str().unwrap(), false)),
        tokio::time::timeout(Duration::from_secs(10), compiler2.run(entry2.to_str().unwrap(), false)),
    );
    r1.expect("must not deadlock").expect("diamond must compile");
    r2.expect("must not deadlock").expect("diamond must compile");
}
