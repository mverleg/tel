//! Import resolution: absolute paths, one candidate, no shadowing.
//!
//! An import spells a path that is absolute **against the project root**
//! (plans/external-deps.md, "Import form decision"). That is the whole rule,
//! and these tests pin what it buys:
//!
//! - the same import text names the same file wherever it is written, because
//!   resolution never consults the importing file's directory;
//! - an import has exactly one candidate by construction, so there is no
//!   search path and no precedence rule deciding which of two files wins;
//! - anything that would reintroduce ambiguity — a bare or relative path, a
//!   repeated import, two paths claiming one callable name — is a hard error
//!   rather than something silently resolved.
//!
//! The root is the innermost `tel.toml` above the entry file, or the entry's
//! own directory when there is no marker at all.

use sandbox::{Compiler, NoopPrinter, ROOT_MARKER};
use std::fs;
use std::sync::Arc;
use tempfile::TempDir;

fn compile(path: &std::path::Path) -> impl std::future::Future<Output = Result<(), sandbox::Error>> {
    let path = path.to_str().unwrap().to_string();
    async move { Compiler::new(Arc::new(NoopPrinter)).run(&path, false).await }
}

/// Compile expecting failure, returning the rendered error.
async fn expect_error(path: &std::path::Path) -> String {
    compile(path).await.expect_err("compile should have failed").to_string()
}

/// The point of root-anchoring: a *nested* file imports `/lib/dbl` and gets
/// `<root>/lib/dbl.telsb` — not `<dir of the importer>/lib/dbl.telsb`, which
/// is what a relative rule would have picked and which does not exist here.
#[tokio::test]
async fn absolute_paths_resolve_against_the_root_not_the_importer() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    fs::write(root.join(ROOT_MARKER), "").unwrap();
    fs::create_dir_all(root.join("lib")).unwrap();
    fs::create_dir_all(root.join("app/deep")).unwrap();

    fs::write(root.join("lib/dbl.telsb"), "(* (arg 1) 2)\n").unwrap();
    // The importer sits two directories below the root, and the import text is
    // identical to what a file *at* the root would write.
    let main = root.join("app/deep/main.telsb");
    fs::write(&main, "(import /lib/dbl)\n(print (call dbl 21))\n").unwrap();

    compile(&main).await.expect("a root-anchored import must resolve from a nested file");
}

/// With no `tel.toml` anywhere, the entry's own directory is the root — which
/// is what makes a single-directory program work with no configuration.
#[tokio::test]
async fn without_a_marker_the_entry_directory_is_the_root() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("dbl.telsb"), "(* (arg 1) 2)\n").unwrap();
    let main = dir.path().join("main.telsb");
    fs::write(&main, "(import /dbl)\n(print (call dbl 21))\n").unwrap();

    compile(&main).await.expect("entry directory should serve as the root");
}

/// The innermost marker wins, so a nested workspace resolves against itself
/// rather than an enclosing one — the same rule the daemon discovers roots by.
#[tokio::test]
async fn the_innermost_marker_wins() {
    let dir = TempDir::new().unwrap();
    let outer = dir.path();
    let inner = outer.join("inner");
    fs::create_dir_all(&inner).unwrap();
    fs::write(outer.join(ROOT_MARKER), "").unwrap();
    fs::write(inner.join(ROOT_MARKER), "").unwrap();

    // `/dbl` exists under both roots. The outer one does not compile, so
    // *which* file was found is observable in the result rather than having to
    // be inferred.
    fs::write(outer.join("dbl.telsb"), "(call nonexistent 1)\n").unwrap();
    fs::write(inner.join("dbl.telsb"), "(* (arg 1) 2)\n").unwrap();

    let main = inner.join("main.telsb");
    fs::write(&main, "(import /dbl)\n(print (call dbl 21))\n").unwrap();

    compile(&main).await.expect("the innermost marker must win, so /dbl is the inner file");
}

/// A bare name is no longer an import path. It is rejected rather than treated
/// as a sibling: sibling resolution is exactly the relative form this design
/// removed.
#[tokio::test]
async fn a_bare_name_is_rejected() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("dbl.telsb"), "(* (arg 1) 2)\n").unwrap();
    let main = dir.path().join("main.telsb");
    fs::write(&main, "(import dbl)\n(print (call dbl 21))\n").unwrap();

    let msg = expect_error(&main).await;
    assert!(msg.contains("Invalid import"), "got: {msg}");
    assert!(msg.contains("must start with '/'"), "the error must say what the form is, got: {msg}");
}

