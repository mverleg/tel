//! Span-sidecar / "fast mode" tests (plans/fast-mode.md).
//!
//! The core AST is span-free; byte spans live in an on-demand **span sidecar**
//! keyed on the source digest. The happy path never builds one, and a fired
//! `panic` upgrades its coarse file-path location to a span-accurate
//! `path:line:col` by demanding the sidecar for exactly that file — the
//! in-session "upgrade on error" story.

use sandbox::{Compiler, NoopPrinter, Printer};
use std::fs;
use std::sync::Arc;
use tempfile::TempDir;

fn compiler() -> Compiler {
    let printer: Arc<dyn Printer> = Arc::new(NoopPrinter);
    Compiler::new(printer)
}

/// A successful compile demands no span sidecar: the metadata is loaded only
/// when needed, so the type-check/exec happy path pays nothing for it.
#[tokio::test]
async fn happy_path_builds_no_span_sidecar() {
    let dir = TempDir::new().unwrap();
    let main = dir.path().join("main.telsb");
    fs::write(&main, "(print 42)\n").unwrap();

    let mut compiler = compiler();
    compiler.run(main.to_str().unwrap(), false).await.unwrap();

    assert_eq!(compiler.cached_spans_count(), 0, "no sidecar on the happy path");
}

/// A top-level `panic` reports `path:line:col`, and the sidecar is built
/// exactly once — on the error path.
#[tokio::test]
async fn top_level_panic_reports_line_and_column() {
    let dir = TempDir::new().unwrap();
    let main = dir.path().join("main.telsb");
    let path = main.to_str().unwrap();
    // `(panic)` is on line 2, column 1.
    fs::write(&main, "(print 1)\n(panic)\n").unwrap();

    let mut compiler = compiler();
    let err = compiler.run(path, false).await.unwrap_err().to_string();

    assert!(err.contains("panic at"), "got: {err}");
    assert!(err.contains(&format!("{}:2:1", path)), "expected {path}:2:1, got: {err}");
    assert_eq!(compiler.cached_spans_count(), 1, "sidecar built once, on the panic");
}

/// Column is real, not just line: a `panic` indented on its line reports the
/// column of its opening paren.
#[tokio::test]
async fn panic_column_tracks_indentation() {
    let dir = TempDir::new().unwrap();
    let main = dir.path().join("main.telsb");
    let path = main.to_str().unwrap();
    // `(panic)` starts at column 4 of line 2.
    fs::write(&main, "(print 1)\n   (panic)\n").unwrap();

    let mut compiler = compiler();
    let err = compiler.run(path, false).await.unwrap_err().to_string();

    assert!(err.contains(&format!("{}:2:4", path)), "expected {path}:2:4, got: {err}");
}

/// A `panic` inside a called function resolves through its own frame: the
/// locator is numbered relative to that function, and the sidecar still maps
/// it to the correct absolute span.
#[tokio::test]
async fn panic_inside_called_function_reports_its_location() {
    let dir = TempDir::new().unwrap();
    let main = dir.path().join("main.telsb");
    let path = main.to_str().unwrap();
    // `(panic)` opens at column 16 of line 1 (inside `boom`'s body).
    fs::write(&main, "(function boom (panic))\n(call boom)\n").unwrap();

    let mut compiler = compiler();
    let err = compiler.run(path, false).await.unwrap_err().to_string();

    assert!(err.contains(&format!("{}:1:16", path)), "expected {path}:1:16, got: {err}");
}

/// A whitespace-only edit shifts every byte offset but changes no node
/// structure, so the core AST is fingerprint-identical: re-running still
/// pays no sidecar, and the caches above parse are untouched (early cutoff is
/// preserved under the span split — plans/fast-mode.md 2b).
#[tokio::test]
async fn whitespace_edit_keeps_core_stable_and_sidecar_absent() {
    let dir = TempDir::new().unwrap();
    let main = dir.path().join("main.telsb");
    let path = main.to_str().unwrap();

    let mut compiler = compiler();

    fs::write(&main, "(print 42)\n").unwrap();
    compiler.run(path, false).await.unwrap();
    let mono_after_first = compiler.cached_mono_count();

    // Reformat: same tokens, different offsets.
    fs::write(&main, "(print   42)\n\n").unwrap();
    compiler.run(path, false).await.unwrap();

    assert_eq!(compiler.cached_spans_count(), 0, "still no sidecar on a clean recompile");
    assert_eq!(
        compiler.cached_mono_count(), mono_after_first,
        "whitespace edit must not add mono instances (early cutoff holds)"
    );
}
