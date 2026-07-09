//! The two cache layers from docs/keys-and-invalidation.md ("Where Each Piece
//! Lives") and docs/cache-invalidation-problem.md ("Two layers — only one is
//! fragile"):
//!
//! - [`ContentStore`] — immutable, append-only `ContentKey -> result` tables.
//!   Never invalidated: an entry under a content key is valid forever by
//!   construction (if any input changed, the key would be different), so this
//!   layer is panic- and error-safe — it cannot be corrupted into returning a
//!   wrong answer. Deterministic *errors* are terminal answers and live here
//!   too. Superseded entries are garbage, not hazards.
//! - [`BindingLayer`] — the mutable session memo, keyed by logical id
//!   ([`StepId`]): which content key a query position currently resolves to,
//!   the fingerprint of its last output, and (later) its dirty state. This is
//!   the only state that can be left torn, and everything about invalidation
//!   operates on it exclusively. Dependency edges (both directions) are the
//!   other session-scoped piece; they live in [`crate::graph::Graph`].
//!
//! Per-phase keyspaces get typed, separate tables (the answer type is fixed
//! per kind — no dynamic downcasts, and each kind can later grow its own
//! eviction policy and disk layout).

use std::sync::Arc;
use crate::common::{Interner, FQ};
use crate::disk::DiskCache;
use crate::graph::StepId;
use crate::keys::{ContentDigest, ContentKey, Fingerprint};
use crate::types::{Expr, FuncData, MonoFuncData, ParseError, PreExpr, ResolveError, SpanTable, SymbolTable, TypeError};
use async_lazy::Cache;
use dashmap::DashMap;
use dashmap::mapref::entry::Entry;

/// A monomorphisation answer: the checked instance plus the callee instances
/// it needs (stored so the worklist can keep walking on a cache hit).
pub type MonoAnswer = Result<(MonoFuncData, Vec<crate::graph::MonoId>), TypeError>;

/// A successful resolution of one file-level unit: the resolved body, its
/// symbol table, and — because resolving also *defines* things — the functions
/// this file registered (its implicit file-function plus local functions), in
/// registration order. The definitions ride along in the answer so a cache hit
/// can replay them into the positional registry without running the resolver.
#[derive(Debug, Clone)]
pub struct ResolveAnswer {
    pub ast: Expr,
    pub table: SymbolTable,
    pub funcs: Vec<(FQ, FuncData)>,
}

/// One stored resolve row: the answer (deterministic errors included) plus
/// the answer's result fingerprint (docs/keys-and-invalidation.md stores the
/// fingerprint *with* the value so dependents can key on it without
/// re-hashing). Errors are fingerprinted too ([`Fingerprint::of_err`]): a
/// stably-erroring answer is a terminal answer, and its dependents key on it
/// like any other.
#[derive(Debug, Clone)]
pub struct ResolveEntry {
    pub answer: Result<ResolveAnswer, ResolveError>,
    pub fingerprint: Fingerprint,
}

/// The immutable content-addressed layer: one append-only table per query
/// kind that has cacheable answers today. Entries are never mutated or
/// removed within a process.
pub struct ContentStore {
    /// Parse answers. Behind an `async-lazy` [`Cache`] because parse is
    /// async (read stays fused with parse) and concurrent demands for the
    /// same key must claim/await a single computation — the cache is
    /// init-once per key, which *is* the append-only guarantee.
    parse: Cache<ContentKey, PreExpr, ParseError>,
    /// Resolve answers. Keyed on `hash(fq, parse fingerprint, import resolve
    /// fingerprints)` — Merkle-over-answers, so a hit is transitively valid by
    /// construction. First-write-wins like `mono` (concurrent duplicate
    /// resolutions of the same key compute equal answers; keeping the first
    /// upholds append-only cheaply).
    resolve: DashMap<ContentKey, ResolveEntry>,
    /// Mono answers. The mono worklist is synchronous, so a plain map with
    /// first-write-wins semantics suffices; deterministic `TypeError`s are
    /// answers and are stored like successes.
    mono: DashMap<ContentKey, MonoAnswer>,
    /// Parse **span sidecars** (plans/fast-mode.md), keyed on the same source
    /// digest as the core parse answer but in the `Spans` keyspace. Memory-only
    /// and *not* written through to disk: a span table is recomputable from
    /// bytes at leaf-query cost, so it is cheaper to rebuild than to persist,
    /// and it feeds no downstream key — losing it can never serve a stale
    /// answer. `Arc` so a lookup hands out a cheap clone.
    spans: DashMap<ContentKey, Arc<SpanTable>>,
    /// Optional persistent tier (src/disk.rs). Read order is always memory →
    /// disk → compute; every fresh compute (and only the *winning* insert of
    /// a race) is written through. `None` is the cold-and-hermetic default —
    /// the `--no-daemon` contract of plans/daemon.md.
    disk: Option<Arc<DiskCache>>,
}

