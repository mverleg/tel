//! Workspace-root and daemon discovery (plans/daemon.md "Process model").
//!
//! The root is defined by a `tel.toml` marker file; clients walk up from the
//! entry file and the innermost marker wins, which also guarantees no two
//! daemons' trees overlap. A running daemon advertises itself in a file
//! under `$XDG_RUNTIME_DIR/tel/` — *not* inside the workspace (keeps repos
//! clean, works on read-only checkouts, inherits per-user permissions).
//!
//! The ad file is the trust boundary: it is written owner-only (0600) and
//! contains the auth token every RPC must present. Whoever can read the
//! file may talk to a daemon that executes code; whoever can merely reach
//! the port may not.
//!
//! Staleness: a crashed daemon leaves its ad behind. Liveness is decided by
//! connecting (src/client.rs), not by locking — a failed connect treats the
//! ad as stale and respawns. Known simplification: two clients racing a
//! cold start can each spawn a daemon; the loser's ad is overwritten and
//! the orphan idles until killed. Acceptable for the sandbox; a file lock
//! can arbitrate later without changing this layout.

use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

/// The workspace-root marker (plans/daemon.md: it defines the root and
/// nothing else for now).
pub const ROOT_MARKER: &str = "tel.toml";

/// Metadata key carrying the auth token on every RPC.
pub const TOKEN_METADATA_KEY: &str = "x-telsb-token";

/// A running daemon's advertisement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ad {
    pub port: u16,
    pub pid: u32,
    /// Build fingerprint (src/version.rs) — pre-checked before connecting,
    /// authoritative answer comes from Handshake.
    pub version: String,
    pub token: String,
}

/// Walk up from `start` (a file or directory) looking for [`ROOT_MARKER`];
/// the innermost marker wins.
pub fn find_root(start: &Path) -> Option<PathBuf> {
    let mut dir = if start.is_dir() { start } else { start.parent()? };
    loop {
        if dir.join(ROOT_MARKER).is_file() {
            return Some(dir.to_path_buf());
        }
        dir = dir.parent()?;
    }
}

/// Directory for ad files: `$XDG_RUNTIME_DIR/tel`, falling back to a
/// per-user directory under the system temp dir.
fn runtime_dir() -> PathBuf {
    match std::env::var_os("XDG_RUNTIME_DIR") {
        Some(dir) => PathBuf::from(dir).join("tel"),
        None => {
            let user = std::env::var("USER").unwrap_or_else(|_| "anon".to_string());
            std::env::temp_dir().join(format!("tel-{}", user))
        }
    }
}

/// Ad file path for `root` (must already be canonical — callers canonicalize
/// once at the CLI boundary). One file per root, named by a stable-enough
/// hash of the canonical path: `DefaultHasher::new()` is zero-keyed SipHash,
/// deterministic across processes of the same build; if a toolchain change
/// ever shifts it, the failure mode is an orphaned daemon the client can no
/// longer find — an idle process, not a wrong answer.
pub fn ad_path(root: &Path) -> PathBuf {
    let mut hasher = DefaultHasher::new();
    root.hash(&mut hasher);
    let name = root.file_name().and_then(|n| n.to_str()).unwrap_or("root");
    runtime_dir().join(format!("{}-{:016x}.json", name, hasher.finish()))
}

pub fn read_ad(ad_file: &Path) -> Option<Ad> {
    let bytes = fs::read(ad_file).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Write the ad owner-only; creates the runtime dir (0700) as needed.
pub fn write_ad(ad_file: &Path, ad: &Ad) -> io::Result<()> {
    let dir = ad_file.parent().expect("ad path always has a parent dir");
    fs::create_dir_all(dir)?;
    let json = serde_json::to_vec_pretty(ad).expect("Ad serialization cannot fail");
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(ad_file)?;
        file.write_all(&json)?;
        Ok(())
    }
    #[cfg(not(unix))]
    {
        fs::write(ad_file, json)
    }
}

/// Remove the ad; missing is fine (a racing client may have replaced it).
pub fn remove_ad(ad_file: &Path) {
    let _ = fs::remove_file(ad_file);
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn find_root_picks_innermost_marker() {
        let dir = TempDir::new().unwrap();
        let outer = dir.path();
        let inner = outer.join("nested/deeper");
        fs::create_dir_all(&inner).unwrap();
        fs::write(outer.join(ROOT_MARKER), "").unwrap();

        assert_eq!(find_root(&inner).as_deref(), Some(outer));

        fs::write(outer.join("nested").join(ROOT_MARKER), "").unwrap();
        assert_eq!(find_root(&inner).as_deref(), Some(outer.join("nested").as_path()));

        let entry = inner.join("main.telsb");
        fs::write(&entry, "(print 1)").unwrap();
        assert_eq!(find_root(&entry).as_deref(), Some(outer.join("nested").as_path()),
            "a file argument must resolve from its directory");
    }

    #[test]
    fn find_root_without_marker_is_none() {
        let dir = TempDir::new().unwrap();
        assert_eq!(find_root(dir.path()), None);
    }

    #[test]
    fn ad_roundtrip_and_distinct_paths_per_root() {
        let dir = TempDir::new().unwrap();
        let ad_file = dir.path().join("ad.json");
        let ad = Ad { port: 4242, pid: 1, version: "v".into(), token: "t".into() };
        write_ad(&ad_file, &ad).unwrap();
        let back = read_ad(&ad_file).unwrap();
        assert_eq!(back.port, 4242);
        assert_eq!(back.token, "t");

        assert_ne!(ad_path(Path::new("/a/proj")), ad_path(Path::new("/b/proj")),
            "same basename under different parents must not collide");

        remove_ad(&ad_file);
        assert!(read_ad(&ad_file).is_none());
        remove_ad(&ad_file); // second removal is fine
    }
}
