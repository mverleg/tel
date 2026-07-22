//! Phase B admission control (plans/concurrency-and-eviction.md Decision 3):
//! a cache byte budget gates each wave's *start*. An over-budget store compacts
//! before the run begins — between waves, never mid-wave — which is exactly the
//! quiescent `&mut self` window Phase A's `compact` needs. The serialized
//! `&mut self` runs are themselves the queue: clients enqueue-and-wait, never
//! drop. Eviction is warmth-only, so a budget only ever trades cache reuse for
//! bounded memory — never a wrong answer.

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

fn computed(c: &Compiler) -> (usize, usize, usize) {
    (c.computed_parse_count(), c.computed_resolve_count(), c.computed_mono_count())
}

/// A distinct little program per index, so its parse/resolve/mono keys never
/// alias another index's — each `run` adds a full, non-shared entry set.
fn program(i: u32) -> String {
    format!("(print (+ {} {}))\n", i % 9, (i + 1) % 9)
}

/// Control: with no budget (the default) a second identical run reuses the
/// whole cache — it recomputes nothing.
#[tokio::test]
async fn unbudgeted_second_run_reuses_cache() {
    let dir = TempDir::new().unwrap();
    let main = dir.path().join("main.telsb");
    let path = main.to_str().unwrap();
    fs::write(&main, program(1)).unwrap();

    let (mut c, out) = recording_compiler();
    c.run(path, false).await.unwrap();
    let first = computed(&c);
    assert!(first.0 > 0, "the first run computes something");

    c.run(path, false).await.unwrap();
    assert_eq!(computed(&c), first, "no budget: the second run is a pure cache hit");
    assert!(!last_output(&out).is_empty());
}

/// A budget too small to hold anything forces the gate to evict the whole cache
/// between waves, so the next run recomputes — and (no disk tier) that recompute
/// is real work, visible in every computed counter. The answer is unchanged:
/// eviction is warmth-only.
#[tokio::test]
async fn tight_budget_evicts_between_waves_forcing_recompute() {
    let dir = TempDir::new().unwrap();
    let main = dir.path().join("main.telsb");
    let path = main.to_str().unwrap();
    fs::write(&main, program(1)).unwrap();

    let (mut c, out) = recording_compiler();
    c.set_cache_budget(1); // 1 byte: nothing fits, so the gate clears the store
    c.run(path, false).await.unwrap();
    let answer = last_output(&out);
    assert!(c.cache_bytes() > 1, "the first wave fills the cache past the budget");
    let (p, r, m) = computed(&c);

    // Second wave: the gate compacts to 1 byte first (evicting all), then the
    // run recomputes from scratch.
    c.run(path, false).await.unwrap();
    assert_eq!(last_output(&out), answer, "eviction is warmth-only: same answer");
    let (p2, r2, m2) = computed(&c);
    assert!(p2 > p, "parse recomputed after eviction ({p} -> {p2})");
    assert!(r2 > r, "resolve recomputed after eviction ({r} -> {r2})");
    assert!(m2 > m, "mono recomputed after eviction ({m} -> {m2})");
}

/// The budget bounds the resident set: running many *distinct* programs under a
/// budget keeps the cache near the budget (each wave trims back to it before
/// growing), whereas the unbudgeted cache accumulates all of them.
#[tokio::test]
async fn budget_bounds_resident_set_across_distinct_runs() {
    let dir = TempDir::new().unwrap();
    let n: u32 = 8;
    let paths: Vec<_> = (0..n)
        .map(|i| {
            let p = dir.path().join(format!("m{i}.telsb"));
            fs::write(&p, program(i)).unwrap();
            p
        })
        .collect();

    // One program's footprint, measured on a throwaway compiler.
    let (mut probe, _) = recording_compiler();
    probe.run(paths[0].to_str().unwrap(), false).await.unwrap();
    let per = probe.cache_bytes();
    assert!(per > 0);

    // Unbudgeted: the cache accumulates every distinct program.
    let (mut unbudgeted, _) = recording_compiler();
    for p in &paths {
        unbudgeted.run(p.to_str().unwrap(), false).await.unwrap();
    }

    // Budgeted at ~3 programs: each wave trims back to budget before adding its
    // own set, so the resident peak stays near budget + one wave's growth.
    let (mut budgeted, _) = recording_compiler();
    budgeted.set_cache_budget(per * 3);
    for p in &paths {
        budgeted.run(p.to_str().unwrap(), false).await.unwrap();
    }

    assert!(
        budgeted.cache_bytes() < unbudgeted.cache_bytes(),
        "budgeted ({}) must stay below unbudgeted ({})",
        budgeted.cache_bytes(),
        unbudgeted.cache_bytes(),
    );
    assert!(
        budgeted.cache_bytes() <= per * 5,
        "budgeted resident set ({}) stays near the budget, not unbounded",
        budgeted.cache_bytes(),
    );
    assert!(
        unbudgeted.cache_bytes() >= per * (n as u64 - 1),
        "unbudgeted cache accumulates all {n} distinct programs",
    );
}

/// Clearing the budget restores unbounded growth: after `clear_cache_budget`,
/// a subsequent wave does not evict.
#[tokio::test]
async fn clearing_budget_stops_eviction() {
    let dir = TempDir::new().unwrap();
    let main = dir.path().join("main.telsb");
    let path = main.to_str().unwrap();
    fs::write(&main, program(1)).unwrap();

    let (mut c, _) = recording_compiler();
    c.set_cache_budget(1);
    c.run(path, false).await.unwrap();
    c.clear_cache_budget();

    // With the budget cleared, this run's gate is a no-op; the previous run's
    // entries survive, so it is a pure cache hit.
    let before = computed(&c);
    c.run(path, false).await.unwrap();
    assert_eq!(computed(&c), before, "no budget: nothing evicted, nothing recomputed");
}