impl ContentStore {
    pub fn new() -> ContentStore {
        ContentStore {
            parse: Cache::new(),
            resolve: DashMap::new(),
            mono: DashMap::new(),
            spans: DashMap::new(),
            disk: None,
        }
    }

    /// A store whose entries also live in (and are revived from) `disk`.
    pub fn with_disk(disk: Arc<DiskCache>) -> ContentStore {
        ContentStore { disk: Some(disk), ..ContentStore::new() }
    }

    /// Get the parse answer for `key`, computing it with `init` on first
    /// demand. Concurrent callers of the same key await one computation.
    ///
    /// The disk probe lives *inside* the single-flight init closure: a disk
    /// hit simply becomes the cache entry — no insert API needed on the
    /// by-reference `async-lazy` cache — and, because the caller's `init`
    /// (which counts computed parses) never ran, counters stay honest: a
    /// disk hit is not a computed parse. Everything reaching this cache is a
    /// terminal answer (IO failures return earlier, in `parse_impl`), so the
    /// unconditional write-through on compute is sound.
    pub async fn parse_get<F, Fut>(&self, key: ContentKey, init: F) -> &Result<PreExpr, ParseError>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<PreExpr, ParseError>>,
    {
        self.parse.get(key, || async move {
            if let Some(disk) = &self.disk {
                if let Some(hit) = disk.get_parse(key) {
                    return hit;
                }
            }
            let answer = init().await;
            if let Some(disk) = &self.disk {
                disk.put_parse(key, &answer);
            }
            answer
        }).await
    }

    pub fn resolve_get(&self, key: &ContentKey, interner: &Interner) -> Option<ResolveEntry> {
        if let Some(hit) = self.resolve.get(key) {
            return Some(hit.clone());
        }
        let revived = self.disk.as_ref()?.get_resolve(*key, interner)?;
        // Populate memory; if a concurrent compute won the race meanwhile,
        // keep it (equal by determinism — first write wins).
        Some(self.resolve.entry(*key).or_insert(revived).clone())
    }

    /// Insert a resolve entry. Append-only: if the key is already present the
    /// existing entry wins and is returned (see [`ContentStore::mono_insert`]).
    /// Only the winning insert reaches disk — losers of the memory race never
    /// enqueue, extending first-write-wins through the persistent tier.
    pub fn resolve_insert(&self, key: ContentKey, entry: ResolveEntry, interner: &Interner) -> ResolveEntry {
        match self.resolve.entry(key) {
            Entry::Occupied(existing) => existing.get().clone(),
            Entry::Vacant(slot) => {
                if let Some(disk) = &self.disk {
                    disk.put_resolve(key, &entry, interner);
                }
                slot.insert(entry).clone()
            }
        }
    }

    pub fn mono_get(&self, key: &ContentKey, interner: &Interner) -> Option<MonoAnswer> {
        if let Some(hit) = self.mono.get(key) {
            return Some(hit.clone());
        }
        let revived = self.disk.as_ref()?.get_mono(*key, interner)?;
        Some(self.mono.entry(*key).or_insert(revived).clone())
    }

    /// Insert a mono answer. Append-only: if the key is already present the
    /// existing entry wins and is returned — an entry, once written, never
    /// changes (both values are pure functions of the key, so a race writes
    /// equal answers; keeping the first upholds the invariant cheaply). Disk
    /// write-through from the winning insert only, like
    /// [`ContentStore::resolve_insert`].
    pub fn mono_insert(&self, key: ContentKey, answer: MonoAnswer, interner: &Interner) -> MonoAnswer {
        match self.mono.entry(key) {
            Entry::Occupied(existing) => existing.get().clone(),
            Entry::Vacant(slot) => {
                if let Some(disk) = &self.disk {
                    disk.put_mono(key, &answer, interner);
                }
                slot.insert(answer).clone()
            }
        }
    }

