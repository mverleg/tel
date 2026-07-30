//! External dependencies as sealed leaves, end to end (plans/external-deps.md,
//! Slice 2).
//!
//! A local file is a *mutable* leaf: re-read and re-hashed every compile to
//! derive its lookup digest. A dependency is **immutable by contract**, so its
//! digest is taken once — at lock time, recorded in the lockfile — and the
//! artifact is never read to compute a key. These tests pin the two halves of
//! that:
//!
//! - the digest really does come from the coordinate (a warm run compiles with
//!   the dependency's bytes *deleted*);
//! - *which* coordinates are in play is decided by the lockfile, which is an
//!   ordinary tracked leaf — and it is depended on **per package**, so bumping
//!   one dependency leaves every other importer alone.
//!
//! All tests in this file share one dependency store (`XDG_CACHE_HOME` is
//! process-global, and the store is content-addressed so sharing is safe).

use sandbox::{Compiler, NoopPrinter, ROOT_MARKER};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use tempfile::TempDir;

/// The one dependency store for this test binary: `$XDG_CACHE_HOME/tel/deps`.
/// Set once, before any compile, so the parallel tests all agree on it.
fn deps_root() -> &'static Path {
    static STORE: OnceLock<(TempDir, PathBuf)> = OnceLock::new();
    let (_, root) = STORE.get_or_init(|| {
        let dir = TempDir::new().unwrap();
        std::env::set_var("XDG_CACHE_HOME", dir.path());
        let root = dir.path().join("tel").join("deps");
        (dir, root)
    });
    root
}

/// Publish a package into the sealed store under `release_hash`, as a fetch
/// would have. The hash is opaque to the engine — it only has to match what the
/// lockfile pins — so the tests spell it literally.
fn publish(release_hash: &str, files: &[(&str, &str)]) {
    let dir = deps_root().join(release_hash);
    for (name, content) in files {
        let path = dir.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }
}

