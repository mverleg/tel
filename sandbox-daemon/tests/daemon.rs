//! Daemon integration tests: the full binary lifecycle (spawn on demand,
//! reuse, shutdown) and the in-process protocol surface (watch waves, token
//! auth, error reporting).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use sandbox_daemon::client::{self, DaemonClient};
use sandbox_daemon::discovery::{self, Ad, ROOT_MARKER};
use sandbox_daemon::proto::sandbox_daemon_client::SandboxDaemonClient;
use sandbox_daemon::proto::{run_event, CompileRequest, Done, HandshakeRequest, RunEvent, ShutdownRequest};
use sandbox_daemon::server;
use tempfile::TempDir;

const DEADLINE: Duration = Duration::from_secs(15);

/// The hot/cold project from the sandbox invalidation tests: main imports
/// two files, so edits to one can be observed leaving the other cached.
fn hot_cold_project(dir: &Path) -> (PathBuf, PathBuf) {
    fs::write(dir.join(ROOT_MARKER), "").unwrap();
    let main = dir.join("main.telsb");
    let hot = dir.join("hot.telsb");
    fs::write(&main, "(import hot)\n(import cold)\n(print (+ (call hot 1) (call cold 1)))\n").unwrap();
    fs::write(&hot, "(+ (arg 1) 10)\n").unwrap();
    fs::write(dir.join("cold.telsb"), "(+ (arg 1) 100)\n").unwrap();
    (main, hot)
}

// ---------------------------------------------------------------------------
// Binary end-to-end
// ---------------------------------------------------------------------------

/// Kills a daemon pid on drop, so a failing test doesn't leak a process;
/// disarm after a verified clean shutdown.
struct KillGuard(Option<u32>);

impl Drop for KillGuard {
    fn drop(&mut self) {
        if let Some(pid) = self.0 {
            let _ = std::process::Command::new("kill").arg("-9").arg(pid.to_string()).status();
        }
    }
}

fn telsb(runtime_dir: &Path) -> std::process::Command {
    let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_telsb"));
    // Isolate discovery per test; the daemon inherits this from the client.
    cmd.env("XDG_RUNTIME_DIR", runtime_dir);
    cmd
}

/// The single ad written under this test's isolated runtime dir.
fn read_only_ad(runtime_dir: &Path) -> Option<Ad> {
    let tel_dir = runtime_dir.join("tel");
    let mut ads: Vec<Ad> = fs::read_dir(&tel_dir).ok()?
        .filter_map(|entry| discovery::read_ad(&entry.ok()?.path()))
        .collect();
    assert!(ads.len() <= 1, "one root, one daemon, one ad; got {}", ads.len());
    ads.pop()
}

fn wait_until(what: &str, mut check: impl FnMut() -> bool) {
    let deadline = Instant::now() + DEADLINE;
    while !check() {
        assert!(Instant::now() < deadline, "timed out waiting for {}", what);
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn run_spawns_reuses_and_shuts_down_a_daemon() {
    let project = TempDir::new().unwrap();
    let runtime = TempDir::new().unwrap();
    let (main, hot) = hot_cold_project(project.path());

    // Cold start: the run must spawn a daemon and still produce the output.
    let out = telsb(runtime.path()).arg("run").arg(&main).output().unwrap();
    assert!(out.status.success(), "run failed: {}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "112");

    let ad = read_only_ad(runtime.path()).expect("run must have advertised a daemon");
    let mut guard = KillGuard(Some(ad.pid));

    // Second run: same daemon (same pid in the ad), warm cache.
    let out = telsb(runtime.path()).arg("run").arg(&main).output().unwrap();
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "112");
    let ad2 = read_only_ad(runtime.path()).expect("ad must survive");
    assert_eq!(ad2.pid, ad.pid, "second run must reuse the daemon, not spawn another");

    // Edit + run: the warm daemon picks up the change (batch stance re-reads
    // demanded leaves, no invalidate needed).
    fs::write(&hot, "(+ (arg 1) 20)\n").unwrap();
    let out = telsb(runtime.path()).arg("run").arg(&main).output().unwrap();
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "122");

    // A compile error must reach the client as a nonzero exit + stderr.
    let broken = project.path().join("broken.telsb");
    fs::write(&broken, "(print (call nowhere 1))\n").unwrap();
    let out = telsb(runtime.path()).arg("run").arg(&broken).output().unwrap();
    assert!(!out.status.success(), "compile error must exit nonzero");
    assert!(!String::from_utf8_lossy(&out.stderr).trim().is_empty(), "error must be reported");

    // Status sees it; shutdown stops it and removes the ad.
    let out = telsb(runtime.path()).arg("status").arg(project.path()).output().unwrap();
    assert!(String::from_utf8_lossy(&out.stdout).contains(&format!("pid {}", ad.pid)));

    let out = telsb(runtime.path()).arg("shutdown").arg(project.path()).output().unwrap();
    assert!(out.status.success(), "shutdown failed: {}", String::from_utf8_lossy(&out.stderr));
    wait_until("ad removal after shutdown", || read_only_ad(runtime.path()).is_none());
    wait_until("daemon process exit", || !Path::new(&format!("/proc/{}", ad.pid)).exists());
    guard.0 = None; // clean shutdown verified; nothing to kill

    // Shutdown with no daemon is a friendly no-op.
    let out = telsb(runtime.path()).arg("shutdown").arg(project.path()).output().unwrap();
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("no daemon"));
}

