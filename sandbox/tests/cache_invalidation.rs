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
    let printer: Arc<dyn Printer> = Arc::new(RecordingPrinter { out: out.clone() });
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

    let (mut compiler, out) = recording_compiler();

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

    let (mut compiler, out) = recording_compiler();

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

    let (mut compiler, out) = recording_compiler();
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

    let (mut compiler, _out) = recording_compiler();

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

    let (mut compiler, _out) = recording_compiler();

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

    let (mut compiler, out) = recording_compiler();

    compiler.run(path, false).await.unwrap();
    assert_eq!(last_output(&out), "42");
    let (parses, resolves, monos) = (
        compiler.computed_parse_count(),
        compiler.computed_resolve_count(),
        compiler.computed_mono_count(),
    );

    compiler.run(path, false).await.unwrap();
    assert_eq!(last_output(&out), "42");
    assert_eq!(compiler.computed_parse_count(), parses, "unchanged files must not re-parse");
    assert_eq!(compiler.computed_resolve_count(), resolves, "unchanged files must not re-resolve");
    assert_eq!(compiler.computed_mono_count(), monos, "unchanged instances must not re-check");
}

/// Editing one file recomputes only its own chain: the changed file re-parses
/// and re-resolves, its importer re-resolves (a dep fingerprint in its key
/// changed), but a sibling import is served from cache at every phase — and
/// because the importer's resolve *answer* comes out unchanged, its mono
/// instance is a pure hit too (the fingerprint chain cutting the change off).
#[tokio::test]
async fn changed_import_recomputes_only_its_chain() {
    let dir = TempDir::new().unwrap();
    let main = dir.path().join("main.telsb");
    let path = main.to_str().unwrap();
    fs::write(&main, "(import stable)\n(import edited)\n(print (+ (call stable 1) (call edited 1)))\n").unwrap();
    fs::write(dir.path().join("stable.telsb"), "(+ (arg 1) 10)\n").unwrap();
    fs::write(dir.path().join("edited.telsb"), "(+ (arg 1) 100)\n").unwrap();

    let (mut compiler, out) = recording_compiler();
    compiler.run(path, false).await.unwrap();
    assert_eq!(last_output(&out), "112");
    let (parses, resolves, monos) = (
        compiler.computed_parse_count(),
        compiler.computed_resolve_count(),
        compiler.computed_mono_count(),
    );

    fs::write(dir.path().join("edited.telsb"), "(+ (arg 1) 200)\n").unwrap();
    compiler.run(path, false).await.unwrap();
    assert_eq!(last_output(&out), "212", "the edited import must not be served stale");

    assert_eq!(compiler.computed_parse_count(), parses + 1, "only the edited file re-parses");
    assert_eq!(
        compiler.computed_resolve_count(),
        resolves + 2,
        "the edited file and its importer re-resolve; the sibling must be a cache hit"
    );
    assert_eq!(
        compiler.computed_mono_count(),
        monos + 1,
        "only the edited instance re-checks: main's resolve answer is unchanged, so its mono key is too"
    );
}

/// Scenario B from docs/cache-invalidation-problem.md, exactly as specified:
/// a whitespace/comment-only edit deep in a leaf dependency re-parses that one
/// file — the input genuinely changed — but its parse *answer* is unchanged,
/// so its fingerprint is unchanged, so every content key above it is unchanged:
/// zero resolve and zero mono recomputation anywhere in the tree.
#[tokio::test]
async fn whitespace_only_edit_reparses_one_file_and_nothing_else() {
    let dir = TempDir::new().unwrap();
    let main = dir.path().join("main.telsb");
    let path = main.to_str().unwrap();
    // Three levels deep so the cutoff is visibly transitive: main -> mid -> leaf.
    fs::write(&main, "(import mid)\n(print (call mid 40))\n").unwrap();
    fs::write(dir.path().join("mid.telsb"), "(import leaf)\n(+ (call leaf (arg 1)) 1)\n").unwrap();
    fs::write(dir.path().join("leaf.telsb"), "(* (arg 1) 2)\n").unwrap();

    let (mut compiler, out) = recording_compiler();
    compiler.run(path, false).await.unwrap();
    assert_eq!(last_output(&out), "81");
    let (parses, resolves, monos) = (
        compiler.computed_parse_count(),
        compiler.computed_resolve_count(),
        compiler.computed_mono_count(),
    );

    // Reformat the deepest leaf: blank line + comment, same code.
    fs::write(dir.path().join("leaf.telsb"), "\n# reformatted, semantically identical\n(* (arg 1) 2)\n").unwrap();
    compiler.run(path, false).await.unwrap();
    assert_eq!(last_output(&out), "81");

    assert_eq!(compiler.computed_parse_count(), parses + 1, "the edited bytes must re-parse — the input did change");
    assert_eq!(compiler.computed_resolve_count(), resolves, "unchanged parse answer: zero resolve recompute (early cutoff)");
    assert_eq!(compiler.computed_mono_count(), monos, "unchanged resolve answers: zero mono recompute (early cutoff)");
}