/// A lockfile pinning each `(package, release hash)` at version 1.0.0.
fn lockfile(packages: &[(&str, &str)]) -> String {
    let entries: Vec<String> = packages.iter()
        .map(|(name, hash)| format!(
            r#""{name}": {{ "hash": "{hash}", "algo": "xxh3-128-tree", "version": "1.0.0" }}"#))
        .collect();
    format!(r#"{{ "version": 1, "packages": {{ {} }} }}"#, entries.join(", "))
}

/// A workspace root with a marker, so imports anchor here.
fn workspace() -> TempDir {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join(ROOT_MARKER), "").unwrap();
    dir
}

fn compiler() -> Compiler {
    Compiler::new(Arc::new(NoopPrinter))
}

/// An import whose first segment names a locked package resolves into the
/// sealed store rather than to a file under the project root.
#[tokio::test]
async fn an_import_of_a_locked_package_resolves_into_the_store() {
    publish("aa01", &[("dbl.telsb", "(* (arg 1) 2)\n")]);
    let ws = workspace();
    fs::write(ws.path().join("tel.lock"), lockfile(&[("mathlib", "aa01")])).unwrap();
    let main = ws.path().join("main.telsb");
    fs::write(&main, "(import /mathlib/dbl)\n(print (call dbl 21))\n").unwrap();

    compiler().run(main.to_str().unwrap(), false).await
        .expect("a locked package's file must resolve into the sealed store");
}

/// The sealed optimization itself: the digest comes from the coordinate, so a
/// **warm** run never reads the dependency's bytes. Deleting the source after
/// the first compile is the sharpest way to show it — a mutable leaf would fail
/// here, because it must re-read to know what it hashes.
#[tokio::test]
async fn a_warm_run_does_not_read_the_sealed_source() {
    publish("bb02", &[("vec.telsb", "(+ (arg 1) 1)\n")]);
    let ws = workspace();
    fs::write(ws.path().join("tel.lock"), lockfile(&[("geom", "bb02")])).unwrap();
    let main = ws.path().join("main.telsb");
    fs::write(&main, "(import /geom/vec)\n(print (call vec 41))\n").unwrap();

    let mut compiler = compiler();
    compiler.run(main.to_str().unwrap(), false).await.expect("cold compile");

    // The bytes are gone; only their coordinate remains, which is all the key
    // needs. (An immutable file cannot legitimately change, so this cannot
    // serve a stale answer — that is the contract sealing rests on.)
    fs::remove_file(deps_root().join("bb02").join("vec.telsb")).unwrap();

    compiler.run(main.to_str().unwrap(), false).await
        .expect("a warm run must key from the coordinate, not from a re-read");
}

/// A dependency the lockfile does not name is *not* sealed, and the import
/// falls back to a path under the project root — the plain local case, which
/// must keep working unchanged.
#[tokio::test]
async fn an_unlocked_first_segment_is_a_local_path() {
    let ws = workspace();
    fs::write(ws.path().join("tel.lock"), lockfile(&[])).unwrap();
    fs::create_dir_all(ws.path().join("mathlib")).unwrap();
    fs::write(ws.path().join("mathlib/dbl.telsb"), "(* (arg 1) 2)\n").unwrap();
    let main = ws.path().join("main.telsb");
    fs::write(&main, "(import /mathlib/dbl)\n(print (call dbl 21))\n").unwrap();

    compiler().run(main.to_str().unwrap(), false).await
        .expect("an unlocked path is an ordinary local import");
}

/// A path that is *both* a locked package and a local file is refused. Neither
/// wins: precedence is the shadowing rule this design deliberately does not
/// have, and silently preferring one would decide it by accident.
#[tokio::test]
async fn a_local_file_colliding_with_a_package_is_rejected() {
    publish("cc03", &[("dbl.telsb", "(* (arg 1) 2)\n")]);
    let ws = workspace();
    fs::write(ws.path().join("tel.lock"), lockfile(&[("mathlib", "cc03")])).unwrap();
    // Same import path, two candidates.
    fs::create_dir_all(ws.path().join("mathlib")).unwrap();
    fs::write(ws.path().join("mathlib/dbl.telsb"), "(* (arg 1) 3)\n").unwrap();
    let main = ws.path().join("main.telsb");
    fs::write(&main, "(import /mathlib/dbl)\n(print (call dbl 21))\n").unwrap();

    let msg = compiler().run(main.to_str().unwrap(), false).await
        .expect_err("an ambiguous import must fail").to_string();
    assert!(msg.contains("Ambiguous import"), "got: {msg}");
    assert!(msg.contains("mathlib"), "the error must name the package, got: {msg}");
}

/// `/pkg` names a package, not a file inside it.
#[tokio::test]
async fn a_package_without_a_file_is_rejected() {
    publish("dd04", &[("dbl.telsb", "(* (arg 1) 2)\n")]);
    let ws = workspace();
    fs::write(ws.path().join("tel.lock"), lockfile(&[("mathlib", "dd04")])).unwrap();
    let main = ws.path().join("main.telsb");
    fs::write(&main, "(import /mathlib)\n(print 1)\n").unwrap();

    let msg = compiler().run(main.to_str().unwrap(), false).await
        .expect_err("importing a bare package must fail").to_string();
    assert!(msg.contains("no file within it"), "got: {msg}");
}

/// A broken lockfile is reported *as* a broken lockfile, not collapsed into
/// "no packages" — otherwise a typo would silently turn every dependency into a
/// missing local file, and the two states would be indistinguishable to the
/// cache as well as to the reader.
#[tokio::test]
async fn an_unreadable_lockfile_is_reported_as_such() {
    publish("ee05", &[("dbl.telsb", "(* (arg 1) 2)\n")]);
    let ws = workspace();
    // A format version this build cannot read.
    fs::write(ws.path().join("tel.lock"),
        r#"{ "version": 99, "packages": {} }"#).unwrap();
    let main = ws.path().join("main.telsb");
    fs::write(&main, "(import /mathlib/dbl)\n(print (call dbl 21))\n").unwrap();

    let msg = compiler().run(main.to_str().unwrap(), false).await
        .expect_err("a lockfile this build cannot read must fail the compile").to_string();
    assert!(msg.contains("lockfile"), "got: {msg}");
    assert!(msg.contains("version 99"), "the error must say what is wrong, got: {msg}");
}

/// The same import text under a *broken* lockfile and under one that merely
/// lacks the package are different answers, so they must not share a key: the
/// error is carried in the projection's answer rather than collapsed to "not a
/// package". Here the local file exists, so the second case succeeds — which it
/// could not do if a cached "unusable lockfile" error were serving both.
#[tokio::test]
async fn a_broken_lockfile_does_not_alias_a_missing_package() {
    let ws = workspace();
    let lock = ws.path().join("tel.lock");
    fs::create_dir_all(ws.path().join("mathlib")).unwrap();
    fs::write(ws.path().join("mathlib/dbl.telsb"), "(* (arg 1) 2)\n").unwrap();
    let main = ws.path().join("main.telsb");
    fs::write(&main, "(import /mathlib/dbl)\n(print (call dbl 21))\n").unwrap();

    let mut compiler = compiler();
    fs::write(&lock, r#"{ "version": 99, "packages": {} }"#).unwrap();
    compiler.run(main.to_str().unwrap(), false).await.expect_err("broken lockfile fails");

    // Same program, same import, repaired lockfile: the import is now an
    // ordinary local path.
    fs::write(&lock, lockfile(&[])).unwrap();
    compiler.run(main.to_str().unwrap(), false).await
        .expect("with a readable lockfile the import resolves locally");
}

/// The reason the lockfile is depended on **per package**: bumping one
/// dependency must not re-resolve importers of the others. One lockfile is
/// shared by every entry point in a workspace, so a whole-table dependency
/// would fan a one-package bump out to all of them.
#[tokio::test]
async fn bumping_one_package_leaves_other_importers_alone() {
    publish("1a01", &[("f.telsb", "(+ (arg 1) 1)\n")]);
    publish("1a02", &[("f.telsb", "(+ (arg 1) 2)\n")]); // alpha, bumped
    publish("1b01", &[("g.telsb", "(* (arg 1) 10)\n")]);

    let ws = workspace();
    let lock = ws.path().join("tel.lock");
    fs::write(&lock, lockfile(&[("alpha", "1a01"), ("beta", "1b01")])).unwrap();

    // Two importers, one per package, joined by main.
    fs::write(ws.path().join("uses_alpha.telsb"),
        "(import /alpha/f)\n(call f (arg 1))\n").unwrap();
    fs::write(ws.path().join("uses_beta.telsb"),
        "(import /beta/g)\n(call g (arg 1))\n").unwrap();
    let main = ws.path().join("main.telsb");
    fs::write(&main, "(import /uses_alpha)\n(import /uses_beta)\n\
        (print (+ (call uses_alpha 1) (call uses_beta 1)))\n").unwrap();

    let mut compiler = compiler();
    compiler.run_watch(main.to_str().unwrap(), false).await.expect("cold compile");
    let before = compiler.computed_resolve_count();

    // Bump alpha only.
    fs::write(&lock, lockfile(&[("alpha", "1a02"), ("beta", "1b01")])).unwrap();
    compiler.invalidate(lock.to_str().unwrap());
    compiler.run_watch(main.to_str().unwrap(), false).await.expect("recompile after bump");

    let recomputed = compiler.computed_resolve_count() - before;
    // alpha's file, its importer, and main (whose import answer changed) —
    // but *not* uses_beta, which a whole-lockfile dependency would have
    // dragged in.
    assert!(recomputed <= 3,
        "a one-package bump must not re-resolve every importer, recomputed {recomputed}");
    assert!(recomputed >= 1, "the bumped package's importer must re-resolve");
}

/// Semver is human-facing metadata, never a key ingredient — so editing only
/// the version string leaves every coordinate fingerprint unchanged and nothing
/// re-resolves. This is what "key on the release hash, not bare semver" buys.
#[tokio::test]
async fn a_semver_only_edit_recomputes_nothing() {
    publish("2a01", &[("f.telsb", "(+ (arg 1) 1)\n")]);
    let ws = workspace();
    let lock = ws.path().join("tel.lock");
    let with_version = |v: &str| format!(
        r#"{{ "version": 1, "packages": {{ "alpha": {{ "hash": "2a01", "algo": "xxh3-128-tree", "version": "{v}" }} }} }}"#);
    fs::write(&lock, with_version("1.0.0")).unwrap();

    let main = ws.path().join("main.telsb");
    fs::write(&main, "(import /alpha/f)\n(print (call f 41))\n").unwrap();

    let mut compiler = compiler();
    compiler.run_watch(main.to_str().unwrap(), false).await.expect("cold compile");
    let before = compiler.computed_resolve_count();

    fs::write(&lock, with_version("1.0.1")).unwrap();
    compiler.invalidate(lock.to_str().unwrap());
    compiler.run_watch(main.to_str().unwrap(), false).await.expect("recompile");

    assert_eq!(compiler.computed_resolve_count(), before,
        "a version-string-only edit must not change any coordinate fingerprint");
}
