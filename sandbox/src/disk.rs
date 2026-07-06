//! The persistent tier of the content store: LMDB (via heed) holding
//! postcard-encoded [`crate::portable`] entries (plans/roadmap.md Phase 3,
//! item 12).
//!
//! Only the content store persists — entries under a [`ContentKey`] are
//! valid forever by construction (docs/cache-invalidation-problem.md), so
//! this tier is append-only and can never serve a stale answer, only a
//! missing one. The binding layer, graph, and everything session-scoped
//! stay in memory.
//!
//! **Reads** are inline and synchronous: an LMDB get is a B-tree lookup in
//! a memory-mapped file — microseconds, not blocking IO in any meaningful
//! sense — so no `spawn_blocking`; the `RoTxn` is created and dropped
//! inside each method and by construction never crosses an await.
//!
//! **Writes** go through one dedicated writer thread (LMDB write
//! transactions are single-writer and must not be held across awaits, and
//! writes arrive from both async and sync compute paths). The thread drains
//! everything pending into one batched transaction — a cold compile writes
//! thousands of entries and the batching amortizes commits — with
//! `NO_OVERWRITE`, the disk mirror of the in-memory first-write-wins
//! invariant. Sends are fire-and-forget: a write lost to a crash costs
//! warmth, never correctness. Dropping the [`DiskCache`] closes the channel
//! and *joins* the thread, so a clean shutdown (or a test) flushes
//! deterministically. Within a session, a not-yet-committed write is
//! invisible to reads — harmless, because the in-memory layer already holds
//! everything this session computed.
//!
//! **Versioning:** a meta record stores `(CACHE_FORMAT_VERSION,
//! SCHEMA_VERSION)`. The format version covers the postcard/portable layout
//! — which `SCHEMA_VERSION` (a key ingredient) does not protect: a layout
//! change with an unchanged schema would *misdecode* old bytes. Any
//! mismatch wipes the cache: format-mismatched bytes are undecodable, and
//! schema-mismatched keys can never be derived again (unreachable garbage);
//! a wipe is always safe because this is a cache.
//!
//! **Corruption** (bit rot, torn writes) is a miss, never a panic: decode
//! or portable-conversion failure logs a warning and returns `None`; the
//! recompute's write-through replaces the bad bytes.
//!
//! **Multi-process:** LMDB is single-writer/MVCC-readers across processes
//! natively, and content-addressing makes concurrent writers benign (equal
//! answers under equal keys — plans/daemon.md). Today one daemon per root
//! is the only writer; no extra locking.

// Transitional: consumed by the ContentStore disk tier (next step of this
// series); until it lands, only the tests exercise this module.
#![allow(dead_code)]

use std::path::Path as FsPath;
use std::sync::mpsc;
use std::thread;
use heed::types::{Bytes, Str};
use heed::{Database, Env, EnvOpenOptions, PutFlags};
use log::warn;
use serde::{Deserialize, Serialize};
use crate::common::Interner;
use crate::keys::{ContentKey, SCHEMA_VERSION};
use crate::portable::{self, PMonoAnswer, PortableEntry, PResolveEntry};
use crate::store::{MonoAnswer, ResolveEntry};
use crate::types::{ParseError, PreExpr};

/// Version of the on-disk encoding (postcard layout of the portable types
/// and this module's record shapes). Bump on any layout change; mismatched
/// caches are wiped on open.
const CACHE_FORMAT_VERSION: u32 = 1;

/// Virtual address-space reservation, not allocation, on Linux (the
/// project's platform); real usage grows with content. If the map ever
/// fills, writes fail and are dropped with a warning — the cache stays
/// valid, it just stops growing (eviction is roadmap item 13).
const MAP_SIZE: usize = 10 * 1024 * 1024 * 1024;

const META_FORMAT_KEY: &str = "format";

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
struct FormatStamp {
    format: u32,
    schema: u64,
}

/// Error opening the cache; rendered to a message so heed never leaks into
/// the public API (same stance as `monitor::MonitorError`).
#[derive(Debug)]
pub struct DiskCacheError {
    message: String,
}

impl std::fmt::Display for DiskCacheError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "disk cache error: {}", self.message)
    }
}

impl std::error::Error for DiskCacheError {}

impl From<heed::Error> for DiskCacheError {
    fn from(e: heed::Error) -> DiskCacheError {
        DiskCacheError { message: e.to_string() }
    }
}

impl From<std::io::Error> for DiskCacheError {
    fn from(e: std::io::Error) -> DiskCacheError {
        DiskCacheError { message: e.to_string() }
    }
}

enum WriteKind {
    Parse,
    Resolve,
    Mono,
}