/// Control for the cutoff: a *semantic* edit to the same leaf must recompute —
/// and stop exactly where its effects stop. The leaf and its importer re-resolve
/// (the leaf's answer fingerprint changed), but the importer's own *answer* is
/// unchanged (it references the leaf by FQ, not by content), so the root's
/// resolve key is untouched and only the leaf's mono instance re-checks.
#[tokio::test]
async fn semantic_edit_recomputes_exactly_the_affected_cone() {
    let dir = TempDir::new().unwrap();
    let main = dir.path().join("main.telsb");
    let path = main.to_str().unwrap();
    fs::write(&main, "(import mid)\n(print (call mid 40))\n").unwrap();
    fs::write(dir.path().join("mid.telsb"), "(import leaf)\n(+ (call leaf (arg 1)) 1)\n").unwrap();
    fs::write(dir.path().join("leaf.telsb"), "(* (arg 1) 2)\n").unwrap();

    let (mut compiler, out) = recording_compiler();
    compiler.run(path, false).await.unwrap();
    assert_eq!(last_output(&out), "81");
    let (parses, resolves, monos) = (
        compiler.computed_parse_count(),
        compiler.computed_resolve_count(),
        compiler.computed_mono_count(),
    );

    fs::write(dir.path().join("leaf.telsb"), "(* (arg 1) 3)\n").unwrap();
    compiler.run(path, false).await.unwrap();
    assert_eq!(last_output(&out), "121", "the semantic edit must take effect — cutoff must not over-fire");

    assert_eq!(compiler.computed_parse_count(), parses + 1, "only the edited file re-parses");
    assert_eq!(
        compiler.computed_resolve_count(),
        resolves + 2,
        "leaf and mid re-resolve; mid's answer is unchanged so main must be a hit"
    );
    assert_eq!(
        compiler.computed_mono_count(),
        monos + 1,
        "only the leaf's instance re-checks; mid's and main's resolved functions are unchanged"
    );
}

/// Function-level cutoff within one file: editing one function re-checks only
/// that function's mono instance — sibling functions in the same (re-parsed,
/// re-resolved) file chain to their own resolved data, which is unchanged.
#[tokio::test]
async fn editing_one_function_leaves_sibling_functions_cached() {
    let dir = TempDir::new().unwrap();
    let main = dir.path().join("main.telsb");
    let path = main.to_str().unwrap();
    fs::write(&main, "(import lib)\n(print (call lib 7))\n").unwrap();
    fs::write(
        dir.path().join("lib.telsb"),
        "(function double (* (arg 1) 2))\n(function triple (* (arg 1) 3))\n(+ (call double (arg 1)) (call triple (arg 1)))\n",
    ).unwrap();

    let (mut compiler, out) = recording_compiler();
    compiler.run(path, false).await.unwrap();
    assert_eq!(last_output(&out), "35");
    let monos = compiler.computed_mono_count();

    // Change double only; triple and lib's own body keep their resolved data.
    fs::write(
        dir.path().join("lib.telsb"),
        "(function double (* (arg 1) 4))\n(function triple (* (arg 1) 3))\n(+ (call double (arg 1)) (call triple (arg 1)))\n",
    ).unwrap();
    compiler.run(path, false).await.unwrap();
    assert_eq!(last_output(&out), "49");

    assert_eq!(
        compiler.computed_mono_count(),
        monos + 1,
        "only the edited function's instance re-checks; its siblings in the same file stay cached"
    );
}

