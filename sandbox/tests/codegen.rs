//! The Python backend (`src/codegen.rs`): a compiling backend beside the
//! interpreter. These tests check the emitted script is well-formed and, when
//! a `python3` is on PATH, that running it reproduces the interpreter's output.

use std::process::Command;

/// Run the generated script through `python3`, returning its stdout. Returns
/// `None` if no interpreter is available, so the test can skip rather than fail
/// in a minimal CI image.
fn run_python(source: &str) -> Option<String> {
    let dir = tempfile::tempdir().ok()?;
    let path = dir.path().join("prog.py");
    std::fs::write(&path, source).ok()?;
    let output = Command::new("python3").arg(&path).output().ok()?;
    assert!(
        output.status.success(),
        "generated python failed:\n{}\n--- source ---\n{}",
        String::from_utf8_lossy(&output.stderr),
        source
    );
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[tokio::test]
async fn emits_shebang_and_entry() {
    let py = sandbox::codegen_python_file("examples/factorial/main.telsb")
        .await
        .expect("factorial should compile");
    assert!(py.starts_with("#!/usr/bin/env python3\n"), "missing shebang");
    assert!(py.contains("def main():"), "missing entry def");
    // Monomorphisation is visible in the output: one def per (function, type).
    assert!(py.contains("def fact_helper_i64("), "missing monomorphised helper");
    assert!(py.contains("if __name__ == \"__main__\":"), "missing entry guard");
}

#[tokio::test]
async fn generated_python_matches_interpreter_factorial() {
    let py = sandbox::codegen_python_file("examples/factorial/main.telsb")
        .await
        .expect("factorial should compile");
    let Some(out) = run_python(&py) else {
        eprintln!("skipping: python3 not available");
        return;
    };
    // 5! = 120, same as the interpreter prints.
    assert_eq!(out.trim(), "120");
}

#[tokio::test]
async fn generated_python_matches_interpreter_math() {
    let py = sandbox::codegen_python_file("examples/math/main.telsb")
        .await
        .expect("math should compile");
    let Some(out) = run_python(&py) else {
        eprintln!("skipping: python3 not available");
        return;
    };
    // Same lines the interpreter prints for the math example: max, min, abs,
    // abs, max — exercises comparisons, `if`, and negation.
    assert_eq!(out.trim(), "20\n15\n42\n42\n-5");
}