    /// Get the span sidecar for `key`, building it with `build` on first
    /// demand. Memory-only and idempotent — the builder is a pure function of
    /// the bytes `key` hashes, so a race merely recomputes an equal table and
    /// first-write-wins keeps one.
    pub fn spans_get_or_build(&self, key: ContentKey, build: impl FnOnce() -> SpanTable) -> Arc<SpanTable> {
        if let Some(hit) = self.spans.get(&key) {
            return hit.clone();
        }
        self.spans.entry(key).or_insert_with(|| Arc::new(build())).clone()
    }

    /// Number of distinct parse answers stored (by content key).
    pub fn parse_len(&self) -> usize {
        self.parse.len()
    }

    /// Number of span sidecars built and cached (by content key).
    pub fn spans_len(&self) -> usize {
        self.spans.len()
    }

    /// Number of distinct resolve answers stored (by content key).
    pub fn resolve_len(&self) -> usize {
        self.resolve.len()
    }

    /// Number of distinct mono answers stored (by content key).
    pub fn mono_len(&self) -> usize {
        self.mono.len()
    }
}

/// One session-memo record: what the logical id currently binds to.
///
/// Updated only as a whole (invariant 4 of docs/keys-and-invalidation.md:
/// memo updates are atomic per query — key and fingerprint together), via a
/// single [`BindingLayer::record`] insert.
#[derive(Debug, Clone, Copy)]
pub struct BindingRecord {
    /// Content key this position currently resolves to. `None` for steps
    /// whose key derivation does not exist yet (resolve/exec get content
    /// keys built from dep fingerprints in a later step).
    pub content_key: Option<ContentKey>,
    /// Fingerprint of the step's last *answer* — success or deterministic
    /// error, tagged apart by [`Fingerprint::of_ok`]/[`of_err`]. Always
    /// recorded with the key (invariant 4: memo updates are atomic per
    /// query); a step whose attempt ended non-terminally (transient IO,
    /// cancelled/joined task, cycle) is never bound at all.
    pub fingerprint: Fingerprint,
    /// For leaf (parse) steps: the source digest the current key was built
    /// from — the recorded `input_state`. Later stages chain their keys to
    /// the content that this position actually consumed, and recovery
    /// (invariant 10) compares current leaf digests against this.
    pub input_digest: Option<ContentDigest>,
    /// Push-invalidation dirty bit — pass 1 of the two-pass protocol
    /// (docs/execution-and-recovery.md) sets it via
    /// [`BindingLayer::mark_dirty`]; pass 2 clears it only as part of a
    /// successful whole-record commit (invariant 8: never at scheduling
    /// time).
    ///
    /// Of the node states the design doc prescribes (`Unknown / Dirty /
    /// Pending(owner, wakers) / Verified(key, fp)`), the reachable ones map
    /// onto this layer as: no record = `Unknown`, `dirty: true` = `Dirty`
    /// (the record's key/fingerprint are the memo the red-green comparison
    /// runs against), `dirty: false` = `Verified`. `Pending` is deliberately
    /// not represented here: single-flight for the async leaf lives inside
    /// the `async-lazy` parse cache, runs on one compiler are serialized
    /// (`Compiler::run` takes `&mut self`), and duplicate computation of a
    /// derived step within one wave is benign by determinism (first-write-wins
    /// in the content store) — so there is no waker machinery to build until
    /// concurrent waves exist.
    pub dirty: bool,
}

/// The mutable binding layer: `logical id -> current binding`.
pub struct BindingLayer {
    records: DashMap<StepId, BindingRecord>,
}

impl BindingLayer {
    pub fn new() -> BindingLayer {
        BindingLayer {
            records: DashMap::new(),
        }
    }

    /// Replace the binding of `step` in one atomic insert.
    pub fn record(&self, step: StepId, record: BindingRecord) {
        self.records.insert(step, record);
    }

    pub fn get(&self, step: &StepId) -> Option<BindingRecord> {
        self.records.get(step).map(|r| *r)
    }

    /// The recorded source digest of a leaf (parse) step, if it has one.
    pub fn leaf_digest(&self, step: &StepId) -> Option<ContentDigest> {
        self.get(step).and_then(|r| r.input_digest)
    }

    /// True if `step` is already bound to `key` *and verified clean* — lets a
    /// re-demand of an unchanged step skip re-fingerprinting its output. A
    /// dirty record is never "current" even under the same key: the caller
    /// must re-commit the whole record so cleaning happens atomically with
    /// the commit (invariant 8), never as a separate flag flip.
    pub fn is_current(&self, step: &StepId, key: ContentKey) -> bool {
        self.get(step)
            .map(|r| !r.dirty && r.content_key == Some(key))
            .unwrap_or(false)
    }