/// `.` and `..` are the relative forms in disguise — and `..` could escape the
/// root entirely — so they are refused even though they are spelled absolutely.
#[tokio::test]
async fn dot_segments_are_rejected() {
    let dir = TempDir::new().unwrap();
    let main = dir.path().join("main.telsb");

    for path in ["/../dbl", "/./dbl", "/lib/../dbl"] {
        fs::write(&main, format!("(import {path})\n(print 1)\n")).unwrap();
        let msg = expect_error(&main).await;
        assert!(msg.contains("Invalid import"), "{path} must be rejected, got: {msg}");
        assert!(msg.contains("'.' or '..'"), "got: {msg}");
    }
}

/// The extension is the language's, not the author's: writing it would make
/// two spellings name one file.
#[tokio::test]
async fn a_written_extension_is_rejected() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("dbl.telsb"), "(* (arg 1) 2)\n").unwrap();
    let main = dir.path().join("main.telsb");
    fs::write(&main, "(import /dbl.telsb)\n(print (call dbl 21))\n").unwrap();

    let msg = expect_error(&main).await;
    assert!(msg.contains("extension is implied"), "got: {msg}");
}

/// Importing one file twice is an error, not a silently deduplicated no-op:
/// uniqueness is enforced rather than assumed.
#[tokio::test]
async fn a_repeated_import_is_rejected() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("dbl.telsb"), "(* (arg 1) 2)\n").unwrap();
    let main = dir.path().join("main.telsb");
    fs::write(&main, "(import /dbl)\n(import /dbl)\n(print (call dbl 21))\n").unwrap();

    let msg = expect_error(&main).await;
    assert!(msg.contains("Duplicate import"), "got: {msg}");
    assert!(msg.contains("/dbl"), "the error must name the path, got: {msg}");
}

/// Two different files whose last segment matches would both want the same
/// callable name. Neither wins — picking one is the shadowing rule this design
/// deliberately does not have.
#[tokio::test]
async fn two_paths_with_one_callable_name_are_rejected() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    fs::create_dir_all(root.join("a")).unwrap();
    fs::create_dir_all(root.join("b")).unwrap();
    fs::write(root.join("a/util.telsb"), "(* (arg 1) 2)\n").unwrap();
    fs::write(root.join("b/util.telsb"), "(* (arg 1) 3)\n").unwrap();

    let main = root.join("main.telsb");
    fs::write(&main, "(import /a/util)\n(import /b/util)\n(print (call util 21))\n").unwrap();

    let msg = expect_error(&main).await;
    assert!(msg.contains("both callable as 'util'"), "got: {msg}");
    assert!(msg.contains("/a/util") && msg.contains("/b/util"),
        "the error must name both paths, got: {msg}");
}

/// Same file, two importers in different directories: one import text, one
/// resolved file, one cached parse — the property that makes root-anchoring
/// worth having for the cache and not just for readability.
#[tokio::test]
async fn one_path_from_two_directories_is_one_file() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    fs::write(root.join(ROOT_MARKER), "").unwrap();
    fs::create_dir_all(root.join("lib")).unwrap();
    fs::create_dir_all(root.join("app")).unwrap();

    fs::write(root.join("lib/dbl.telsb"), "(* (arg 1) 2)\n").unwrap();
    fs::write(root.join("app/helper.telsb"), "(import /lib/dbl)\n(call dbl (arg 1))\n").unwrap();
    let main = root.join("main.telsb");
    fs::write(&main, "(import /lib/dbl)\n(import /app/helper)\n(print (+ (call dbl 1) (call helper 2)))\n").unwrap();

    let mut compiler = Compiler::new(Arc::new(NoopPrinter));
    compiler.run(main.to_str().unwrap(), false).await.expect("compile");

    // main, helper, dbl — dbl parsed once despite two importers.
    assert_eq!(compiler.computed_parse_count(), 3,
        "the shared import must be parsed once, not once per importer");
}
