//! External dependencies as **sealed leaves** (plans/external-deps.md).
//!
//! A local source file is a *mutable* leaf: the engine re-reads and re-hashes
//! it every compile to derive the lookup digest. An external dependency is a
//! *sealed* leaf — **immutable by contract** — so its content digest is taken
//! **once, at lock time** (recorded in the lockfile) instead of being
//! re-derived from the filesystem each run. This module owns the three pieces
//! that make that concrete:
//!
//! - [`ReleaseHash`] / [`LeafSource`] — the coordinate a sealed leaf keys on,
//!   `(release_hash, path within package)`, which deterministically names
//!   immutable bytes without reading them (see [`ContentDigest::sealed`]).
//! - [`Lockfile`] — the one **tracked, local** leaf that says *which* sealed
//!   coordinates are in play; editing it (a dep bump) yields a different
//!   coordinate → a different key, so "an external dependency changed" is
//!   handled entirely through the file we do watch and hash.
//! - [`deps_root`] / [`lock_package`] — where sealed sources live on disk, and
//!   the **temporary** hash-at-lock that produces a release hash for the
//!   sandbox (which has no registry). Both are placeholders until a real
//!   fetch/registry story exists.

use crate::keys::ContentDigest;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path as FsPath, PathBuf};
use xxhash_rust::xxh3::Xxh3;

/// The lockfile-recorded checksum that pins an entire dependency package's
/// bytes. Stored as the raw decoded bytes (whatever width the source used); the
/// hex form is only for display and for the on-disk store directory name.
///
/// This is a *claim of integrity*, not a bare semver: keying on it keeps the
/// content-addressed invariant exactly intact (the key is still a hash of
/// immutable bytes), while the optimization is purely about *when* the hash is
/// taken (plans/external-deps.md "Key on the release hash, not bare semver").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseHash(Vec<u8>);

impl ReleaseHash {
    pub fn from_bytes(bytes: Vec<u8>) -> ReleaseHash {
        ReleaseHash(bytes)
    }

    /// Parse a lowercase-or-uppercase hex string (as written in the lockfile).
    pub fn from_hex(s: &str) -> Result<ReleaseHash, LockError> {
        if s.len() % 2 != 0 {
            return Err(LockError::BadHash(format!("odd-length hex string: {s:?}")));
        }
        let mut bytes = Vec::with_capacity(s.len() / 2);
        let digits = s.as_bytes();
        for pair in digits.chunks_exact(2) {
            let hi = hex_val(pair[0]).ok_or_else(|| LockError::BadHash(format!("non-hex char in {s:?}")))?;
            let lo = hex_val(pair[1]).ok_or_else(|| LockError::BadHash(format!("non-hex char in {s:?}")))?;
            bytes.push((hi << 4) | lo);
        }
        Ok(ReleaseHash(bytes))
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Lowercase hex — the directory name a package's sealed source tree lives
    /// under (Cargo/Go both name their immutable stores by the pinned hash).
    pub fn hex(&self) -> String {
        let mut s = String::with_capacity(self.0.len() * 2);
        for b in &self.0 {
            s.push(char::from_digit((b >> 4) as u32, 16).unwrap());
            s.push(char::from_digit((b & 0xf) as u32, 16).unwrap());
        }
        s
    }
}

fn hex_val(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

/// Where a leaf's bytes come from, and — for a sealed leaf — how its digest is
/// taken. The load-bearing distinction of plans/external-deps.md: a `Local`
/// leaf is re-read and re-hashed every compile; a `Sealed` leaf's digest is a
/// fixed coordinate, so the artifact is never read to compute a key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LeafSource {
    /// A mutable, tracked local file. The digest is `ContentDigest::of(bytes)`
    /// read fresh each compile; the path is the logical id.
    Local,
    /// An immutable external dependency file. Its digest is derived from the
    /// coordinate below — never from the artifact bytes.
    Sealed(SealedCoord),
}

/// The immutable coordinate of one file inside a sealed package: the package's
/// pinned release hash plus the file's path *within* that package. Semver is
/// carried as human-facing metadata only — never a key ingredient.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SealedCoord {
    pub release_hash: ReleaseHash,
    pub path_within_package: String,
    pub semver: String,
}

