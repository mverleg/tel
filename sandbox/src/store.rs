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

use crate::common::FQ;
use crate::graph::StepId;
use crate::keys::{ContentDigest, ContentKey, Fingerprint};
use crate::types::{Expr, FuncData, MonoFuncData, ParseError, PreExpr, ResolveError, SymbolTable, TypeError};
use async_lazy::Cache;
use dashmap::DashMap;

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
/// re-hashing). `None` for errors — error fingerprints arrive with early
/// cutoff.
#[derive(Debug, Clone)]
pub struct ResolveEntry {
    pub answer: Result<ResolveAnswer, ResolveError>,
    pub fingerprint: Option<Fingerprint>,
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
}

impl ContentStore {
    pub fn new() -> ContentStore {
        ContentStore {
            parse: Cache::new(),
            resolve: DashMap::new(),
            mono: DashMap::new(),
        }
    }

    /// Get the parse answer for `key`, computing it with `init` on first
    /// demand. Concurrent callers of the same key await one computation.
    pub async fn parse_get<F, Fut>(&self, key: ContentKey, init: F) -> &Result<PreExpr, ParseError>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<PreExpr, ParseError>>,
    {
        self.parse.get(key, init).await
    }

    pub fn resolve_get(&self, key: &ContentKey) -> Option<ResolveEntry> {
        self.resolve.get(key).map(|hit| hit.clone())
    }

    /// Insert a resolve entry. Append-only: if the key is already present the
    /// existing entry wins and is returned (see [`ContentStore::mono_insert`]).
    pub fn resolve_insert(&self, key: ContentKey, entry: ResolveEntry) -> ResolveEntry {
        self.resolve.entry(key).or_insert(entry).clone()
    }

    pub fn mono_get(&self, key: &ContentKey) -> Option<MonoAnswer> {
        self.mono.get(key).map(|hit| hit.clone())
    }

    /// Insert a mono answer. Append-only: if the key is already present the
    /// existing entry wins and is returned — an entry, once written, never
    /// changes (both values are pure functions of the key, so a race writes
    /// equal answers; keeping the first upholds the invariant cheaply).
    pub fn mono_insert(&self, key: ContentKey, answer: MonoAnswer) -> MonoAnswer {
        self.mono.entry(key).or_insert(answer).clone()
    }

    /// Number of distinct parse answers stored (by content key).
    pub fn parse_len(&self) -> usize {
        self.parse.len()
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
    /// Fingerprint of the step's last output; `None` if the last answer was
    /// an error (error fingerprints arrive with early cutoff).
    pub fingerprint: Option<Fingerprint>,
    /// For leaf (parse) steps: the source digest the current key was built
    /// from — the recorded `input_state`. Later stages chain their keys to
    /// the content that this position actually consumed, and recovery
    /// (invariant 10) compares current leaf digests against this.
    pub input_digest: Option<ContentDigest>,
    /// Push-invalidation dirty bit. Placeholder: nothing sets it yet; the
    /// two-pass marking protocol (docs/execution-and-recovery.md) lands
    /// separately.
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

    /// True if `step` is already bound to `key` with a fingerprint — lets a
    /// re-demand of an unchanged step skip re-fingerprinting its output.
    pub fn is_current(&self, step: &StepId, key: ContentKey) -> bool {
        self.get(step)
            .map(|r| r.content_key == Some(key) && r.fingerprint.is_some())
            .unwrap_or(false)
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

        let first = store.mono_insert(key(1), mono_answer(&interner, "f", 42));
        let second = store.mono_insert(key(1), mono_answer(&interner, "f", 99));

        let fp = |a: &MonoAnswer| {
            let ctx = StableCtx { interner: &interner };
            Fingerprint::of(&a.as_ref().unwrap().0, &ctx)
        };
        assert_eq!(fp(&second), fp(&first), "second insert must return the first answer");
        assert_eq!(fp(&store.mono_get(&key(1)).unwrap()), fp(&first));
        assert_eq!(store.mono_len(), 1);
    }

    /// Deterministic errors are first-class answers in the content store.
    #[test]
    fn content_store_caches_errors() {
        let store = ContentStore::new();
        let err: MonoAnswer = Err(TypeError::FunctionNotResolved { context: "f".to_string() });
        assert!(store.mono_insert(key(2), err).is_err());
        assert!(store.mono_get(&key(2)).unwrap().is_err());
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
            fingerprint: None,
            input_digest: None,
            dirty: false,
        });
        // Replacing the binding swaps the whole record: no field of the old
        // one survives to mix with the new (invariant 4).
        bindings.record(step, BindingRecord {
            content_key: Some(key(2)),
            fingerprint: None,
            input_digest: None,
            dirty: false,
        });
        let got = bindings.get(&step).unwrap();
        assert_eq!(got.content_key, Some(key(2)));
        assert!(got.fingerprint.is_none());
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
            fingerprint: None,
            input_digest: Some(digest),
            dirty: false,
        });
        assert_eq!(bindings.leaf_digest(&step), Some(digest));
    }
}
