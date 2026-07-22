//! The daemon: one warm [`sandbox::Compiler`] per workspace root, served
//! over gRPC (plans/daemon.md). The daemon is the single home of the
//! in-memory layer — dependency graph, bindings, dirty marks — which is why
//! exactly one covers a root; the content store needs no such guard.
//!
//! Runs are serialized by a mutex around the compiler (mirroring the
//! engine's own `run(&mut self)` contract). A `Watch` stream holds the lock
//! only per wave, so `Compile` requests interleave between waves.

use std::path::PathBuf;
use std::sync::Arc;
use log::{info, warn};
use sandbox::monitor::{DiskMonitor, FileMonitor};
use sandbox::{Compiler, Printer};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_stream::wrappers::{TcpListenerStream, UnboundedReceiverStream};
use tonic::transport::Server;
use tonic::{Request, Response, Status};
use crate::discovery::{self, Ad, TOKEN_METADATA_KEY};
use crate::proto::sandbox_daemon_server::{SandboxDaemon, SandboxDaemonServer};
use crate::proto::{run_event, CompileRequest, Done, HandshakeReply, HandshakeRequest, RunEvent, ShutdownReply, ShutdownRequest};
use crate::version;

type EventTx = mpsc::UnboundedSender<Result<RunEvent, Status>>;
type EventStream = UnboundedReceiverStream<Result<RunEvent, Status>>;

/// Routes the compiler's print output to whichever request is currently
/// running. The compiler lock serializes runs, so the target is set and
/// cleared strictly around one run at a time — no cross-talk between
/// concurrent streams.
struct RoutingPrinter {
    target: std::sync::Mutex<Option<EventTx>>,
}

impl Printer for RoutingPrinter {
    fn print(&self, message: &str) {
        match &*self.target.lock().unwrap() {
            Some(tx) => {
                // A gone client is detected on the Done send; program output
                // to a closed stream is simply dropped.
                let _ = tx.send(Ok(output_line(message)));
            }
            // No run in flight demands output outside a request; daemon
            // stdout is null when spawned, so this only matters for
            // foreground debugging.
            None => println!("{}", message),
        }
    }
}

fn output_line(message: &str) -> RunEvent {
    RunEvent { event: Some(run_event::Event::OutputLine(message.to_string())) }
}

fn done_event(result: Result<(), sandbox::Error>) -> RunEvent {
    let done = match result {
        Ok(()) => Done { ok: true, error: String::new() },
        Err(e) => Done { ok: false, error: e.to_string() },
    };
    RunEvent { event: Some(run_event::Event::Done(done)) }
}

enum Stance {
    Batch,
    Watch,
}

struct DaemonState {
    root: PathBuf,
    /// The persistent cache directory under `root`. Watch events beneath it
    /// are the compiler's own cache writes (the cache lives inside the
    /// watched tree), so they must be filtered out before `invalidate` —
    /// otherwise every compile's write-through provokes another wave.
    cache_dir: PathBuf,
    compiler: tokio::sync::Mutex<Compiler>,
    printer: Arc<RoutingPrinter>,
    shutdown: mpsc::Sender<()>,
    version: String,
}

impl DaemonState {
    /// One run with output routed to `tx`, then its Done event. Returns
    /// whether the client is still listening (the cancellation signal:
    /// a dropped stream fails the send).
    async fn run_streaming(&self, compiler: &mut Compiler, req: &CompileRequest, stance: Stance, tx: &EventTx) -> bool {
        *self.printer.target.lock().unwrap() = Some(tx.clone());
        let result = match stance {
            Stance::Batch => compiler.run(&req.entry_path, req.show_deps).await,
            Stance::Watch => compiler.run_watch(&req.entry_path, req.show_deps).await,
        };
        *self.printer.target.lock().unwrap() = None;
        tx.send(Ok(done_event(result))).is_ok()
    }
}

struct DaemonService {
    state: Arc<DaemonState>,
}

#[tonic::async_trait]
impl SandboxDaemon for DaemonService {
    async fn handshake(&self, _request: Request<HandshakeRequest>) -> Result<Response<HandshakeReply>, Status> {
        // The daemon only reports its version; the client is the authority
        // on mismatch (plans/daemon.md "Versioning").
        Ok(Response::new(HandshakeReply { version: self.state.version.clone() }))
    }

    type CompileStream = EventStream;

    async fn compile(&self, request: Request<CompileRequest>) -> Result<Response<Self::CompileStream>, Status> {
        let req = request.into_inner();
        let (tx, rx) = mpsc::unbounded_channel();
        let state = self.state.clone();
        tokio::spawn(async move {
            let mut compiler = state.compiler.lock().await;
            state.run_streaming(&mut compiler, &req, Stance::Batch, &tx).await;
        });
        Ok(Response::new(UnboundedReceiverStream::new(rx)))
    }

    type WatchStream = EventStream;