#[test]
fn run_without_marker_falls_back_to_in_process() {
    let project = TempDir::new().unwrap();
    let runtime = TempDir::new().unwrap();
    let main = project.path().join("main.telsb");
    fs::write(&main, "(print 41)\n").unwrap(); // no ROOT_MARKER on purpose

    let out = telsb(runtime.path()).arg("run").arg(&main).output().unwrap();
    assert!(out.status.success(), "run failed: {}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "41");
    assert!(read_only_ad(runtime.path()).is_none(), "no root, no daemon");
}

/// The daemon persists its content store under `<root>/out/cache`, so its
/// work warms a later process. We prove it without adding RPC surface: run
/// through the daemon, shut it down (flushing the cache writer on
/// `Compiler` drop), then open an in-process `Compiler::with_disk_cache` on
/// the same cache dir and assert it recomputes nothing — the daemon's
/// writes are exactly the reusable cache.
#[tokio::test(flavor = "multi_thread")]
async fn daemon_persists_a_reusable_cache() {
    let project = TempDir::new().unwrap();
    let runtime = TempDir::new().unwrap();
    let root = project.path().canonicalize().unwrap();
    let (main, _hot) = hot_cold_project(&root);
    let entry = main.canonicalize().unwrap();

    let out = telsb(runtime.path()).arg("run").arg(&entry).output().unwrap();
    assert!(out.status.success(), "run failed: {}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "112");
    let ad = read_only_ad(runtime.path()).expect("run must have advertised a daemon");
    let mut guard = KillGuard(Some(ad.pid));

    // Shut down so the daemon drops its Compiler and flushes the writer.
    let out = telsb(runtime.path()).arg("shutdown").arg(&root).output().unwrap();
    assert!(out.status.success(), "shutdown failed: {}", String::from_utf8_lossy(&out.stderr));
    wait_until("daemon process exit", || !Path::new(&format!("/proc/{}", ad.pid)).exists());
    guard.0 = None;

    let cache_dir = sandbox::default_cache_dir(&root);
    assert!(cache_dir.join("data.mdb").exists(), "the daemon must have written an LMDB store");

    // Warmth probe: a fresh in-process compiler over the same cache dir must
    // serve everything the daemon computed.
    let printer: std::sync::Arc<dyn sandbox::Printer> = std::sync::Arc::new(sandbox::NoopPrinter);
    let mut probe = sandbox::Compiler::with_disk_cache(printer, &cache_dir).unwrap();
    probe.run(entry.to_str().unwrap(), false).await.unwrap();
    assert_eq!(
        (probe.computed_parse_count(), probe.computed_resolve_count(), probe.computed_mono_count()),
        (0, 0, 0),
        "the daemon's persisted cache must warm a later process completely",
    );
}

// ---------------------------------------------------------------------------
// In-process protocol tests
// ---------------------------------------------------------------------------

/// Start `server::serve` as a task and wait for its ad.
async fn start_server(root: &Path, ad_file: &Path) -> Ad {
    let root = root.to_path_buf();
    let ad_file_owned = ad_file.to_path_buf();
    tokio::spawn(async move {
        server::serve(&root, &ad_file_owned).await.expect("serve failed");
    });
    let deadline = Instant::now() + DEADLINE;
    loop {
        if let Some(ad) = discovery::read_ad(ad_file) {
            return ad;
        }
        assert!(Instant::now() < deadline, "server never advertised");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

async fn connect(ad: &Ad) -> DaemonClient {
    client::try_connect(ad).await.expect("advertised server must accept connections")
}

/// Read the stream up to the next Done, collecting output lines.
async fn next_wave(stream: &mut tonic::Streaming<RunEvent>, out: &mut Vec<String>) -> Done {
    loop {
        let message = tokio::time::timeout(DEADLINE, stream.message()).await
            .expect("no event within deadline")
            .expect("stream error")
            .expect("stream must not close mid-wave");
        match message.event {
            Some(run_event::Event::OutputLine(line)) => out.push(line),
            Some(run_event::Event::Done(done)) => return done,
            None => {}
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn watch_streams_a_wave_per_edit() {
    let project = TempDir::new().unwrap();
    let root = project.path().canonicalize().unwrap();
    let (main, hot) = hot_cold_project(&root);
    let runtime = TempDir::new().unwrap();
    let ad_file = runtime.path().join("ad.json");

    let ad = start_server(&root, &ad_file).await;
    let mut client = connect(&ad).await;

    let request = CompileRequest { entry_path: main.to_str().unwrap().to_string(), show_deps: false };
    let mut stream = client.watch(request).await.unwrap().into_inner();

    let mut out = Vec::new();
    let done = next_wave(&mut stream, &mut out).await;
    assert!(done.ok, "initial wave failed: {}", done.error);
    assert_eq!(out, vec!["112"]);

    fs::write(&hot, "(+ (arg 1) 20)\n").unwrap();
    let mut out = Vec::new();
    let done = next_wave(&mut stream, &mut out).await;
    assert!(done.ok, "edit wave failed: {}", done.error);
    assert_eq!(out, vec!["122"], "the wave triggered by the edit must recompute");

    // A broken edit is a failing wave, not a dead stream; fixing it heals.
    fs::write(&hot, "(+ (arg 1)\n").unwrap();
    let done = next_wave(&mut stream, &mut Vec::new()).await;
    assert!(!done.ok, "syntax error must fail the wave");
    assert!(!done.error.is_empty());

    fs::write(&hot, "(+ (arg 1) 30)\n").unwrap();
    let mut out = Vec::new();
    let done = next_wave(&mut stream, &mut out).await;
    assert!(done.ok, "healed wave failed: {}", done.error);
    assert_eq!(out, vec!["132"]);

    drop(stream);
    client.shutdown(ShutdownRequest {}).await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn requests_without_the_token_are_rejected() {
    let project = TempDir::new().unwrap();
    fs::write(project.path().join(ROOT_MARKER), "").unwrap();
    let root = project.path().canonicalize().unwrap();
    let runtime = TempDir::new().unwrap();
    let ad_file = runtime.path().join("ad.json");

    let ad = start_server(&root, &ad_file).await;

    // No token at all.
    let mut bare = SandboxDaemonClient::connect(format!("http://127.0.0.1:{}", ad.port)).await.unwrap();
    let status = bare.handshake(HandshakeRequest { version: "v".into() }).await
        .expect_err("tokenless request must be rejected");
    assert_eq!(status.code(), tonic::Code::Unauthenticated);

    // Wrong token.
    let forged = Ad { token: "0".repeat(32), ..ad.clone() };
    let mut wrong = client::try_connect(&forged).await.unwrap();
    let status = wrong.handshake(HandshakeRequest { version: "v".into() }).await
        .expect_err("wrong token must be rejected");
    assert_eq!(status.code(), tonic::Code::Unauthenticated);

    // The real token still works, then stops the server.
    let mut client = connect(&ad).await;
    client.handshake(HandshakeRequest { version: "v".into() }).await.unwrap();
    client.shutdown(ShutdownRequest {}).await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn shutdown_removes_the_ad() {
    let project = TempDir::new().unwrap();
    fs::write(project.path().join(ROOT_MARKER), "").unwrap();
    let root = project.path().canonicalize().unwrap();
    let runtime = TempDir::new().unwrap();
    let ad_file = runtime.path().join("ad.json");

    let ad = start_server(&root, &ad_file).await;
    let mut client = connect(&ad).await;
    client.shutdown(ShutdownRequest {}).await.unwrap();

    let deadline = Instant::now() + DEADLINE;
    while discovery::read_ad(&ad_file).is_some() {
        assert!(Instant::now() < deadline, "ad must be removed on shutdown");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}
