#![cfg(feature = "step-trace")]
//! Step-trace debug mode (src/trace.rs): with the `step-trace` feature on,
//! every step appends one JSON line — logical id in full, key and result as
//! hashes only, cache hit/miss with entry age, duration in micros — to the
//! file named by `TEL_SANDBOX_TRACE_FILE`.
//!
//! Run with `cargo test --features step-trace`. This is the only test in this
//! binary because the trace file path is configured through a process-global
//! env var.

use sandbox::{Compiler, NoopPrinter};
use serde_json::Value;
use std::fs;
use std::sync::Arc;
use tempfile::TempDir;

#[tokio::test]
async fn trace_records_steps_cache_outcomes_and_ages() {
    let dir = TempDir::new().unwrap();
    let trace_path = dir.path().join("trace.jsonl");
    std::env::set_var("TEL_SANDBOX_TRACE_FILE", trace_path.to_str().unwrap());

    let main = dir.path().join("main.telsb");
    let dep = dir.path().join("dep.telsb");
    fs::write(&main, "(import dep)\n(print (call dep 20))\n").unwrap();
    fs::write(&dep, "(+ (arg 1) 1)\n").unwrap();

    let mut compiler = Compiler::new(Arc::new(NoopPrinter));
    let path = main.to_str().unwrap();
    compiler.run(path, false).await.unwrap();
    compiler.run(path, false).await.unwrap();
    drop(compiler);

    let content = fs::read_to_string(&trace_path).unwrap();
    let lines: Vec<Value> = content
        .lines()
        .map(|line| serde_json::from_str(line).expect("every trace line is valid JSON"))
        .collect();

    assert_eq!(lines[0]["event"], "trace_start");
    assert!(lines[0]["pid"].is_u64());
    let steps: Vec<&Value> = lines.iter().filter(|l| l["event"] == "step").collect();

    // All four step kinds are traced.
    for kind in ["parse", "resolve", "mono", "exec"] {
        assert!(steps.iter().any(|s| s["kind"] == kind), "no {kind} step traced");
    }

    // Every step records its duration in micros and its logical id in full.
    for step in &steps {
        assert!(step["dur_us"].is_u64(), "step without duration: {step}");
        assert!(step["t_us"].is_u64(), "step without start offset: {step}");
        let logical = step["step"].as_str().expect("logical id is a string");
        assert!(!logical.is_empty());
    }

    // First run computes (miss, no age yet); second run is served from the
    // content store (hit), with the entry's age measured from the insert the
    // tracer stored on the miss. Key and fingerprint appear as hex hashes
    // only, identical across runs because the content is identical.
    let parses: Vec<&&Value> = steps
        .iter()
        .filter(|s| s["kind"] == "parse" && s["step"].as_str().unwrap().contains("main.telsb"))
        .collect();
    assert_eq!(parses.len(), 2, "one parse of main.telsb per run");
    assert_eq!(parses[0]["cache"], "miss");
    assert!(parses[0]["age_us"].is_null(), "a miss has no entry age");
    assert_eq!(parses[1]["cache"], "hit");
    assert!(parses[1]["age_us"].as_u64().unwrap() > 0, "hit records the entry's age");
    assert_eq!(parses[0]["key"], parses[1]["key"], "same content, same key");
    assert_eq!(parses[0]["key"].as_str().unwrap().len(), 16, "key is a 64-bit hex hash");
    assert_eq!(parses[0]["fp"], parses[1]["fp"], "same answer, same fingerprint");

    // Resolve and mono behave the same way on the second run.
    for kind in ["resolve", "mono"] {
        assert!(
            steps.iter().any(|s| s["kind"] == kind && s["cache"] == "hit" && s["age_us"].is_u64()),
            "second run must serve {kind} from cache with an age"
        );
    }

    // Exec is deliberately uncached: traced for timing only.
    let execs: Vec<&&Value> = steps.iter().filter(|s| s["kind"] == "exec").collect();
    assert_eq!(execs.len(), 2);
    assert!(execs.iter().all(|s| s["cache"].is_null() && s["key"].is_null()));
}
