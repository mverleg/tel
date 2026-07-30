//! Leaf-driven (push) invalidation tests (plans/roadmap.md item 10,
//! doc/book/src/19a-compiler-internals/07-execution-and-recovery.md,
//! doc/book/src/19a-compiler-internals/04-invalidation.md "Push: from the leafs").
//!
//! The protocol under test: `Compiler::invalidate(path)` is pass 1 — an
//! infallible reverse-edge marking walk; the next `run_watch` is pass 2 —
//! dirty nodes re-derive through the normal pull (clearing dirty only on
//! successful commit), while *clean* subgraphs are served from their
//! memoized bindings with zero recursion: their source files are not even
//! re-read. `run` (batch stance) stays available and never needs invalidate.

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

/// The core payoff of push over pull: after invalidating one changed leaf,
/// the watch run touches exactly the affected cone. The clean sibling is not
/// recomputed at any phase — and, unlike the batch stance, its source file is
/// not even re-read: the only leaf read of the whole wave is the changed
/// file itself (the dirty importer trusts its *clean* parse binding too).
#[tokio::test]
async fn invalidate_recomputes_only_the_affected_cone() {
    let dir = TempDir::new().unwrap();
    let main = dir.path().join("main.telsb");
    let hot = dir.path().join("hot.telsb");
    let cold = dir.path().join("cold.telsb");
    let path = main.to_str().unwrap();
    fs::write(&main, "(import /hot)\n(import /cold)\n(print (+ (call hot 1) (call cold 1)))\n").unwrap();
    fs::write(&hot, "(+ (arg 1) 10)\n").unwrap();
    fs::write(&cold, "(+ (arg 1) 100)\n").unwrap();

    let (mut compiler, out) = recording_compiler();
    compiler.run_watch(path, false).await.unwrap();
    assert_eq!(last_output(&out), "112");

    // Semantic edit to one import, announced like a watcher would.
    fs::write(&hot, "(+ (arg 1) 20)\n").unwrap();
    compiler.invalidate(hot.to_str().unwrap());

    let before = counts(&compiler);
    compiler.run_watch(path, false).await.unwrap();
    assert_eq!(last_output(&out), "122");

    let (reads, parses, resolves, monos) = delta(before, counts(&compiler));
    assert_eq!(reads, 1, "only the changed file may be read; clean subtrees skip even the digest probe");
    assert_eq!(parses, 1, "only the changed file re-parses");
    assert_eq!(resolves, 2, "the changed file and its importer re-resolve; the sibling must not");
    assert_eq!(monos, 1, "only the changed function re-checks; the sibling and main hit their keys");
}

/// The worked example of doc/book/src/19a-compiler-internals/04-invalidation.md: a formatting-only
/// edit dirties the whole cone (marking is conservative and does no hashing),
/// but pass 2 un-dirties it via early cutoff — one parse recompute, and the
/// unchanged fingerprint stops propagation before any resolve or mono work.
/// The follow-up watch run proves the cone was really cleaned (not left
/// dirty): it does no work at all, not even a read.
#[tokio::test]
async fn formatting_only_change_undirties_via_cutoff() {
    let dir = TempDir::new().unwrap();
    let main = dir.path().join("main.telsb");
    let dep = dir.path().join("dep.telsb");
    let path = main.to_str().unwrap();
    fs::write(&main, "(import /dep)\n(print (call dep 2))\n").unwrap();
    fs::write(&dep, "(* (arg 1) 21)\n").unwrap();

    let (mut compiler, out) = recording_compiler();
    compiler.run_watch(path, false).await.unwrap();
    assert_eq!(last_output(&out), "42");

    fs::write(&dep, "# a comment changes bytes, not meaning\n(* (arg 1) 21)\n").unwrap();
    compiler.invalidate(dep.to_str().unwrap());

    let before = counts(&compiler);
    compiler.run_watch(path, false).await.unwrap();
    assert_eq!(last_output(&out), "42");
    assert_eq!(
        delta(before, counts(&compiler)),
        (1, 1, 0, 0),
        "one read + one parse; the unchanged parse fingerprint must cut off everything above"
    );

    // The cutoff must have *cleaned* the cone, not just skipped it: a wave
    // with no dirty nodes is pure memo hits — zero reads, zero recompute.
    let before = counts(&compiler);
    compiler.run_watch(path, false).await.unwrap();
    assert_eq!(last_output(&out), "42");
    assert_eq!(
        delta(before, counts(&compiler)),
        (0, 0, 0, 0),
        "a fully clean wave must cost nothing — dirty bits were cleared on verify"
    );
}