struct WriteMsg {
    kind: WriteKind,
    key: [u8; 16],
    bytes: Vec<u8>,
}

pub(crate) struct DiskCache {
    env: Env,
    parse: Database<Bytes, Bytes>,
    resolve: Database<Bytes, Bytes>,
    mono: Database<Bytes, Bytes>,
    /// `Option` only so `Drop` can close the channel before joining.
    tx: Option<mpsc::Sender<WriteMsg>>,
    writer: Option<thread::JoinHandle<()>>,
}

impl DiskCache {
    pub(crate) fn open(dir: &FsPath) -> Result<DiskCache, DiskCacheError> {
        DiskCache::open_with_stamp(dir, FormatStamp { format: CACHE_FORMAT_VERSION, schema: SCHEMA_VERSION })
    }

    /// Test seam: open claiming a different format/schema, to prove the
    /// wipe-on-mismatch policy without forging raw bytes.
    fn open_with_stamp(dir: &FsPath, stamp: FormatStamp) -> Result<DiskCache, DiskCacheError> {
        std::fs::create_dir_all(dir)?;
        // SAFETY (heed's contract): no other process may have the file
        // mapped with a different page/map configuration, and the map must
        // not be truncated externally while open. The cache dir is owned by
        // this compiler (one daemon per root; tests use tempdirs).
        let env = unsafe {
            EnvOpenOptions::new()
                .map_size(MAP_SIZE)
                .max_dbs(4)
                .open(dir)?
        };

        let mut wtxn = env.write_txn()?;
        let parse = env.create_database(&mut wtxn, Some("parse"))?;
        let resolve = env.create_database(&mut wtxn, Some("resolve"))?;
        let mono = env.create_database(&mut wtxn, Some("mono"))?;
        let meta: Database<Str, Bytes> = env.create_database(&mut wtxn, Some("meta"))?;

        // Wipe-on-mismatch: absent stamp = fresh cache; a stamp that fails
        // to decode counts as mismatched too.
        let found = meta.get(&wtxn, META_FORMAT_KEY)?
            .and_then(|bytes| postcard::from_bytes::<FormatStamp>(bytes).ok());
        if found.as_ref() != Some(&stamp) {
            if let Some(found) = found {
                warn!(
                    "cache format {:?} != expected {:?}: wiping {} (entries were unreachable or undecodable)",
                    found, stamp, dir.display(),
                );
            }
            parse.clear(&mut wtxn)?;
            resolve.clear(&mut wtxn)?;
            mono.clear(&mut wtxn)?;
            let bytes = postcard::to_allocvec(&stamp).expect("FormatStamp serialization cannot fail");
            meta.put(&mut wtxn, META_FORMAT_KEY, &bytes)?;
        }
        wtxn.commit()?;

        let (tx, rx) = mpsc::channel::<WriteMsg>();
        let writer_env = env.clone();
        let writer = thread::Builder::new()
            .name("tel-sandbox-cache-writer".to_string())
            .spawn(move || write_loop(writer_env, parse, resolve, mono, rx))
            .map_err(DiskCacheError::from)?;

        Ok(DiskCache {
            env,
            parse,
            resolve,
            mono,
            tx: Some(tx),
            writer: Some(writer),
        })
    }

    // ---- reads (inline, sync) ----------------------------------------------

    pub(crate) fn get_parse(&self, key: ContentKey) -> Option<Result<PreExpr, ParseError>> {
        let bytes = self.get_raw(self.parse, key)?;
        decode_or_miss::<Result<PreExpr, ParseError>>(&bytes, "parse", key)
    }

    pub(crate) fn get_resolve(&self, key: ContentKey, interner: &Interner) -> Option<ResolveEntry> {
        let bytes = self.get_raw(self.resolve, key)?;
        let entry = decode_or_miss::<PortableEntry<PResolveEntry>>(&bytes, "resolve", key)?;
        let converted = portable::resolve_entry_from_portable(&entry, interner);
        if converted.is_none() {
            warn!("corrupt resolve cache entry under {:?} (bad string index): treating as miss", key);
        }
        converted
    }

    pub(crate) fn get_mono(&self, key: ContentKey, interner: &Interner) -> Option<MonoAnswer> {
        let bytes = self.get_raw(self.mono, key)?;
        let entry = decode_or_miss::<PortableEntry<PMonoAnswer>>(&bytes, "mono", key)?;
        let converted = portable::mono_answer_from_portable(&entry, interner);
        if converted.is_none() {
            warn!("corrupt mono cache entry under {:?} (bad string index): treating as miss", key);
        }
        converted
    }

