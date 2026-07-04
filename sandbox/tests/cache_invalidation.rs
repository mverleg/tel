//! Cross-run cache invalidation tests.
//!
//! Exercises the content-addressed parse cache described in
//! `docs/cache-invalidation-problem.md`: a `Compiler` keeps its cache across
//! runs, so an unchanged/reverted file is not re-parsed, while a changed file is
//! re-parsed and never served stale.

use sandbox::{Compiler, Printer};
use std::fs;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

/// A `Printer` that records everything printed, so a test can assert which
/// program actually executed (proving results are not stale).
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
    let printer: &'static dyn Printer = Box::leak(Box::new(RecordingPrinter { out: out.clone() }));
    (Compiler::new(printer), out)
}

fn last_output(out: &Arc<Mutex<Vec<String>>>) -> String {
    out.lock().unwrap().last().cloned().unwrap_or_default()
}

/// The core scenario from the design doc: within one long-lived compiler,
/// changing a file's content re-parses it (fresh cache key, no stale result),
/// and reverting to previously-seen content hits the cache again (Scenario A).
#[tokio::test]
async fn cross_run_cache_invalidates_and_reuses_by_content() {
    let dir = TempDir::new().unwrap();
    let main = dir.path().join("main.telsb");
    let path = main.to_str().unwrap();

    let (compiler, out) = recording_compiler();

    // Run 1: content v1. First parse -> exactly one cached content.
    fs::write(&main, "(print 42)\n").unwrap();
    compiler.run(path, false).await.unwrap();
    assert_eq!(last_output(&out), "42");
    assert_eq!(compiler.cached_parse_count(), 1);

    // Run 2: content changed. Must re-parse (new digest -> new cache entry) and
    // execute the NEW program -- proving the cache never serves stale data.
    fs::write(&main, "(print 43)\n").unwrap();
    compiler.run(path, false).await.unwrap();
    assert_eq!(last_output(&out), "43", "changed content must not be served stale");
    assert_eq!(
        compiler.cached_parse_count(),
        2,
        "changed content must produce a new cache entry"
    );

    // Run 3: revert to v1 (edit-then-revert / branch-switch). Byte-identical
    // content -> identical digest -> cache hit, no new entry.
    fs::write(&main, "(print 42)\n").unwrap();
    compiler.run(path, false).await.unwrap();
    assert_eq!(last_output(&out), "42");
    assert_eq!(
        compiler.cached_parse_count(),
        2,
        "reverting to previously-seen content must hit the cache (Scenario A)"
    );
}

/// The mono cache is chained to the parse stage: its key contains the content
/// digest of the instance's defining file. So editing a file recomputes (only)
/// that file's instances, and reverting it hits the old entries again.
#[tokio::test]
async fn mono_cache_follows_content_per_file() {
    let dir = TempDir::new().unwrap();
    let main = dir.path().join("main.telsb");
    let dbl = dir.path().join("dbl.telsb");
    let path = main.to_str().unwrap();

    let (compiler, out) = recording_compiler();

    // Run 1: two instances get checked and cached: main @ i64 and dbl @ i64.
    fs::write(&main, "(import dbl)\n(print (call dbl 21))\n").unwrap();
    fs::write(&dbl, "(* (arg 1) 2)\n").unwrap();
    compiler.run(path, false).await.unwrap();
    assert_eq!(last_output(&out), "42");
    assert_eq!(compiler.cached_mono_count(), 2);

    // Run 2: dbl's content changed, so dbl @ i64 gets a fresh cache key and is
    // re-checked (the NEW body must execute -- no staleness). main is
    // unchanged, so its instance is reused: exactly one new entry.
    fs::write(&dbl, "(* (arg 1) 3)\n").unwrap();
    compiler.run(path, false).await.unwrap();
    assert_eq!(last_output(&out), "63", "changed content must not be served stale");
    assert_eq!(
        compiler.cached_mono_count(),
        3,
        "only the changed file's instance may produce a new cache entry"
    );

    // Run 3: revert dbl. Both instances hit the cache; no new entries.
    fs::write(&dbl, "(* (arg 1) 2)\n").unwrap();
    compiler.run(path, false).await.unwrap();
    assert_eq!(last_output(&out), "42");
    assert_eq!(
        compiler.cached_mono_count(),
        3,
        "reverting to previously-seen content must hit the mono cache"
    );
}