/// Invalidation is a hint, and spurious hints must be harmless: a path the
/// compiler has never seen, and a seen path whose bytes did not actually
/// change. The unchanged file costs one read (its digest is re-derived
/// because the leaf was marked — ground truth over events) and nothing else.
#[tokio::test]
async fn spurious_invalidations_are_harmless() {
    let dir = TempDir::new().unwrap();
    let main = dir.path().join("main.telsb");
    let dep = dir.path().join("dep.telsb");
    let path = main.to_str().unwrap();
    fs::write(&main, "(import /dep)\n(print (call dep 5))\n").unwrap();
    fs::write(&dep, "(+ (arg 1) 2)\n").unwrap();

    let (mut compiler, out) = recording_compiler();
    compiler.run_watch(path, false).await.unwrap();
    assert_eq!(last_output(&out), "7");

    // A path that was never part of any compile.
    compiler.invalidate(dir.path().join("nonexistent.telsb").to_str().unwrap());
    // A real leaf, but its content is unchanged.
    compiler.invalidate(dep.to_str().unwrap());

    let before = counts(&compiler);
    compiler.run_watch(path, false).await.unwrap();
    assert_eq!(last_output(&out), "7");
    assert_eq!(
        delta(before, counts(&compiler)),
        (1, 0, 0, 0),
        "an unchanged-but-marked leaf costs exactly one verification read"
    );
}

/// Panic safety (doc/book/src/19a-compiler-internals/09-invariants.md, "Panics specifically"): a panic at the
/// recompute boundary must leave the node dirty — never clean — and must not
/// poison either cache layer. Observable consequence: after the panicking
/// wave fails, a plain retry (with *no* new invalidate call) still recomputes
/// the full affected chain and picks up the edit. A falsely-clean node would
/// instead serve the stale pre-edit answer from its memo.
#[tokio::test]
async fn panicking_recompute_leaves_node_dirty_and_caches_unpoisoned() {
    let dir = TempDir::new().unwrap();
    let main = dir.path().join("main.telsb");
    let dep = dir.path().join("dep.telsb");
    let path = main.to_str().unwrap();
    fs::write(&main, "(import /dep)\n(print (call dep 1))\n").unwrap();
    fs::write(&dep, "(+ (arg 1) 1)\n").unwrap();

    let (mut compiler, out) = recording_compiler();
    compiler.run_watch(path, false).await.unwrap();
    assert_eq!(last_output(&out), "2");
    let stored_resolves = compiler.cached_resolve_count();

    fs::write(&dep, "(+ (arg 1) 7)\n").unwrap();
    compiler.invalidate(dep.to_str().unwrap());
    compiler.inject_panic_on_resolve(Some("dep.telsb"));

    let err = compiler.run_watch(path, false).await
        .expect_err("a panicking recompute must fail the wave, not fabricate an answer");
    assert!(
        err.to_string().contains("panic"),
        "the failure must surface as the caught panic, got: {}",
        err
    );
    assert_eq!(
        compiler.cached_resolve_count(),
        stored_resolves,
        "a panic is not an answer: nothing may have entered the content store"
    );

    // Retry without any new invalidate: only still-dirty bindings force this
    // recompute, so succeeding with the *new* output proves the panicking
    // node stayed dirty and nothing above it was falsely marked clean.
    compiler.inject_panic_on_resolve(None);
    compiler.run_watch(path, false).await.unwrap();
    assert_eq!(last_output(&out), "8", "the edit must take effect on retry — a stale 2 means a falsely-clean node");

    // And the caches came out healthy: an idle follow-up wave costs nothing.
    let before = counts(&compiler);
    compiler.run_watch(path, false).await.unwrap();
    assert_eq!(last_output(&out), "8");
    assert_eq!(delta(before, counts(&compiler)), (0, 0, 0, 0));
}