    fn get_raw(&self, db: Database<Bytes, Bytes>, key: ContentKey) -> Option<Vec<u8>> {
        let rtxn = match self.env.read_txn() {
            Ok(rtxn) => rtxn,
            Err(e) => {
                warn!("disk cache read transaction failed: {}", e);
                return None;
            }
        };
        match db.get(&rtxn, &key.to_be_bytes()) {
            Ok(hit) => hit.map(|bytes| bytes.to_vec()),
            Err(e) => {
                warn!("disk cache read failed: {}", e);
                None
            }
        }
    }

    // ---- writes (fire-and-forget via the writer thread) ---------------------

    pub(crate) fn put_parse(&self, key: ContentKey, answer: &Result<PreExpr, ParseError>) {
        let bytes = postcard::to_allocvec(answer).expect("parse answers serialize infallibly");
        self.send(WriteKind::Parse, key, bytes);
    }

    pub(crate) fn put_resolve(&self, key: ContentKey, entry: &ResolveEntry, interner: &Interner) {
        let portable = portable::resolve_entry_to_portable(entry, interner);
        let bytes = postcard::to_allocvec(&portable).expect("portable entries serialize infallibly");
        self.send(WriteKind::Resolve, key, bytes);
    }

    pub(crate) fn put_mono(&self, key: ContentKey, answer: &MonoAnswer, interner: &Interner) {
        let portable = portable::mono_answer_to_portable(answer, interner);
        let bytes = postcard::to_allocvec(&portable).expect("portable entries serialize infallibly");
        self.send(WriteKind::Mono, key, bytes);
    }

    fn send(&self, kind: WriteKind, key: ContentKey, bytes: Vec<u8>) {
        let msg = WriteMsg { kind, key: key.to_be_bytes(), bytes };
        // A closed channel means the writer died (it logged why); dropping
        // the write is the documented degradation.
        let _ = self.tx.as_ref().expect("tx lives until Drop").send(msg);
    }
}

impl Drop for DiskCache {
    fn drop(&mut self) {
        drop(self.tx.take()); // close the channel: the writer drains and exits
        if let Some(writer) = self.writer.take() {
            if writer.join().is_err() {
                warn!("cache writer thread panicked; pending writes were lost (cache stays valid)");
            }
        }
    }
}

/// The writer thread: drain everything pending into one transaction per
/// wake-up, commit, repeat until the channel closes.
fn write_loop(
    env: Env,
    parse: Database<Bytes, Bytes>,
    resolve: Database<Bytes, Bytes>,
    mono: Database<Bytes, Bytes>,
    rx: mpsc::Receiver<WriteMsg>,
) {
    while let Ok(first) = rx.recv() {
        let mut batch = vec![first];
        while let Ok(msg) = rx.try_recv() {
            batch.push(msg);
        }
        if let Err(e) = commit_batch(&env, parse, resolve, mono, &batch) {
            // A failed transaction is aborted whole (MAP_FULL, IO error):
            // the batch is lost, the cache stays valid, warmth degrades.
            warn!("disk cache batch of {} dropped ({}); cache stays valid", batch.len(), e);
        }
    }
}

