#![cfg(feature = "step-trace")]
//! Eviction tracing (src/trace.rs + ContentStore::compact): with `step-trace`
//! on, a compaction pass appends an `evict` line per dropped entry (kind, key
//! hash, size, age) plus a `compaction` summary, in the same JSONL stream as
//! the per-step hit/miss lines — so a trace can tell a cold miss apart from a
//! re-miss on an evicted entry.
//!
//! Its own test binary (separate process) because the trace path is a
//! process-global env var — see `tests/trace.rs`.

use sandbox::{Compiler, NoopPrinter};
use serde_json::Value;
use std::collections::HashSet;
use std::fs;
use std::sync::Arc;
use tempfile::TempDir;

#[tokio::test]
async fn trace_records_evictions_from_compaction() {
    let dir = TempDir::new().unwrap();
    let trace_path = dir.path().join("trace.jsonl");
    std::env::set_var("TEL_SANDBOX_TRACE_FILE", trace_path.to_str().unwrap());

    let main = dir.path().join("main.telsb");
    fs::write(&main, "(print (+ 1 2))\n").unwrap();
    let path = main.to_str().unwrap();

    let mut compiler = Compiler::new(Arc::new(NoopPrinter));
    compiler.set_cache_budget(1); // 1 byte: nothing fits
    compiler.run(path, false).await.unwrap(); // fills the cache (gate is a no-op: empty)
    compiler.run(path, false).await.unwrap(); // gate compacts first -> evicts everything
    drop(compiler);

    let lines: Vec<Value> = fs::read_to_string(&trace_path)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).expect("every trace line is valid JSON"))
        .collect();

    // Compaction emitted an `evict` line per dropped entry.
    let evicts: Vec<&Value> = lines.iter().filter(|l| l["event"] == "evict").collect();
    assert!(!evicts.is_empty(), "compaction under a tight budget must trace evictions");
    for e in &evicts {
        let kind = e["kind"].as_str().unwrap();
        assert!(
            ["parse", "resolve", "mono", "spans"].contains(&kind),
            "unexpected evict kind: {e}"
        );
        assert_eq!(e["key"].as_str().unwrap().len(), 32, "evict logs the 128-bit key hash");
        assert!(e["size"].is_u64(), "evict logs the entry size");
    }

    // Every evicted key is one an earlier step actually stored — no phantom keys.
    let step_keys: HashSet<&str> = lines
        .iter()
        .filter(|l| l["event"] == "step")
        .filter_map(|l| l["key"].as_str())
        .collect();
    for e in &evicts {
        assert!(
            step_keys.contains(e["key"].as_str().unwrap()),
            "evicted a key that no step ever stored: {e}"
        );
    }

    // A compaction summary accounts for exactly those evictions.
    let comps: Vec<&Value> = lines.iter().filter(|l| l["event"] == "compaction").collect();
    assert!(!comps.is_empty(), "a compaction summary is emitted");
    assert!(comps.iter().all(|c| c["budget"] == 1));
    let summed: u64 = comps.iter().map(|c| c["evicted"].as_u64().unwrap()).sum();
    assert_eq!(summed as usize, evicts.len(), "summary count matches the evict lines");
    assert!(comps.iter().all(|c| c["bytes_freed"].as_u64().unwrap() >= 1));
}