impl SealedCoord {
    /// The leaf digest of this sealed file — taken from the coordinate, no read
    /// (plans/external-deps.md). This is the entire "sealed" optimization: the
    /// release hash was computed once at lock time, and here it is merely
    /// folded in.
    pub fn digest(&self) -> ContentDigest {
        ContentDigest::sealed(self.release_hash.as_bytes(), &self.path_within_package)
    }

    /// Absolute on-disk location of this file's bytes, for the *cold-miss*
    /// read (the key came from the coordinate; the bytes are still needed to
    /// actually parse on a miss). `<deps_root>/<release-hash>/<path>`.
    pub fn source_path(&self) -> PathBuf {
        deps_root().join(self.release_hash.hex()).join(&self.path_within_package)
    }
}

/// One package entry as written in the lockfile.
#[derive(Debug, Clone, Deserialize)]
struct PackageEntry {
    /// Hex-encoded release hash (hash-at-lock output for now).
    hash: String,
    version: String,
}

/// The on-disk lockfile: `package name -> pinned (hash, version)`. A tracked,
/// local leaf — the one file we watch and hash so a dep bump flows through the
/// normal invalidation path while the dependency artifacts stay frozen.
///
/// Format (JSON, versioned):
/// ```json
/// { "version": 1, "packages": { "mathlib": { "hash": "ab12…", "version": "1.4.2" } } }
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct Lockfile {
    #[allow(dead_code)] // reserved: reject formats this build can't read
    version: u32,
    packages: HashMap<String, PackageEntry>,
}

impl Lockfile {
    pub fn parse(json: &str) -> Result<Lockfile, LockError> {
        serde_json::from_str(json).map_err(|e| LockError::Parse(e.to_string()))
    }

    pub fn load(path: &FsPath) -> Result<Lockfile, LockError> {
        let text = std::fs::read_to_string(path).map_err(|e| LockError::Io(e.to_string()))?;
        Lockfile::parse(&text)
    }

    /// Resolve one file of a locked package to its sealed coordinate. `None`
    /// when the package is not in the lockfile — the caller then treats the
    /// import as a local sibling, exactly as today.
    pub fn coord(&self, package: &str, path_within_package: &str) -> Result<Option<SealedCoord>, LockError> {
        let Some(entry) = self.packages.get(package) else {
            return Ok(None);
        };
        Ok(Some(SealedCoord {
            release_hash: ReleaseHash::from_hex(&entry.hash)?,
            path_within_package: path_within_package.to_string(),
            semver: entry.version.clone(),
        }))
    }
}

/// Errors from reading a lockfile or a release hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LockError {
    Io(String),
    Parse(String),
    BadHash(String),
}

impl std::fmt::Display for LockError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LockError::Io(e) => write!(f, "lockfile IO error: {e}"),
            LockError::Parse(e) => write!(f, "lockfile parse error: {e}"),
            LockError::BadHash(e) => write!(f, "bad release hash: {e}"),
        }
    }
}

impl std::error::Error for LockError {}

/// Root of the sealed dependency source store: `$XDG_CACHE_HOME/tel/deps`,
/// falling back to `~/.cache/tel/deps` (Go's module cache and Deno's store both
/// live under the XDG cache dir — everything here is reconstructible). Only the
/// *cold-miss read* touches it; a warm run never does.
pub fn deps_root() -> PathBuf {
    cache_root().join("tel").join("deps")
}

fn cache_root() -> PathBuf {
    if let Some(dir) = std::env::var_os("XDG_CACHE_HOME") {
        if !dir.is_empty() {
            return PathBuf::from(dir);
        }
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join(".cache");
    }
    // Last resort: a relative cache dir (keeps the sandbox runnable in a bare
    // environment with neither var set).
    PathBuf::from(".cache")
}