fn commit_batch(
    env: &Env,
    parse: Database<Bytes, Bytes>,
    resolve: Database<Bytes, Bytes>,
    mono: Database<Bytes, Bytes>,
    batch: &[WriteMsg],
) -> Result<(), heed::Error> {
    let mut wtxn = env.write_txn()?;
    for msg in batch {
        let db = match msg.kind {
            WriteKind::Parse => parse,
            WriteKind::Resolve => resolve,
            WriteKind::Mono => mono,
        };
        // NO_OVERWRITE: first write wins, like the in-memory tables; a
        // racing equal write is dropped silently.
        match db.put_with_flags(&mut wtxn, PutFlags::NO_OVERWRITE, &msg.key, &msg.bytes) {
            Ok(()) | Err(heed::Error::Mdb(heed::MdbError::KeyExist)) => {}
            Err(e) => return Err(e),
        }
    }
    wtxn.commit()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::FQ;
    use crate::keys::{Fingerprint, QueryKind, StableCtx};
    use crate::store::ResolveAnswer;
    use crate::types::{Expr, FuncId, SymbolTable};
    use tempfile::TempDir;

    fn key(n: u64) -> ContentKey {
        ContentKey::build(QueryKind::Parse, |h| h.write_u64(n))
    }

    fn sample_resolve_entry(interner: &Interner) -> ResolveEntry {
        let ast = Expr::Call {
            func: FuncId(FQ::intern(interner, "lib.telsb", "f")),
            args: vec![Box::new(Expr::Arg(1))],
        };
        ResolveEntry {
            answer: Ok(ResolveAnswer { ast, table: SymbolTable::new(), funcs: vec![] }),
            fingerprint: Fingerprint::of(&1u64, &StableCtx { interner }),
        }
    }

    /// Write through the channel, drop (joins the writer), reopen, read —
    /// the cross-process persistence path in miniature, for all three
    /// tables.
    #[test]
    fn entries_survive_reopen() {
        let dir = TempDir::new().unwrap();
        let interner = Interner::new();

        let cache = DiskCache::open(dir.path()).unwrap();
        cache.put_parse(key(1), &Ok(PreExpr::Number { value: 42, ty: None }));
        cache.put_resolve(key(2), &sample_resolve_entry(&interner), &interner);
        cache.put_mono(key(3), &Err(crate::types::TypeError::FunctionNotResolved { context: "f".into() }), &interner);
        drop(cache);

        let cache = DiskCache::open(dir.path()).unwrap();
        let fresh = Interner::new();
        let parse = cache.get_parse(key(1)).expect("parse entry must survive reopen");
        assert!(matches!(parse, Ok(PreExpr::Number { value: 42, .. })));
        let resolve = cache.get_resolve(key(2), &fresh).expect("resolve entry must survive reopen");
        assert_eq!(resolve.fingerprint, sample_resolve_entry(&interner).fingerprint);
        let mono = cache.get_mono(key(3), &fresh).expect("mono entry must survive reopen");
        assert!(matches!(mono, Err(crate::types::TypeError::FunctionNotResolved { .. })));

        assert!(cache.get_parse(key(99)).is_none(), "unknown key is a miss");
    }

    /// First write wins on disk like in memory: a second value under the
    /// same key is silently dropped.
    #[test]
    fn first_write_wins() {
        let dir = TempDir::new().unwrap();
        let cache = DiskCache::open(dir.path()).unwrap();
        cache.put_parse(key(1), &Ok(PreExpr::Number { value: 1, ty: None }));
        drop(cache);
        let cache = DiskCache::open(dir.path()).unwrap();
        cache.put_parse(key(1), &Ok(PreExpr::Number { value: 2, ty: None }));
        drop(cache);

        let cache = DiskCache::open(dir.path()).unwrap();
        let got = cache.get_parse(key(1)).unwrap();
        assert!(matches!(got, Ok(PreExpr::Number { value: 1, .. })), "the first write must win");
    }

    /// A format or schema mismatch wipes the cache on open: old entries are
    /// unreachable (schema) or undecodable (format) garbage either way.
    #[test]
    fn version_mismatch_wipes_on_open() {
        let dir = TempDir::new().unwrap();
        let old = DiskCache::open_with_stamp(
            dir.path(),
            FormatStamp { format: CACHE_FORMAT_VERSION, schema: SCHEMA_VERSION - 1 },
        ).unwrap();
        old.put_parse(key(1), &Ok(PreExpr::Number { value: 42, ty: None }));
        drop(old);

        let cache = DiskCache::open(dir.path()).unwrap();
        assert!(cache.get_parse(key(1)).is_none(), "mismatched-version entries must be wiped");

        // And the stamp is updated: a second normal open keeps entries.
        cache.put_parse(key(2), &Ok(PreExpr::Number { value: 7, ty: None }));
        drop(cache);
        let cache = DiskCache::open(dir.path()).unwrap();
        assert!(cache.get_parse(key(2)).is_some(), "same-version reopen must not wipe");
    }

    /// Garbage bytes under a key are a miss, not a panic — the recompute's
    /// write-through would replace them (here: is blocked by NO_OVERWRITE,
    /// which is fine; the memory tier serves the session).
    #[test]
    fn corrupt_bytes_are_a_miss() {
        let dir = TempDir::new().unwrap();
        let cache = DiskCache::open(dir.path()).unwrap();
        // Forge garbage directly, bypassing the typed puts.
        let mut wtxn = cache.env.write_txn().unwrap();
        cache.parse.put(&mut wtxn, &key(1).to_be_bytes(), &[0xFF, 0x13, 0x37]).unwrap();
        cache.resolve.put(&mut wtxn, &key(2).to_be_bytes(), &[0xFF]).unwrap();
        wtxn.commit().unwrap();

        assert!(cache.get_parse(key(1)).is_none());
        assert!(cache.get_resolve(key(2), &Interner::new()).is_none());
    }
}

/// Decode helper: corruption is a miss with a warning, never a panic.
fn decode_or_miss<'a, T: serde::Deserialize<'a>>(bytes: &'a [u8], table: &str, key: ContentKey) -> Option<T> {
    match postcard::from_bytes(bytes) {
        Ok(value) => Some(value),
        Err(e) => {
            warn!("corrupt {} cache entry under {:?} ({}): treating as miss", table, key, e);
            None
        }
    }
}