    /// Pass-1 marking: flip `step` to dirty. A bit flip only — no hashing, no
    /// IO, infallible (docs/keys-and-invalidation.md "Invalidation From the
    /// Leafs"). A step with no record is `Unknown` and needs no flip: it has
    /// no memoized key that could be wrongly trusted.
    pub fn mark_dirty(&self, step: &StepId) {
        if let Some(mut r) = self.records.get_mut(step) {
            r.dirty = true;
        }
    }

    /// True if `step` has a record that is currently marked dirty.
    pub fn is_dirty(&self, step: &StepId) -> bool {
        self.get(step).map(|r| r.dirty).unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::{Interner, FQ};
    use crate::graph::{MonoId, ParseId, ResolveId};
    use crate::keys::{QueryKind, StableCtx};
    use crate::types::{MExpr, Ty, Value};

    fn key(n: u64) -> ContentKey {
        ContentKey::build(QueryKind::Mono, |h| h.write_u64(n))
    }

    fn fp(interner: &Interner, n: u64) -> Fingerprint {
        Fingerprint::of(&n, &StableCtx { interner })
    }

    fn mono_answer(interner: &Interner, name: &str, v: i64) -> MonoAnswer {
        let id = MonoId { func_loc: FQ::intern(interner, "f.telsb", name), ty: Ty::I64 };
        Ok((
            MonoFuncData { key: id, arity: 0, ast: MExpr::Number(Value::I64(v)) },
            Vec::new(),
        ))
    }

    /// The append-only invariant: a second write to an existing key is
    /// discarded; the first answer stands forever.
    #[test]
    fn content_store_never_overwrites() {
        let interner = Interner::new();
        let store = ContentStore::new();

        let first = store.mono_insert(key(1), mono_answer(&interner, "f", 42), &interner);
        let second = store.mono_insert(key(1), mono_answer(&interner, "f", 99), &interner);

        let fp = |a: &MonoAnswer| {
            let ctx = StableCtx { interner: &interner };
            Fingerprint::of(&a.as_ref().unwrap().0, &ctx)
        };
        assert_eq!(fp(&second), fp(&first), "second insert must return the first answer");
        assert_eq!(fp(&store.mono_get(&key(1), &interner).unwrap()), fp(&first));
        assert_eq!(store.mono_len(), 1);
    }

    /// Deterministic errors are first-class answers in the content store.
    #[test]
    fn content_store_caches_errors() {
        let interner = Interner::new();
        let store = ContentStore::new();
        let err: MonoAnswer = Err(TypeError::FunctionNotResolved { context: "f".to_string() });
        assert!(store.mono_insert(key(2), err, &interner).is_err());
        assert!(store.mono_get(&key(2), &interner).unwrap().is_err());
        assert_eq!(store.mono_len(), 1);
    }

    #[test]
    fn binding_updates_are_whole_record() {
        let interner = Interner::new();
        let bindings = BindingLayer::new();
        let step = StepId::Resolve(ResolveId { func_loc: FQ::intern(&interner, "f.telsb", "f") });

        assert!(bindings.get(&step).is_none());
        bindings.record(step, BindingRecord {
            content_key: Some(key(1)),
            fingerprint: fp(&interner, 1),
            input_digest: Some(ContentDigest::of("stale")),
            dirty: false,
        });
        // Replacing the binding swaps the whole record: no field of the old
        // one survives to mix with the new (invariant 4).
        bindings.record(step, BindingRecord {
            content_key: Some(key(2)),
            fingerprint: fp(&interner, 2),
            input_digest: None,
            dirty: false,
        });
        let got = bindings.get(&step).unwrap();
        assert_eq!(got.content_key, Some(key(2)));
        assert_eq!(got.fingerprint, fp(&interner, 2));
        assert!(got.input_digest.is_none(), "no field of the old record may survive");
    }

    #[test]
    fn leaf_digest_comes_from_the_binding() {
        let interner = Interner::new();
        let bindings = BindingLayer::new();
        let step = StepId::Parse(ParseId { file_path: crate::common::Path::intern(&interner, "f.telsb") });

        assert!(bindings.leaf_digest(&step).is_none());
        let digest = ContentDigest::of("(print 42)");
        bindings.record(step, BindingRecord {
            content_key: Some(key(3)),
            fingerprint: fp(&interner, 3),
            input_digest: Some(digest),
            dirty: false,
        });
        assert_eq!(bindings.leaf_digest(&step), Some(digest));
    }
}