/// **Temporary** hash-at-lock: derive a release hash by hashing a package's
/// source tree, deterministically. Stands in for a registry-provided checksum
/// until a real fetch story exists — it is *explicitly* a placeholder
/// (plans/external-deps.md decided this is interim).
///
/// Deterministic by construction: relative file paths are sorted before
/// folding, and each `(path, bytes)` pair is length-prefixed so no two trees
/// can alias. Uses xxh3-128 (already the engine's hash), rendered as hex.
pub fn lock_package(package_dir: &FsPath) -> Result<ReleaseHash, LockError> {
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    collect_files(package_dir, package_dir, &mut files)?;
    files.sort_by(|a, b| a.0.cmp(&b.0));

    let mut h = Xxh3::new();
    h.update(&(files.len() as u64).to_le_bytes());
    for (rel, bytes) in &files {
        let rel = rel.as_bytes();
        h.update(&(rel.len() as u64).to_le_bytes());
        h.update(rel);
        h.update(&(bytes.len() as u64).to_le_bytes());
        h.update(bytes);
    }
    Ok(ReleaseHash(h.digest128().to_be_bytes().to_vec()))
}

fn collect_files(root: &FsPath, dir: &FsPath, out: &mut Vec<(String, Vec<u8>)>) -> Result<(), LockError> {
    let entries = std::fs::read_dir(dir).map_err(|e| LockError::Io(e.to_string()))?;
    for entry in entries {
        let entry = entry.map_err(|e| LockError::Io(e.to_string()))?;
        let path = entry.path();
        let ft = entry.file_type().map_err(|e| LockError::Io(e.to_string()))?;
        if ft.is_dir() {
            collect_files(root, &path, out)?;
        } else if ft.is_file() {
            let rel = path.strip_prefix(root).unwrap_or(&path).to_string_lossy().replace('\\', "/");
            let bytes = std::fs::read(&path).map_err(|e| LockError::Io(e.to_string()))?;
            out.push((rel, bytes));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_roundtrips() {
        let h = ReleaseHash::from_hex("00ff10ab").unwrap();
        assert_eq!(h.as_bytes(), &[0x00, 0xff, 0x10, 0xab]);
        assert_eq!(h.hex(), "00ff10ab");
        assert!(ReleaseHash::from_hex("0f0").is_err(), "odd length must error");
        assert!(ReleaseHash::from_hex("zz").is_err(), "non-hex must error");
    }

    #[test]
    fn lockfile_resolves_known_package_only() {
        let lock = Lockfile::parse(
            r#"{ "version": 1, "packages": { "mathlib": { "hash": "ab12cd", "version": "1.4.2" } } }"#,
        )
        .unwrap();

        let coord = lock.coord("mathlib", "vec.telsb").unwrap().expect("known package resolves");
        assert_eq!(coord.semver, "1.4.2");
        assert_eq!(coord.path_within_package, "vec.telsb");
        assert_eq!(coord.release_hash, ReleaseHash::from_hex("ab12cd").unwrap());
        // The digest is exactly the coordinate digest — proving it is taken
        // from the lockfile, not from any file on disk.
        assert_eq!(
            coord.digest(),
            ContentDigest::sealed(&[0xab, 0x12, 0xcd], "vec.telsb"),
        );

        assert!(lock.coord("unknown", "x.telsb").unwrap().is_none(), "unlocked package is not sealed");
    }

    /// The property that gates a dep bump: a different pinned hash in the
    /// lockfile yields a different coordinate digest for the same file — so the
    /// edit re-keys the sealed cone, exactly like editing any tracked leaf.
    #[test]
    fn bumping_the_lockfile_rekeys_the_coordinate() {
        let coord = |hash: &str| {
            Lockfile::parse(&format!(
                r#"{{ "version": 1, "packages": {{ "m": {{ "hash": "{hash}", "version": "1.0.0" }} }} }}"#
            ))
            .unwrap()
            .coord("m", "lib.telsb")
            .unwrap()
            .unwrap()
            .digest()
        };
        assert_eq!(coord("aa"), coord("aa"));
        assert_ne!(coord("aa"), coord("bb"));
    }

    #[test]
    fn hash_at_lock_is_deterministic_and_content_sensitive() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join("sub")).unwrap();
        std::fs::write(dir.path().join("a.telsb"), "(print 1)").unwrap();
        std::fs::write(dir.path().join("sub/b.telsb"), "(print 2)").unwrap();

        let h1 = lock_package(dir.path()).unwrap();
        let h2 = lock_package(dir.path()).unwrap();
        assert_eq!(h1, h2, "same tree hashes identically");

        std::fs::write(dir.path().join("a.telsb"), "(print 99)").unwrap();
        let h3 = lock_package(dir.path()).unwrap();
        assert_ne!(h1, h3, "a content change changes the release hash");
    }
}