/// Import restructuring under push: edges are replaced with sets re-derived
/// from current content on every recompute, so marking after a restructure
/// neither misses new dependents nor resurrects zombie ones — and reverting
/// the restructure is pure cache (every key was already seen).
#[tokio::test]
async fn import_restructure_under_push_invalidation() {
    let dir = TempDir::new().unwrap();
    let main = dir.path().join("main.telsb");
    let a = dir.path().join("a.telsb");
    let b = dir.path().join("b.telsb");
    let path = main.to_str().unwrap();

    let v1_main = "(import /a)\n(print (call a 1))\n";
    let v1_a = "(import /b)\n(+ (call b (arg 1)) 1)\n";
    let v1_b = "(* (arg 1) 10)\n";
    fs::write(&main, v1_main).unwrap();
    fs::write(&a, v1_a).unwrap();
    fs::write(&b, v1_b).unwrap();

    let (mut compiler, out) = recording_compiler();
    compiler.run_watch(path, false).await.unwrap();
    assert_eq!(last_output(&out), "11");

    // Invert the relationship: main -> b -> a. All three files change.
    fs::write(&main, "(import /b)\n(print (call b 1))\n").unwrap();
    fs::write(&b, "(import /a)\n(+ (call a (arg 1)) 1)\n").unwrap();
    fs::write(&a, "(* (arg 1) 10)\n").unwrap();
    compiler.invalidate(main.to_str().unwrap());
    compiler.invalidate(a.to_str().unwrap());
    compiler.invalidate(b.to_str().unwrap());
    compiler.run_watch(path, false).await
        .expect("an inverted (acyclic) import graph must compile under push invalidation");
    assert_eq!(last_output(&out), "11");

    // Revert the restructure: every content key was seen before, so the wave
    // verifies (three reads) but recomputes nothing.
    fs::write(&main, v1_main).unwrap();
    fs::write(&a, v1_a).unwrap();
    fs::write(&b, v1_b).unwrap();
    compiler.invalidate(main.to_str().unwrap());
    compiler.invalidate(a.to_str().unwrap());
    compiler.invalidate(b.to_str().unwrap());

    let before = counts(&compiler);
    compiler.run_watch(path, false).await.unwrap();
    assert_eq!(last_output(&out), "11");
    let (reads, parses, resolves, monos) = delta(before, counts(&compiler));
    assert_eq!(reads, 3, "all three marked leafs re-verify their digests");
    assert_eq!((parses, resolves, monos), (0, 0, 0), "a reverted restructure is pure cache");
}

/// The definer-edge precision fix: an instance of a *local* function (not the
/// file-function) must sit in its defining file's marking cone. If the mono
/// step's graph edge pointed at a phantom `Resolve(function FQ)` node instead
/// of the real file-level resolve step, invalidating the file would under-mark
/// and watch mode would serve the stale instance.
#[tokio::test]
async fn local_function_instances_are_in_their_files_marking_cone() {
    let dir = TempDir::new().unwrap();
    let main = dir.path().join("main.telsb");
    let dep = dir.path().join("dep.telsb");
    let path = main.to_str().unwrap();
    fs::write(&main, "(import /dep)\n(print (call dep 3))\n").unwrap();
    // `helper` is a local function: its FQ names dep.telsb but not the
    // file-level resolve unit.
    fs::write(&dep, "(function helper (* (arg 1) 2))\n(call helper (arg 1))\n").unwrap();

    let (mut compiler, out) = recording_compiler();
    compiler.run_watch(path, false).await.unwrap();
    assert_eq!(last_output(&out), "6");

    // Change only helper's body; announce; re-run in watch mode.
    fs::write(&dep, "(function helper (* (arg 1) 5))\n(call helper (arg 1))\n").unwrap();
    compiler.invalidate(dep.to_str().unwrap());
    compiler.run_watch(path, false).await.unwrap();
    assert_eq!(
        last_output(&out),
        "15",
        "the local function's instance must have been marked dirty and recomputed"
    );
}