/// A dependent above a *stably erroring* import is itself a cached answer: the
/// erroring dep still has an answer fingerprint (errors are answers), so the
/// importer's content key is derivable and its propagated failure is stored.
/// The second compile serves both from the store.
#[tokio::test]
async fn importer_of_stably_erroring_import_is_cached() {
    let dir = TempDir::new().unwrap();
    let main = dir.path().join("main.telsb");
    let path = main.to_str().unwrap();
    fs::write(&main, "(import broken)\n(print (call broken))\n").unwrap();
    fs::write(dir.path().join("broken.telsb"), "(print undefined_variable)\n").unwrap();

    let (mut compiler, _out) = recording_compiler();

    let first = compiler.run(path, false).await.expect_err("the broken import must fail the compile").to_string();
    assert_eq!(
        compiler.cached_resolve_count(),
        2,
        "both the failing dep's answer and the importer's propagated failure must be stored"
    );
    let resolves = compiler.computed_resolve_count();

    let second = compiler.run(path, false).await.expect_err("the same failure must be reported again").to_string();
    assert_eq!(second, first, "the cached failure must be byte-identical");
    assert_eq!(
        compiler.computed_resolve_count(),
        resolves,
        "the second compile must be pure cache hits — dependent included"
    );
}

/// Byte-identical files share one parse answer across paths, so that answer
/// must embed nothing path-derived — `panic` diagnostics get their location
/// attached at resolve (whose key pins the FQ), and each file reports its OWN
/// path even though the parse entry is shared.
#[tokio::test]
async fn identical_panic_files_report_their_own_path() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("pana.telsb"), "(panic)\n").unwrap();
    fs::write(dir.path().join("panb.telsb"), "(panic)\n").unwrap();
    let main_a = dir.path().join("main_a.telsb");
    let main_b = dir.path().join("main_b.telsb");
    fs::write(&main_a, "(import pana)\n(call pana)\n").unwrap();
    fs::write(&main_b, "(import panb)\n(call panb)\n").unwrap();

    let (mut compiler, _out) = recording_compiler();

    let err_a = compiler.run(main_a.to_str().unwrap(), false).await.expect_err("pana panics").to_string();
    assert!(err_a.contains("pana.telsb"), "panic must point at pana's path, got: {err_a}");

    let err_b = compiler.run(main_b.to_str().unwrap(), false).await.expect_err("panb panics").to_string();
    assert!(err_b.contains("panb.telsb"), "panic must point at panb's own path even though its parse answer is shared, got: {err_b}");

    assert_eq!(
        compiler.cached_parse_count(),
        3,
        "the two identical panic files must still share one parse entry (2 mains + 1 shared body)"
    );
}

/// Deterministic resolve errors are terminal answers like parse/type errors:
/// re-demanding the same broken content serves the cached error without
/// running the resolver again.
#[tokio::test]
async fn resolve_errors_are_cached_answers() {
    let dir = TempDir::new().unwrap();
    let main = dir.path().join("main.telsb");
    let path = main.to_str().unwrap();
    fs::write(&main, "(print undefined_variable)\n").unwrap();

    let (mut compiler, _out) = recording_compiler();

    assert!(compiler.run(path, false).await.is_err());
    let after_first = compiler.computed_resolve_count();
    assert!(compiler.run(path, false).await.is_err(), "the same error must be reported again");
    assert_eq!(
        compiler.computed_resolve_count(),
        after_first,
        "the second run must hit the cached resolve error, not re-resolve"
    );
}

/// A resolve answer is shared wherever its key is re-derived, no matter which
/// entry point demanded it: a second program importing the same unchanged
/// library re-derives the same key and is served from the store.
#[tokio::test]
async fn shared_import_is_served_from_cache_across_entry_points() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("lib.telsb"), "(* (arg 1) 2)\n").unwrap();
    let entry1 = dir.path().join("one.telsb");
    let entry2 = dir.path().join("two.telsb");
    fs::write(&entry1, "(import lib)\n(print (call lib 21))\n").unwrap();
    fs::write(&entry2, "(import lib)\n(print (call lib 50))\n").unwrap();

    let (mut compiler, out) = recording_compiler();
    compiler.run(entry1.to_str().unwrap(), false).await.unwrap();
    assert_eq!(last_output(&out), "42");
    let resolves = compiler.computed_resolve_count();

    compiler.run(entry2.to_str().unwrap(), false).await.unwrap();
    assert_eq!(last_output(&out), "100");
    assert_eq!(
        compiler.computed_resolve_count(),
        resolves + 1,
        "only the new entry point resolves; the shared library must be a cache hit"
    );
}

/// Re-running the same unchanged file reuses the cached parse rather than
/// creating a second entry.
#[tokio::test]
async fn unchanged_file_reuses_cache_across_runs() {
    let dir = TempDir::new().unwrap();
    let main = dir.path().join("main.telsb");
    let path = main.to_str().unwrap();
    fs::write(&main, "(print 7)\n").unwrap();

    let (mut compiler, out) = recording_compiler();

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