    async fn watch(&self, request: Request<CompileRequest>) -> Result<Response<Self::WatchStream>, Status> {
        let req = request.into_inner();
        // Set up before returning the stream, so a failure to watch the root
        // surfaces as a proper error instead of an empty stream.
        let (mut monitor, mut events) = DiskMonitor::new()
            .map_err(|e| Status::internal(e.to_string()))?;
        monitor.watch(&self.state.root)
            .map_err(|e| Status::internal(e.to_string()))?;

        let (tx, rx) = mpsc::unbounded_channel();
        let state = self.state.clone();
        tokio::spawn(async move {
            // Owns the monitor for the life of the stream; dropping it on
            // exit releases the OS watches. A gone client is only noticed
            // when there is something to send, so an idle watch lingers
            // until the next file event — harmless, by design.
            let _monitor = monitor;
            {
                let mut compiler = state.compiler.lock().await;
                if !state.run_streaming(&mut compiler, &req, Stance::Watch, &tx).await {
                    return;
                }
            }
            while let Some(batch) = events.next_batch().await {
                // Drop the compiler's own cache writes: they live under
                // `root` (so the OS reports them) but announcing them would
                // dirty nothing and spin an extra wave per compile.
                let relevant: Vec<_> = batch.iter()
                    .filter(|p| !p.starts_with(&state.cache_dir))
                    .collect();
                if relevant.is_empty() {
                    continue;
                }
                // Lock per wave: Compile requests interleave between waves.
                let mut compiler = state.compiler.lock().await;
                for changed in relevant {
                    match changed.to_str() {
                        Some(changed) => compiler.invalidate(changed),
                        None => warn!("ignoring non-UTF8 changed path {:?}", changed),
                    }
                }
                if !state.run_streaming(&mut compiler, &req, Stance::Watch, &tx).await {
                    return;
                }
            }
        });
        Ok(Response::new(UnboundedReceiverStream::new(rx)))
    }

    async fn shutdown(&self, _request: Request<ShutdownRequest>) -> Result<Response<ShutdownReply>, Status> {
        info!("shutdown requested");
        let _ = self.state.shutdown.try_send(());
        Ok(Response::new(ShutdownReply {}))
    }
}

/// Run the daemon for `root` until a Shutdown RPC: bind an ephemeral
/// localhost port, advertise it in `ad_file` (with the auth token and build
/// fingerprint), serve, and remove the ad on the way out.
pub async fn serve(root: &std::path::Path, ad_file: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    let root = root.canonicalize()?;
    let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
    let port = listener.local_addr()?.port();
    let token = format!("{:032x}", rand::random::<u128>());
    let fingerprint = version::build_fingerprint();

    let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
    // The compiler's printer and the state's routing handle must be one
    // object: run_streaming redirects exactly the printer the compiler holds.
    let printer = Arc::new(RoutingPrinter { target: std::sync::Mutex::new(None) });
    let cache_dir = sandbox::default_cache_dir(&root);
    // The daemon is the persistent-cache's primary consumer (plans/daemon.md).
    // Degrade, don't die: an unusable cache dir means memory-only, same as
    // `--no-daemon`, rather than a dead daemon.
    let mut compiler = match Compiler::with_disk_cache(printer.clone(), &cache_dir) {
        Ok(compiler) => compiler,
        Err(e) => {
            warn!("disk cache at {} unavailable ({}); running memory-only", cache_dir.display(), e);
            Compiler::new(printer.clone())
        }
    };
    // Optional in-memory cache budget (plans/concurrency-and-eviction.md
    // Decision 3). A *soft high-water mark*, not a hard cap: a compile may grow
    // the cache past it, and once the compile completes the store is GC'd back
    // down (size-aware LRU, warmth-only — evicted entries re-load from the disk
    // tier). Unset ⇒ unbounded, the prior behavior. Env-configured so it needs
    // no schema; bytes, e.g. `TEL_SANDBOX_CACHE_BUDGET=536870912` for 512 MiB.
    match std::env::var("TEL_SANDBOX_CACHE_BUDGET") {
        Ok(raw) => match raw.trim().parse::<u64>() {
            Ok(bytes) => {
                info!("cache budget: {} bytes (soft, GC'd on compile completion)", bytes);
                compiler.set_cache_budget(bytes);
            }
            Err(_) => warn!("ignoring TEL_SANDBOX_CACHE_BUDGET={raw:?}: not a byte count"),
        },
        Err(_) => {} // unset: unbounded
    }
    let state = Arc::new(DaemonState {
        root: root.clone(),
        cache_dir,
        compiler: tokio::sync::Mutex::new(compiler),
        printer,
        shutdown: shutdown_tx,
        version: fingerprint.clone(),
    });

    let ad = Ad {
        port,
        pid: std::process::id(),
        version: fingerprint,
        token: token.clone(),
    };
    discovery::write_ad(ad_file, &ad)?;
    info!("daemon for {} listening on 127.0.0.1:{}", root.display(), port);

    let check_token = move |request: Request<()>| -> Result<Request<()>, Status> {
        let presented = request.metadata().get(TOKEN_METADATA_KEY).and_then(|v| v.to_str().ok());
        if presented == Some(token.as_str()) {
            Ok(request)
        } else {
            Err(Status::unauthenticated("missing or wrong token (read it from the ad file)"))
        }
    };

    let result = Server::builder()
        .add_service(SandboxDaemonServer::with_interceptor(DaemonService { state }, check_token))
        .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async move {
            shutdown_rx.recv().await;
        })
        .await;

    discovery::remove_ad(ad_file);
    info!("daemon for {} exited", root.display());
    Ok(result?)
}