/// Scenario A (doc/book/src/19a-compiler-internals/01-overview.md, "Revert and branch-switch") under the watch stance:
/// edit, then revert to the original bytes. The revert's wave re-reads the
/// announced leaf (ground truth over events) but its digest leads straight to
/// answers the content store already holds — zero recompute at every phase,
/// and the wave un-dirties the cone back to a free steady state.
#[tokio::test]
async fn watch_revert_recomputes_nothing() {
    let dir = TempDir::new().unwrap();
    let main = dir.path().join("main.telsb");
    let dep = dir.path().join("dep.telsb");
    let path = main.to_str().unwrap();
    let original = "(* (arg 1) 21)\n";
    fs::write(&main, "(import /dep)\n(print (call dep 2))\n").unwrap();
    fs::write(&dep, original).unwrap();

    let (mut compiler, out) = recording_compiler();
    compiler.run_watch(path, false).await.unwrap();
    assert_eq!(last_output(&out), "42");

    // Semantic edit, announced and applied.
    fs::write(&dep, "(* (arg 1) 50)\n").unwrap();
    compiler.invalidate(dep.to_str().unwrap());
    compiler.run_watch(path, false).await.unwrap();
    assert_eq!(last_output(&out), "100");

    // Revert to the exact original bytes.
    fs::write(&dep, original).unwrap();
    compiler.invalidate(dep.to_str().unwrap());
    let before = counts(&compiler);
    compiler.run_watch(path, false).await.unwrap();
    assert_eq!(last_output(&out), "42");
    assert_eq!(
        delta(before, counts(&compiler)),
        (1, 0, 0, 0),
        "a revert costs one read: every answer below, beside and above is already stored"
    );

    // The revert wave must have cleaned the cone, not merely served it.
    let before = counts(&compiler);
    compiler.run_watch(path, false).await.unwrap();
    assert_eq!(delta(before, counts(&compiler)), (0, 0, 0, 0));
}

/// Partial failure midway up a leaf→root pass
/// (doc/book/src/19a-compiler-internals/09-invariants.md, "Invariant 8, restated"): while a mid-chain file is broken (deterministic resolve error), a
/// *different* leaf underneath it is edited. When the mid-chain file is later
/// fixed, the whole affected chain must recompute — the leaf edit made during
/// the broken period must be reflected. A falsely-clean node anywhere in the
/// chain would resurrect the pre-edit answer instead.
#[tokio::test]
async fn edit_during_error_period_is_not_lost_when_chain_heals() {
    let dir = TempDir::new().unwrap();
    let main = dir.path().join("main.telsb");
    let mid = dir.path().join("mid.telsb");
    let leaf = dir.path().join("leaf.telsb");
    let path = main.to_str().unwrap();
    let mid_good = "(import /leaf)\n(+ (call leaf (arg 1)) 1)\n";
    fs::write(&main, "(import /mid)\n(print (call mid 10))\n").unwrap();
    fs::write(&mid, mid_good).unwrap();
    fs::write(&leaf, "(* (arg 1) 2)\n").unwrap();

    let (mut compiler, out) = recording_compiler();
    compiler.run_watch(path, false).await.unwrap();
    assert_eq!(last_output(&out), "21");

    // Break the middle of the chain: undefined variable, a deterministic
    // (and cacheable) resolve error.
    fs::write(&mid, "(import /leaf)\n(+ (call leaf (arg 1)) undefined_var)\n").unwrap();
    compiler.invalidate(mid.to_str().unwrap());
    compiler.run_watch(path, false).await
        .expect_err("the broken mid-chain file must fail the wave");

    // While broken, edit the leaf underneath.
    fs::write(&leaf, "(* (arg 1) 3)\n").unwrap();
    compiler.invalidate(leaf.to_str().unwrap());
    compiler.run_watch(path, false).await
        .expect_err("still broken: fixing the leaf cannot heal mid");

    // Heal the chain.
    fs::write(&mid, mid_good).unwrap();
    compiler.invalidate(mid.to_str().unwrap());
    compiler.run_watch(path, false).await.unwrap();
    assert_eq!(
        last_output(&out),
        "31",
        "the leaf edit from the broken period must be in the healed answer (10*3+1); \
         a 21 means a falsely-clean node served the pre-edit chain"
    );

    // Steady state after recovery is free.
    let before = counts(&compiler);
    compiler.run_watch(path, false).await.unwrap();
    assert_eq!(last_output(&out), "31");
    assert_eq!(delta(before, counts(&compiler)), (0, 0, 0, 0));
}
