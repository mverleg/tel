//! `telsb` — the sandbox CLI (sandbox/plans/daemon.md).
//!
//! With a `tel.toml` workspace root, `run` and `watch` go through the
//! per-root daemon (spawned on demand, warm cache); without one — or with
//! `--no-daemon` — they run in-process, cold and hermetic, exactly today's
//! `run_file` path. The daemon is this same binary re-exec'd as
//! `telsb server --root <dir>`.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use clap::{Parser, Subcommand};
use sandbox::monitor::{DiskMonitor, FileMonitor};
use sandbox_daemon::proto::{run_event, CompileRequest, HandshakeRequest, ShutdownRequest};
use sandbox_daemon::{client, discovery, server, version};

#[derive(Parser)]
#[clap(name = "telsb", about = "Sandbox language CLI: compiles via a per-workspace daemon when a tel.toml root exists.")]
struct Cli {
    #[clap(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Compile and execute a .telsb file (batch stance).
    Run {
        entry: PathBuf,
        /// Show the dependency tree after the run.
        #[clap(long)]
        show_deps: bool,
        /// Run in-process (cold cache, no daemon involved).
        #[clap(long)]
        no_daemon: bool,
    },
    /// Compile, then recompile on every file change until interrupted.
    Watch {
        entry: PathBuf,
        /// Watch in-process instead of through the daemon.
        #[clap(long)]
        no_daemon: bool,
    },
    /// Run the daemon for a workspace root (normally spawned by `run`/`watch`).
    Server {
        #[clap(long)]
        root: PathBuf,
    },
    /// Stop the daemon for a workspace root (defaults to the root above the
    /// current directory).
    Shutdown { dir: Option<PathBuf> },
    /// Report whether a daemon is running for a workspace root.
    Status { dir: Option<PathBuf> },
}

#[tokio::main]
async fn main() -> ExitCode {
    env_logger::init();
    let cli = Cli::parse();
    let code = match cli.cmd {
        Cmd::Run { entry, show_deps, no_daemon } => cmd_run(&entry, show_deps, no_daemon).await,
        Cmd::Watch { entry, no_daemon } => cmd_watch(&entry, no_daemon).await,
        Cmd::Server { root } => cmd_server(&root).await,
        Cmd::Shutdown { dir } => cmd_shutdown(dir.as_deref()).await,
        Cmd::Status { dir } => cmd_status(dir.as_deref()).await,
    };
    ExitCode::from(code)
}

/// Canonicalize the entry path — required, not cosmetic: the engine interns
/// paths textually and the daemon matches file events against them (path
/// identity contract, sandbox/src/monitor.rs).
fn canonical_entry(entry: &Path) -> Result<PathBuf, u8> {
    entry.canonicalize().map_err(|e| {
        eprintln!("Error: {}: {}", entry.display(), e);
        1u8
    })
}

async fn cmd_run(entry: &Path, show_deps: bool, no_daemon: bool) -> u8 {
    let entry = match canonical_entry(entry) {
        Ok(entry) => entry,
        Err(code) => return code,
    };
    let root = discovery::find_root(&entry);
    let (root, use_daemon) = match (no_daemon, root) {
        (false, Some(root)) => (root, true),
        // No tel.toml above the entry: fall back to in-process, same as
        // --no-daemon (a daemon needs a root to own).
        _ => (PathBuf::new(), false),
    };

    if !use_daemon {
        return match sandbox::run_file(entry.to_str().expect("canonical path is UTF-8"), show_deps).await {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("Error: {}", e);
                1
            }
        };
    }

    let mut client = match client::connect_or_spawn(&root).await {
        Ok(client) => client,
        Err(e) => {
            eprintln!("Error: {}", e);
            return 1;
        }
    };
    let request = CompileRequest {
        entry_path: entry.to_str().expect("canonical path is UTF-8").to_string(),
        show_deps,
    };
    let stream = match client.compile(request).await {
        Ok(response) => response.into_inner(),
        Err(status) => {
            eprintln!("Error: daemon refused compile: {}", status);
            return 1;
        }
    };
    // Compile has exactly one Done and it is terminal.
    match consume_stream(stream, false).await {
        StreamEnd::Done { ok: true } => 0,
        StreamEnd::Done { ok: false } => 1,
        StreamEnd::Broken => 1,
    }
}