/// Parse is the leaf query, so its content key hashes the bytes only — the
/// file path is the *logical* id, not a key ingredient
/// (docs/keys-and-invalidation.md). Byte-identical files at two paths
/// therefore share one parse entry (sound: the parse output embeds no paths),
/// while the mono cache, whose output embeds path-based FQs, keeps them apart.
#[tokio::test]
async fn identical_content_at_two_paths_shares_parse_but_not_mono() {
    let dir = TempDir::new().unwrap();
    let main = dir.path().join("main.telsb");
    fs::write(&main, "(import dbl)\n(import dup)\n(print (+ (call dbl 10) (call dup 11)))\n").unwrap();
    // dbl and dup are byte-identical but live at different paths.
    fs::write(dir.path().join("dbl.telsb"), "(* (arg 1) 2)\n").unwrap();
    fs::write(dir.path().join("dup.telsb"), "(* (arg 1) 2)\n").unwrap();

    let (compiler, out) = recording_compiler();
    compiler.run(main.to_str().unwrap(), false).await.unwrap();
    assert_eq!(last_output(&out), "42");
    assert_eq!(
        compiler.cached_parse_count(),
        2,
        "identical bytes at two paths must share one parse entry (main + shared body)"
    );
    assert_eq!(
        compiler.cached_mono_count(),
        3,
        "mono results embed path-based FQs, so the two instances must not share"
    );
}

/// Deterministic errors are terminal answers (docs/keys-and-invalidation.md
/// invariant 6): a file that fails to parse is not re-parsed on the next run —
/// the cached error is served from the content store.
#[tokio::test]
async fn parse_errors_are_cached_answers() {
    let dir = TempDir::new().unwrap();
    let main = dir.path().join("main.telsb");
    let path = main.to_str().unwrap();
    fs::write(&main, "(print 42").unwrap(); // missing closing paren

    let (compiler, _out) = recording_compiler();

    assert!(compiler.run(path, false).await.is_err());
    assert!(compiler.run(path, false).await.is_err(), "the same error must be reported again");
    assert_eq!(
        compiler.computed_parse_count(),
        1,
        "the second run must hit the cached error, not re-parse"
    );
}

/// Same for the type check + mono phase: a deterministic `TypeError` is
/// stored under the instance's content key like a success, so re-running
/// unchanged bad content reports the same error without re-checking.
#[tokio::test]
async fn type_errors_are_cached_answers() {
    let dir = TempDir::new().unwrap();
    let main = dir.path().join("main.telsb");
    let path = main.to_str().unwrap();
    fs::write(&main, "(print (+ 1i32 2i64))\n").unwrap(); // type mismatch

    let (compiler, _out) = recording_compiler();

    assert!(compiler.run(path, false).await.is_err());
    let after_first = compiler.computed_mono_count();
    assert!(compiler.run(path, false).await.is_err(), "the same error must be reported again");
    assert_eq!(
        compiler.computed_mono_count(),
        after_first,
        "the second run must hit the cached type error, not re-check"
    );
}

/// Recompiling a byte-identical project computes nothing new in the cached
/// phases: parse and mono are pure content-store hits.
#[tokio::test]
async fn unchanged_recompile_recomputes_no_cached_phase() {
    let dir = TempDir::new().unwrap();
    let main = dir.path().join("main.telsb");
    let dbl = dir.path().join("dbl.telsb");
    let path = main.to_str().unwrap();
    fs::write(&main, "(import dbl)\n(print (call dbl 21))\n").unwrap();
    fs::write(&dbl, "(* (arg 1) 2)\n").unwrap();

    let (compiler, out) = recording_compiler();

    compiler.run(path, false).await.unwrap();
    assert_eq!(last_output(&out), "42");
    let (parses, monos) = (compiler.computed_parse_count(), compiler.computed_mono_count());

    compiler.run(path, false).await.unwrap();
    assert_eq!(last_output(&out), "42");
    assert_eq!(compiler.computed_parse_count(), parses, "unchanged files must not re-parse");
    assert_eq!(compiler.computed_mono_count(), monos, "unchanged instances must not re-check");
}

/// Re-running the same unchanged file reuses the cached parse rather than
/// creating a second entry.
#[tokio::test]
async fn unchanged_file_reuses_cache_across_runs() {
    let dir = TempDir::new().unwrap();
    let main = dir.path().join("main.telsb");
    let path = main.to_str().unwrap();
    fs::write(&main, "(print 7)\n").unwrap();

    let (compiler, out) = recording_compiler();

    compiler.run(path, false).await.unwrap();
    assert_eq!(last_output(&out), "7");
    let after_first = compiler.cached_parse_count();
    assert_eq!(after_first, 1);

    // Second run of the identical file: no new parse entry.
    compiler.run(path, false).await.unwrap();
    assert_eq!(last_output(&out), "7");
    assert_eq!(
        compiler.cached_parse_count(),
        after_first,
        "an unchanged file must not add a cache entry on re-run"
    );
}
