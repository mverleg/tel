//! Client-side daemon discovery: read the ad, connect, verify version, and
//! spawn a daemon when there is none (plans/daemon.md "Process model" /
//! "Versioning").
//!
//! Liveness is decided by connecting, never by locking: a failed connect
//! means the ad is stale (crashed daemon), so it is removed and a fresh
//! daemon spawned. On a version mismatch the client is the authority — it
//! asks the old daemon to shut down, waits for its ad to disappear, and
//! spawns its own build; `cargo install` upgrades self-heal instead of
//! failing mysteriously.

use std::path::Path;
use std::process::Stdio;
use std::time::{Duration, Instant};
use log::{debug, info};
use tonic::codegen::InterceptedService;
use tonic::metadata::{Ascii, MetadataValue};
use tonic::service::Interceptor;
use tonic::transport::{Channel, Endpoint};
use tonic::{Request, Status};
use crate::discovery::{self, Ad, TOKEN_METADATA_KEY};
use crate::proto::sandbox_daemon_client::SandboxDaemonClient;
use crate::proto::{HandshakeRequest, ShutdownRequest};
use crate::version;

/// How long a cold start may take end to end (spawn, bind, write ad,
/// connect) before the client gives up.
const SPAWN_DEADLINE: Duration = Duration::from_secs(15);
const RETRY_INTERVAL: Duration = Duration::from_millis(50);

/// Attaches the ad-file token to every request.
#[derive(Clone)]
pub struct AuthInterceptor {
    token: MetadataValue<Ascii>,
}

impl Interceptor for AuthInterceptor {
    fn call(&mut self, mut request: Request<()>) -> Result<Request<()>, Status> {
        request.metadata_mut().insert(TOKEN_METADATA_KEY, self.token.clone());
        Ok(request)
    }
}

pub type DaemonClient = SandboxDaemonClient<InterceptedService<Channel, AuthInterceptor>>;

/// Connect to an advertised daemon. `None` means the ad is stale (nothing
/// listening, or a token that doesn't parse into metadata).
pub async fn try_connect(ad: &Ad) -> Option<DaemonClient> {
    let endpoint = Endpoint::from_shared(format!("http://127.0.0.1:{}", ad.port)).ok()?
        .connect_timeout(Duration::from_secs(2));
    let channel = endpoint.connect().await.ok()?;
    let token: MetadataValue<Ascii> = ad.token.parse().ok()?;
    Some(SandboxDaemonClient::with_interceptor(channel, AuthInterceptor { token }))
}

/// Connect to the daemon for `root` (which must be canonical), spawning or
/// version-replacing one as needed. Returns a handshaken client.
pub async fn connect_or_spawn(root: &Path) -> Result<DaemonClient, String> {
    let ad_file = discovery::ad_path(root);
    let my_version = version::build_fingerprint();
    let deadline = Instant::now() + SPAWN_DEADLINE;
    let mut spawned = false;

    loop {
        if let Some(ad) = discovery::read_ad(&ad_file) {
            match try_connect(&ad).await {
                Some(mut client) => {
                    match client.handshake(HandshakeRequest { version: my_version.clone() }).await {
                        Ok(reply) => {
                            let theirs = reply.into_inner().version;
                            if theirs == my_version {
                                return Ok(client);
                            }
                            // Exact-version rule: the client wins. Ask the
                            // old daemon to exit; its ad disappears on the
                            // way out and the next iterations spawn ours.
                            info!("daemon version {} != ours {}; replacing it", theirs, my_version);
                            let _ = client.shutdown(ShutdownRequest {}).await;
                        }
                        Err(status) => {
                            // Live listener refusing us (e.g. token from a
                            // half-overwritten ad): treat as stale.
                            debug!("handshake refused ({}); discarding ad", status);
                            discovery::remove_ad(&ad_file);
                        }
                    }
                }
                None => {
                    debug!("stale ad (nothing listening on port {}); discarding", ad.port);
                    discovery::remove_ad(&ad_file);
                }
            }
        } else if !spawned {
            spawn_daemon(root)?;
            spawned = true;
        }

        if Instant::now() > deadline {
            return Err(format!(
                "no daemon for {} within {:?} (spawned: {})",
                root.display(), SPAWN_DEADLINE, spawned,
            ));
        }
        tokio::time::sleep(RETRY_INTERVAL).await;
    }
}

/// Re-exec this binary as the daemon, detached from the terminal. The child
/// inherits the environment, so discovery (XDG_RUNTIME_DIR) agrees between
/// client and daemon.
fn spawn_daemon(root: &Path) -> Result<(), String> {
    let exe = std::env::current_exe()
        .map_err(|e| format!("cannot find own executable: {}", e))?;
    std::process::Command::new(exe)
        .arg("server")
        .arg("--root")
        .arg(root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("failed to spawn daemon: {}", e))?;
    info!("spawned daemon for {}", root.display());
    Ok(())
}