async fn cmd_watch(entry: &Path, no_daemon: bool) -> u8 {
    let entry = match canonical_entry(entry) {
        Ok(entry) => entry,
        Err(code) => return code,
    };
    let root = discovery::find_root(&entry);

    if no_daemon || root.is_none() {
        // In-process watch: same loop the daemon runs, minus the daemon.
        let watch_root = root.unwrap_or_else(|| entry.parent().expect("canonical file has a parent").to_path_buf());
        let (mut monitor, mut events) = match DiskMonitor::new() {
            Ok(pair) => pair,
            Err(e) => {
                eprintln!("Error: {}", e);
                return 1;
            }
        };
        if let Err(e) = monitor.watch(&watch_root) {
            eprintln!("Error: {}", e);
            return 1;
        }
        let mut compiler = sandbox::Compiler::new(Arc::new(sandbox::StdoutPrinter));
        compiler.run_watch_loop(
            entry.to_str().expect("canonical path is UTF-8"),
            &mut events,
            |result| {
                if let Err(e) = result {
                    eprintln!("Error: {}", e);
                }
            },
        ).await;
        return 0; // unreachable in practice: the monitor lives for the whole loop
    }

    let root = root.expect("checked above");
    let mut client = match client::connect_or_spawn(&root).await {
        Ok(client) => client,
        Err(e) => {
            eprintln!("Error: {}", e);
            return 1;
        }
    };
    let request = CompileRequest {
        entry_path: entry.to_str().expect("canonical path is UTF-8").to_string(),
        show_deps: false,
    };
    let stream = match client.watch(request).await {
        Ok(response) => response.into_inner(),
        Err(status) => {
            eprintln!("Error: daemon refused watch: {}", status);
            return 1;
        }
    };
    // Watch streams one Done per wave and only ends abnormally (Ctrl-C kills
    // us first in the normal case).
    match consume_stream(stream, true).await {
        StreamEnd::Broken => 1,
        StreamEnd::Done { .. } => unreachable!("Done is not terminal when per_wave"),
    }
}

enum StreamEnd {
    /// A terminal Done was received (Compile).
    Done { ok: bool },
    /// The stream ended without a terminal Done (daemon gone, transport error).
    Broken,
}

/// Print output lines as they stream; handle Done terminally (Compile) or
/// per wave (Watch).
async fn consume_stream(mut stream: tonic::Streaming<sandbox_daemon::proto::RunEvent>, per_wave: bool) -> StreamEnd {
    loop {
        match stream.message().await {
            Ok(Some(event)) => match event.event {
                Some(run_event::Event::OutputLine(line)) => println!("{}", line),
                Some(run_event::Event::Done(done)) => {
                    if !done.ok {
                        eprintln!("Error: {}", done.error);
                    }
                    if !per_wave {
                        return StreamEnd::Done { ok: done.ok };
                    }
                }
                None => {} // unknown future event: ignore
            },
            Ok(None) => {
                eprintln!("Error: daemon closed the stream");
                return StreamEnd::Broken;
            }
            Err(status) => {
                eprintln!("Error: lost daemon connection: {}", status);
                return StreamEnd::Broken;
            }
        }
    }
}

async fn cmd_server(root: &Path) -> u8 {
    let root = match root.canonicalize() {
        Ok(root) => root,
        Err(e) => {
            eprintln!("Error: {}: {}", root.display(), e);
            return 1;
        }
    };
    let ad_file = discovery::ad_path(&root);
    match server::serve(&root, &ad_file).await {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("Error: {}", e);
            1
        }
    }
}

/// Resolve the root for daemon-management commands: explicit dir or cwd,
/// then the marker walk.
fn management_root(dir: Option<&Path>) -> Result<PathBuf, u8> {
    let start = match dir {
        Some(dir) => dir.to_path_buf(),
        None => std::env::current_dir().expect("cwd exists"),
    };
    let start = start.canonicalize().map_err(|e| {
        eprintln!("Error: {}: {}", start.display(), e);
        1u8
    })?;
    discovery::find_root(&start).ok_or_else(|| {
        eprintln!("Error: no {} found above {}", discovery::ROOT_MARKER, start.display());
        1u8
    })
}

async fn cmd_shutdown(dir: Option<&Path>) -> u8 {
    let root = match management_root(dir) {
        Ok(root) => root,
        Err(code) => return code,
    };
    let ad_file = discovery::ad_path(&root);
    let Some(ad) = discovery::read_ad(&ad_file) else {
        println!("no daemon for {}", root.display());
        return 0;
    };
    match client::try_connect(&ad).await {
        Some(mut client) => match client.shutdown(ShutdownRequest {}).await {
            Ok(_) => {
                println!("daemon for {} shutting down (pid {})", root.display(), ad.pid);
                0
            }
            Err(status) => {
                eprintln!("Error: shutdown refused: {}", status);
                1
            }
        },
        None => {
            // Nothing listening: crashed daemon, clear the stale ad.
            discovery::remove_ad(&ad_file);
            println!("no daemon for {} (removed stale ad)", root.display());
            0
        }
    }
}

async fn cmd_status(dir: Option<&Path>) -> u8 {
    let root = match management_root(dir) {
        Ok(root) => root,
        Err(code) => return code,
    };
    let ad_file = discovery::ad_path(&root);
    let Some(ad) = discovery::read_ad(&ad_file) else {
        println!("no daemon for {}", root.display());
        return 0;
    };
    match client::try_connect(&ad).await {
        Some(mut client) => {
            let mine = version::build_fingerprint();
            match client.handshake(HandshakeRequest { version: mine.clone() }).await {
                Ok(reply) => {
                    let theirs = reply.into_inner().version;
                    let note = if theirs == mine { "same build" } else { "DIFFERENT BUILD (next run replaces it)" };
                    println!(
                        "daemon for {}: pid {}, port {}, version {} ({})",
                        root.display(), ad.pid, ad.port, theirs, note,
                    );
                    0
                }
                Err(status) => {
                    eprintln!("Error: daemon reachable but handshake failed: {}", status);
                    1
                }
            }
        }
        None => {
            println!("stale ad for {} (pid {} not listening)", root.display(), ad.pid);
            0
        }
    }
}
