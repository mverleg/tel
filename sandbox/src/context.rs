use crate::common::{Ctx, Interner, Path, FQ};
use crate::graph::{ExecId, Graph, MonoId, ParseId, ResolveId, StepId};
use crate::keys::{ContentDigest, ContentKey, Fingerprint, QueryKind, StableCtx, StableHash};
use crate::store::{BindingLayer, BindingRecord, ContentStore, MonoAnswer, ResolveAnswer, ResolveEntry};
use crate::types::{ExecuteError, Expr, FuncData, FuncId, Located, MonoFuncData, ParseError, PreExpr, ResolveError, SpanTable, SymbolTable, Ty, TypeError};
use crate::Printer;
use dashmap::DashMap;
use log::debug;
use std::collections::HashSet;
use std::future::Future;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// Immutable chain of resolutions in progress on this task's path, root-most
/// first (docs/cycle-detection.md). Each child resolution extends the chain by
/// one; the `Arc` shares the prefix, so extending is cheap and parallel
/// branches never see each other's chain — which is what keeps a diamond
/// (shared dependency, different chains) from false-positiving as a cycle.
#[derive(Clone)]
pub struct AncestorPath(Arc<Vec<FQ>>);

impl AncestorPath {
    pub fn empty() -> AncestorPath {
        AncestorPath(Arc::new(Vec::new()))
    }

    pub fn contains(&self, fq: FQ) -> bool {
        self.0.contains(&fq)
    }

    /// New path with `fq` appended; the receiver is unchanged.
    pub fn extended(&self, fq: FQ) -> AncestorPath {
        let mut chain = Vec::with_capacity(self.0.len() + 1);
        chain.extend_from_slice(&self.0);
        chain.push(fq);
        AncestorPath(Arc::new(chain))
    }

    /// The offending chain as human-readable `path::name` strings: from the
    /// first occurrence of `repeat`, through the tail, closed by `repeat`.
    pub fn cycle_strings(&self, repeat: FQ, interner: &Interner) -> Vec<String> {
        let fq_str = |fq: &FQ| format!("{}::{}", fq.path_str(interner), fq.name_str(interner));
        let start = self.0.iter().position(|fq| *fq == repeat).unwrap_or(0);
        let mut cycle: Vec<String> = self.0[start..].iter().map(fq_str).collect();
        cycle.push(fq_str(&repeat));
        cycle
    }
}

/// How a pull decides whether a memoized binding can be trusted — the two
/// reconciliation stances of docs/execution-and-recovery.md ("When
/// `reconcile()` runs, by mode").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PullMode {
    /// Batch stance: never trust events; every demanded leaf is re-read and
    /// re-digested (leaf digests are ground truth), and every demanded step
    /// re-derives its content key. Always correct with no invalidation state
    /// at all — this is what [`crate::Compiler::run`] does.
    Reconcile,
    /// Watch stance: trust that every change was announced via
    /// [`crate::Compiler::invalidate`]. A *clean* (`Verified`) binding is
    /// served from its memoized content key with zero recursion — its leaf is
    /// not even re-read — while dirty nodes re-derive through the normal pull
    /// path (pass 2 of the two-pass protocol). This is the entire payoff of
    /// push invalidation: untouched subgraphs cost nothing per wave.
    TrustClean,
}

/// The shared core of one persistent compiler: every cache layer and the
/// session memos live here, behind an `Arc` so spawned tasks and long-lived
/// [`crate::Compiler`]s share it without leaking (the old `Box::leak`
/// approach is gone — dropping the last handle reclaims everything).
pub struct Global {
    graph: Graph,
    interner: Interner,
    /// The immutable content-addressed layer (docs/keys-and-invalidation.md):
    /// append-only `content key -> answer` tables for parse, resolve, and
    /// mono. A key always maps to the same answer, or is absent (recompute);
    /// invalidation is implicit — changed input, different key. Parse keys
    /// hash the source byte digest (parse is the leaf query — read stays
    /// fused with parse — so the file *path* is the logical id, not a key
    /// ingredient); resolve and mono keys chain to their upstream steps via
    /// the *answer fingerprints* of their direct dependencies, one hop at a
    /// time — a Merkle DAG over answers, which is what makes a hit
    /// transitively valid by construction.
    store: ContentStore,
    /// The mutable session memo (`logical id -> current binding`): which
    /// content key each step position currently resolves to, its last output
    /// fingerprint, and the recorded leaf digests the mono phase chains its
    /// keys to. The only cache state that is ever rebound.
    bindings: BindingLayer,
    func_registry: DashMap<FQ, FuncData>,
    /// Which file-level resolve step *defined* each function — recorded at
    /// resolve commit. The mono phase depends on the resolved data of one
    /// function, and for reverse-edge cone marking that dependency must point
    /// at the real resolve step (the file-level unit), not at a phantom
    /// `Resolve(function FQ)` node that no resolution ever binds. Session
    /// memo, like the graph edges.
    definer: DashMap<FQ, ResolveId>,
    /// Monomorphised instances of the *current* run, keyed by function +
    /// numeric type. This is the positional view the interpreter executes
    /// from; it is rebuilt per run (from content-store hits where possible).
    mono_registry: DashMap<MonoId, MonoFuncData>,
    /// Source files actually read and digested (the input probe of the pull).
    /// In watch mode a clean subtree skips even this.
    leaf_reads: AtomicUsize,
    /// Parse computations actually executed (content-store misses).
    computed_parses: AtomicUsize,
    /// Resolve computations actually executed (content-store misses).
    computed_resolves: AtomicUsize,
    /// Mono checks actually executed (content-store misses).
    computed_monos: AtomicUsize,
    /// Test support: when set, resolving a file whose path contains this
    /// needle panics at the recompute boundary. The least invasive way to
    /// exercise the panic-safety path (`catch_unwind` + node-stays-dirty)
    /// from integration tests; never set in production use.
    panic_on_resolve: Mutex<Option<String>>,
    /// Debug step tracer (feature `step-trace`): records each step's logical
    /// id, key hash, cache outcome, entry age, and timing to a JSONL file.
    /// A zero-sized no-op when the feature is off — see `src/trace.rs`.
    trace: crate::trace::StepTrace,
    /// Ambient flavor environment of this compile (roadmap item 15,
    /// src/flavors.rs). Read by the backend-analog (`execute`); no cached
    /// query depends on it, so it never enters a content key — Option C, no
    /// fragmentation.
    flavors: crate::flavors::Flavors,
    printer: Arc<dyn Printer>,
}

/// Render a caught panic payload for the non-terminal `Panicked` variants.
fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "non-string panic payload".to_string()
    }
}

/// One resolve outcome: the answer triple, or the failure paired with the
/// failure's *answer fingerprint* when it is a terminal (cached) answer.
/// `None` on the error side marks a non-terminal failure — a cycle, a task
/// join failure, transient IO — which is not an answer: nothing above it may
/// be keyed or cached (invariant 6 of docs/keys-and-invalidation.md).
type ResolveOutcome = Result<(Expr, SymbolTable, Fingerprint), (Located<ResolveError>, Option<Fingerprint>)>;

/// Content-key *preimage* of one monomorphised instance: the instance identity
/// plus the fingerprint of the one thing `check_function` consumes — the
/// resolved [`FuncData`] of the instance's function. Hashed (via `StableHash`,
/// so the FQ enters as strings, not interner indices) into the [`ContentKey`]
/// the mono store is keyed by.
///
/// Chaining to the *function's* resolved data (rather than the file's source
/// digest) buys function-level cutoff: an edit that leaves this function's
/// resolved AST unchanged — whitespace anywhere, or a change to a *different*
/// function in the same file — leaves this instance's key unchanged, so it is
/// a pure hit. The FQ stays in the preimage because the monomorphised AST
/// embeds path-based FQs; `is_entry` participates because the entry point is
/// checked with a relaxed return type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MonoCacheKey {
    func_fp: Fingerprint,
    func_loc: FQ,
    ty: Ty,
    is_entry: bool,
}

impl MonoCacheKey {
    fn content_key(&self, ctx: &StableCtx<'_>) -> ContentKey {
        ContentKey::build(QueryKind::Mono, |h| {
            self.func_fp.stable_hash(ctx, h);
            self.func_loc.stable_hash(ctx, h);
            self.ty.stable_hash(ctx, h);
            self.is_entry.stable_hash(ctx, h);
        })
    }
}

impl Global {
    pub fn new(printer: Arc<dyn Printer>, flavors: crate::flavors::Flavors) -> Self {
        Global::with_store(printer, ContentStore::new(), flavors)
    }

    /// A `Global` whose content store also lives in (and is revived from)
    /// the persistent tier at `cache_dir`. Only the content store persists;
    /// the binding layer, graph, definer, and mono registry stay per-process
    /// session state. Uses the default flavors — the daemon's opt-level is
    /// [`OptLevel::Debug`], and flavors touch no persisted key anyway.
    pub fn with_disk_cache(printer: Arc<dyn Printer>, cache_dir: &std::path::Path) -> Result<Self, crate::disk::DiskCacheError> {
        let disk = Arc::new(crate::disk::DiskCache::open(cache_dir)?);
        Ok(Global::with_store(printer, ContentStore::with_disk(disk), crate::flavors::Flavors::default()))
    }

    fn with_store(printer: Arc<dyn Printer>, store: ContentStore, flavors: crate::flavors::Flavors) -> Self {
        Global {
            graph: Graph::new(),
            interner: Interner::new(),
            store,
            bindings: BindingLayer::new(),
            func_registry: DashMap::new(),
            definer: DashMap::new(),
            mono_registry: DashMap::new(),
            leaf_reads: AtomicUsize::new(0),
            computed_parses: AtomicUsize::new(0),
            computed_resolves: AtomicUsize::new(0),
            computed_monos: AtomicUsize::new(0),
            panic_on_resolve: Mutex::new(None),
            trace: crate::trace::StepTrace::new(),
            flavors,
            printer,
        }
    }

    pub fn flavors(&self) -> crate::flavors::Flavors {
        self.flavors
    }

    /// Total recorded bytes across every content-store table — what the
    /// admission-control gate compares against the byte budget
    /// (plans/concurrency-and-eviction.md Decision 3).
    pub fn cache_bytes(&self) -> u64 {
        self.store.cache_bytes()
    }

    /// Compact the content store to `budget_bytes` (size-aware LRU eviction,
    /// Decision 2 + 6). `&mut self`: reachable only between waves, when the
    /// `Compiler` is the sole owner and `Arc::get_mut` yields exclusive access —
    /// which is exactly when no borrow into the tables is live.
    pub fn compact(&mut self, budget_bytes: u64) {
        self.store.compact(budget_bytes);
    }

    /// Number of distinct source contents parsed and cached so far (by digest).
    /// Lets callers observe cross-run cache reuse: an unchanged or reverted file
    /// does not grow this count on a subsequent run.
    pub fn cached_parse_count(&self) -> usize {
        self.store.parse_len()
    }

    /// Number of parse span sidecars built and cached so far. Stays `0` across
    /// a run with no diagnostics — the happy path never demands one — so a test
    /// can assert the sidecar is computed only on the error/detail path.
    pub fn cached_spans_count(&self) -> usize {
        self.store.spans_len()
    }

    /// Number of distinct monomorphised instances cached so far (by resolved
    /// function fingerprint + instance). Grows only when an instance is
    /// checked against resolved content it has not seen before.
    pub fn cached_mono_count(&self) -> usize {
        self.store.mono_len()
    }

    /// Number of distinct resolve answers cached so far (by content key).
    pub fn cached_resolve_count(&self) -> usize {
        self.store.resolve_len()
    }

    /// Number of parse computations actually executed so far — unlike
    /// [`cached_parse_count`](Global::cached_parse_count) this counts work
    /// done, not entries: a cache hit (including a cached *error*) does not
    /// grow it.
    pub fn computed_parse_count(&self) -> usize {
        self.computed_parses.load(Ordering::Relaxed)
    }

    /// Number of resolve computations actually executed so far (cache hits —
    /// including cached *errors* — excluded).
    pub fn computed_resolve_count(&self) -> usize {
        self.computed_resolves.load(Ordering::Relaxed)
    }

    /// Number of mono checks actually executed so far (cache hits excluded).
    pub fn computed_mono_count(&self) -> usize {
        self.computed_monos.load(Ordering::Relaxed)
    }

    /// Number of source files read and digested so far (every demanded leaf
    /// in [`PullMode::Reconcile`]; only dirty cones in
    /// [`PullMode::TrustClean`]).
    pub fn leaf_read_count(&self) -> usize {
        self.leaf_reads.load(Ordering::Relaxed)
    }

    /// Test support — see the `panic_on_resolve` field.
    pub fn set_panic_on_resolve(&self, needle: Option<&str>) {
        *self.panic_on_resolve.lock().unwrap() = needle.map(String::from);
    }

    /// Pass 1 of two-pass invalidation (docs/execution-and-recovery.md,
    /// docs/cache-invalidation-problem.md #7): mark the changed leaf and its
    /// whole reverse-edge cone dirty. Bit flips only — infallible, no
    /// hashing, no IO — so it can never half-fail into a falsely-clean state;
    /// recompute (pass 2) happens lazily on the next pull and clears dirty
    /// only on successful commit.
    ///
    /// A path never seen just interns a fresh leaf with no record and no
    /// dependents: harmless. Over-marking through stale edges of dead
    /// subgraphs is inherent and safe (marked nodes nobody pulls are never
    /// executed); under-marking is impossible because edges are only ever
    /// *replaced* with sets re-derived from current content, never dropped.
    pub fn invalidate_path(&self, path: &str) {
        let leaf = StepId::Parse(ParseId { file_path: Path::intern(&self.interner, path) });
        let mut visited = HashSet::new();
        self.mark_cone(leaf, &mut visited);
    }

    /// Post-order marking walk: recurse up the reverse edges first, flip on
    /// the way back, so the leaf flips last. At every intermediate state the
    /// marking invariant (*dirty implies all transitive dependents are
    /// dirty*) holds, which is what makes an interrupted walk resumable and
    /// the stop-at-dirty short-circuit sound (docs/keys-and-invalidation.md
    /// "Marking invariant and resumability").
    fn mark_cone(&self, step: StepId, visited: &mut HashSet<StepId>) {
        if !visited.insert(step) {
            return;
        }
        // Already dirty: by the marking invariant its entire cone is already
        // dirty too — the common IDE case of a file changing again before any
        // compile ran costs O(1).
        if self.bindings.is_dirty(&step) {
            return;
        }
        // Collect before recursing: holding a DashMap read guard across a
        // recursive re-entry of the same map risks deadlock with writers.
        let dependents: Vec<StepId> = self.graph.get_dependents(&step)
            .map(|d| d.iter().copied().collect())
            .unwrap_or_default();
        for dependent in dependents {
            self.mark_cone(dependent, visited);
        }
        self.bindings.mark_dirty(&step);
    }
}

impl Global {
    async fn parse_impl(&self, caller: StepId, id: ParseId, mode: PullMode) -> Result<&PreExpr, ParseError> {
        debug!("CoreContext::parse_impl: {:?}", id);
        let mut span = self.trace.span("parse", || format!("{}", Ctx(&id.file_path, &self.interner)));

        // Always register dependency, regardless of cache hit/miss. The graph
        // node stays keyed on the file *position* (path); the content digest is
        // an implementation detail of the parse cache below.
        self.graph.register_dependency(caller, StepId::Parse(id));

        // Watch stance: a clean leaf binding is trusted outright — no read,
        // no digest. Sound under the watch contract (every change is
        // announced via invalidate, so an unannounced clean leaf really is
        // unchanged); the binding is written strictly after the store insert,
        // so the memoized key always has a stored answer behind it.
        if mode == PullMode::TrustClean {
            if let Some(record) = self.bindings.get(&StepId::Parse(id)) {
                if !record.dirty {
                    if let Some(key) = record.content_key {
                        span.cache_hit(key);
                        span.set_fingerprint(|| Some(record.fingerprint));
                        let result = self.store.parse_get(key, || async {
                            unreachable!("a clean binding implies a stored parse answer")
                        }).await;
                        return result.as_ref().map_err(|e| e.clone());
                    }
                }
            }
        }

        // Read+parse stay fused, but the read happens on every compile so we can
        // compute a content digest and key the cache on it. This saves the parse
        // *computation* (not the read) when the bytes are unchanged, and reuses
        // the result across runs on revert / branch-switch.
        let path = id.file_path.resolve(&self.interner);
        let source = match tokio::fs::read_to_string(path).await {
            Ok(source) => source,
            // An IO failure is transient, not a deterministic answer
            // (invariant 6 of docs/keys-and-invalidation.md): without content
            // there is no digest to key on, so nothing enters the content
            // store or the binding layer.
            Err(err) => return Err(ParseError::from(err)),
        };
        self.leaf_reads.fetch_add(1, Ordering::Relaxed);

        let digest = ContentDigest::of(&source);
        // Parse is the leaf query, so its content key hashes the external
        // input (the byte digest) — schema version and kind tag are folded in
        // by construction.
        let key = ContentKey::build(QueryKind::Parse, |h| {
            digest.stable_hash(&StableCtx { interner: &self.interner }, h);
        });
        // Tracing's hit/miss probe: with single-flight in the parse cache,
        // "our closure ran" is the only reliable miss signal. A no-op unit
        // type unless `step-trace` is on.
        let computed = crate::trace::ComputeFlag::new();
        let computed_flag = &computed;
        let result = self.store.parse_get(key, move || async move {
            debug!("CoreContext::parse_impl parsing {:?} (digest {:?})", path, digest);
            computed_flag.set();
            self.computed_parses.fetch_add(1, Ordering::Relaxed);
            crate::parse::tokenize_and_parse(&source)
        }).await;
        span.record_cache(key, || !computed.is_set());

        // Rebind this position to what its content resolves to now: content
        // key, output fingerprint, and the leaf digest later stages chain
        // their keys to. One whole-record insert (memo updates are atomic per
        // query); skipped when the binding is already current, so an
        // unchanged re-demand does not re-fingerprint the AST.
        let step = StepId::Parse(id);
        if !self.bindings.is_current(&step, key) {
            let ctx = StableCtx { interner: &self.interner };
            // A deterministic parse error is a terminal answer like any
            // other: cached above and fingerprinted (tagged apart from
            // successes), so dependents can key on it.
            let fingerprint = match result {
                Ok(pre) => Fingerprint::of_ok(pre, &ctx),
                Err(e) => Fingerprint::of_err(e, &ctx),
            };
            self.bindings.record(step, BindingRecord {
                content_key: Some(key),
                fingerprint,
                input_digest: Some(digest),
                dirty: false,
            });
        }
        span.set_fingerprint(|| self.bindings.get(&step).map(|r| r.fingerprint));

        // Success borrows from the content store; a cached error is cloned
        // out (errors are small, and an owned error keeps the signature free
        // of store lifetimes).
        result.as_ref().map_err(|e| e.clone())
    }

    async fn resolve_all_impl(this: &Arc<Global>, caller: StepId, ancestors: AncestorPath, ids: &[ResolveId], mode: PullMode) -> Result<(Vec<Expr>, SymbolTable), Located<ResolveError>> {
        let (results, table) = Global::resolve_all_fp(this, caller, ancestors, ids, mode).await?;
        Ok((results.into_iter().map(|(expr, _fp)| expr).collect(), table))
    }

    /// Resolve every id (concurrently for more than one), returning each
    /// answer *with its result fingerprint* — the ingredient a dependent needs
    /// to build its own content key. Fail-fast batch view over
    /// [`Global::resolve_each`]: the first failure (in id order) becomes the
    /// batch's error.
    async fn resolve_all_fp(this: &Arc<Global>, caller: StepId, ancestors: AncestorPath, ids: &[ResolveId], mode: PullMode) -> Result<(Vec<(Expr, Fingerprint)>, SymbolTable), Located<ResolveError>> {
        let outcomes = Global::resolve_each(this, caller, ancestors, ids, mode).await?;

        let mut results = Vec::with_capacity(outcomes.len());
        let mut merged_table = SymbolTable::new();
        for outcome in outcomes {
            let (expr, table, fp) = outcome.map_err(|(e, _fp)| e)?;
            results.push((expr, fp));
            merged_table.vars.extend(table.vars);
            // funcs are now in global registry, no need to merge
        }
        Ok((results, merged_table))
    }

    /// Resolve every id (concurrently for more than one), returning the
    /// per-id outcomes in id order — *without* failing the batch on a single
    /// dep's terminal error. A dependent needs exactly this view: a dep whose
    /// answer is a deterministic error still has an answer fingerprint, so
    /// the dependent's own content key stays derivable.
    ///
    /// The outer `Err` is batch-level and always non-terminal: an import
    /// cycle caught by the pre-flight ancestor check, or a task join failure.
    async fn resolve_each(this: &Arc<Global>, caller: StepId, ancestors: AncestorPath, ids: &[ResolveId], mode: PullMode) -> Result<Vec<ResolveOutcome>, Located<ResolveError>> {
        debug!("CoreContext::resolve_each x{}: {:?}", ids.len(), ids);

        // Deadlock-safe cycle detection (docs/cycle-detection.md): a requested
        // resolution that is already on this task's in-progress ancestor chain
        // closes an import cycle. Checked before any await/spawn, so the
        // wait-for cycle — two parked tasks awaiting each other — never forms.
        for id in ids {
            if ancestors.contains(id.func_loc) {
                return Err(ResolveError::CyclicDependency {
                    cycle: ancestors.cycle_strings(id.func_loc, &this.interner),
                }.into());
            }
        }

        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let n = ids.len();
        if n == 1 {
            let id = ids[0];
            this.graph.register_dependency(caller, StepId::Resolve(id));
            return Ok(vec![Global::resolve_one(this, ancestors, id, mode).await]);
        }

        // Spawn tasks for items 0..N-1; each task owns its own handle to the
        // core, which is what lets the future be 'static without leaking.
        let mut handles = Vec::new();
        for i in 0..n-1 {
            let id = ids[i];
            let ancestors = ancestors.clone();
            let core = this.clone();
            let handle = tokio::spawn(async move {
                core.graph.register_dependency(caller, StepId::Resolve(id));
                Global::resolve_one(&core, ancestors, id, mode).await
            });
            handles.push(handle);
        }

        // Use current task for the Nth item
        let last_id = ids[n-1];
        this.graph.register_dependency(caller, StepId::Resolve(last_id));
        let last_outcome = Global::resolve_one(this, ancestors, last_id, mode).await;

        // Wait for all spawned tasks; only a *join* failure aborts the batch
        // (it is transient, never an answer).
        let mut outcomes = Vec::with_capacity(n);
        for handle in handles {
            let outcome = handle.await
                .map_err(|e| ResolveError::JoinError(format!("Task join failed: {}", e)))?;
            outcomes.push(outcome);
        }
        outcomes.push(last_outcome);
        Ok(outcomes)
    }

    /// Resolve one file-level unit through the two-layer cache — the pull
    /// direction of docs/keys-and-invalidation.md ("Recomputation From the
    /// Root"):
    ///
    /// 1. Re-derive the content key top-down: demand the parse (leaf) first,
    ///    then the imports' resolutions recursively; each level's key embeds
    ///    the *answer fingerprints* of its direct deps, and cache hits stop
    ///    the recursion below.
    /// 2. Look the key up. A hit is served without running the resolver, and
    ///    is transitively valid *by construction* — the key can only match if
    ///    every upstream answer (all the way to the leaf digests) matched
    ///    first. This is the transitive validation of
    ///    docs/cache-invalidation-problem.md #4, done by key derivation
    ///    instead of a separate validity walk.
    /// 3. On a miss, run the resolver body and store the answer under the key
    ///    — deterministic errors included, as terminal answers.
    ///
    /// The dependency set for the key (the import list) is re-derived fresh
    /// from the parse answer on every demand, never read from recorded graph
    /// edges — old edges may describe other content (the "zombie" problem the
    /// design doc warns about).
    ///
    /// Boxed because it is recursive through `resolve_each` (import chains
    /// recurse; spawning breaks the cycle for all but the inline last id).
    /// The future owns its own handle to the core, so it satisfies the
    /// `'static` bound of `tokio::spawn` without any leaking.
    fn resolve_one(this: &Arc<Global>, ancestors: AncestorPath, id: ResolveId, mode: PullMode) -> Pin<Box<dyn Future<Output = ResolveOutcome> + Send>> {
        let this = this.clone();
        Box::pin(async move {
            let fq = id.func_loc;
            debug!("CoreContext::resolve_one: {:?}", id);
            let sctx = StableCtx { interner: &this.interner };
            let parse_step = StepId::Parse(ParseId { file_path: fq.path() });
            let mut span = this.trace.span("resolve", || format!("{}", Ctx(&fq, &this.interner)));

            // Watch stance: a clean (Verified) binding is served from its
            // memoized key with zero recursion — no parse demand, no leaf
            // read, no import walk. This is step 2 of "Invalidation From the
            // Leafs" (docs/keys-and-invalidation.md): queries not marked
            // reuse their memo outright; only the dirty cone re-derives. The
            // commit replays the answer's definitions and rebinds (a no-op
            // rebind of the same record), and the step's edges stay as last
            // derived — same content, same deps.
            if mode == PullMode::TrustClean {
                if let Some(record) = this.bindings.get(&StepId::Resolve(id)) {
                    if !record.dirty {
                        if let Some(key) = record.content_key {
                            if let Some(entry) = this.store.resolve_peek(key, &this.interner).await {
                                debug!("CoreContext::resolve_one trusting clean binding for {:?}", id);
                                span.cache_hit(key);
                                span.set_fingerprint(|| Some(entry.fingerprint));
                                return this.commit_resolve(id, key, entry);
                            }
                        }
                    }
                }
            }

            // Leaf first: (re-)demand the parse. A content-store hit costs a
            // read + digest, not a parse.
            let pre = match this.parse_impl(StepId::Resolve(id), ParseId { file_path: fq.path() }, mode).await {
                Ok(pre) => pre,
                Err(e) => {
                    // A transient IO failure never produced a digest: nothing
                    // to key on, nothing may be cached (invariant 6).
                    if matches!(e, ParseError::IoError(_)) {
                        let path_str = fq.path_str(&this.interner).to_string();
                        return Err((ResolveError::ParseError(path_str, e).into(), None));
                    }
                    // A deterministic parse error is the leaf's terminal
                    // answer; the wrapper is *this* step's answer,
                    // deterministic in (fq, parse answer) — cacheable under a
                    // key with an empty dep list, like any other answer. The
                    // dep set re-derived from this content is just the parse:
                    // edges from previous content must not linger.
                    this.graph.replace_dependencies(StepId::Resolve(id), [parse_step]);
                    let parse_fp = Fingerprint::of_err(&e, &sctx);
                    let path_str = fq.path_str(&this.interner).to_string();
                    let err = Located::bare(ResolveError::ParseError(path_str, e));
                    let fp = Fingerprint::of_err(&err, &sctx);
                    let key = Self::resolve_key(&sctx, fq, parse_fp, &[]);
                    let entry = this.store.resolve_store_terminal(key, ResolveEntry { answer: Err(err), fingerprint: fp }, &this.interner).await;
                    span.cache_miss(key);
                    span.set_fingerprint(|| Some(entry.fingerprint));
                    return this.commit_resolve(id, key, entry);
                }
            };
            // Fingerprint the answer we actually hold (not a memo), so the
            // key derivation can never pair a stale fingerprint with fresh
            // content.
            let parse_fp = Fingerprint::of_ok(pre, &sctx);

            let context_str = fq.name_str(&this.interner);
            let imports = match crate::resolve::extract_import_names(pre, context_str) {
                Ok(imports) => imports,
                // Deterministic in the parse answer alone, so cacheable under
                // a key with an empty dep list. That preimage cannot alias a
                // successful no-import resolution: same parse fingerprint
                // means same PreExpr means the same extraction outcome.
                Err(e) => {
                    this.graph.replace_dependencies(StepId::Resolve(id), [parse_step]);
                    let e = Located::bare(e);
                    let fp = Fingerprint::of_err(&e, &sctx);
                    let key = Self::resolve_key(&sctx, fq, parse_fp, &[]);
                    let entry = this.store.resolve_store_terminal(key, ResolveEntry { answer: Err(e), fingerprint: fp }, &this.interner).await;
                    span.cache_miss(key);
                    span.set_fingerprint(|| Some(entry.fingerprint));
                    return this.commit_resolve(id, key, entry);
                }
            };

            // Map import names to their file-level resolutions, as
            // process_imports did: sibling `.telsb` files, callable by name.
            let base_dir = std::path::Path::new(fq.path_str(&this.interner))
                .parent()
                .unwrap_or(std::path::Path::new("."))
                .to_path_buf();
            let mut import_ids = Vec::with_capacity(imports.len());
            let mut import_funcs = Vec::with_capacity(imports.len());
            for name in &imports {
                let full_path = base_dir.join(format!("{}.telsb", name));
                let import_fq = FQ::intern(&this.interner, &full_path.to_string_lossy(), name);
                import_ids.push(ResolveId { func_loc: import_fq });
                import_funcs.push((name.clone(), FuncId(import_fq)));
            }

            // The dep set just re-derived from the *current* content replaces
            // this step's edges wholesale. In a persistent process the same
            // logical id is re-demanded across edits; letting edges from
            // previous content accumulate would grow the graph with zombies —
            // and an import restructure could even join stale and fresh edges
            // into a phantom cycle.
            let mut dep_steps = Vec::with_capacity(import_ids.len() + 1);
            dep_steps.push(parse_step);
            dep_steps.extend(import_ids.iter().map(|i| StepId::Resolve(*i)));
            this.graph.replace_dependencies(StepId::Resolve(id), dep_steps);

            // Recurse into the deps. The chain now includes this resolution,
            // so an import that re-enters it errors as a cycle before any
            // wait-for edge can form.
            let chain = ancestors.extended(fq);
            let outcomes = match Global::resolve_each(&this, StepId::Resolve(id), chain, &import_ids, mode).await {
                Ok(outcomes) => outcomes,
                // Batch-level failure: cycle or join — non-terminal, nothing
                // above it may be cached.
                Err(e) => return Err((e, None)),
            };

            // Collect each dep's answer fingerprint — success or terminal
            // error alike. A dep that stably errors is still an answer, so
            // this step's key stays derivable and its (propagated) failure is
            // itself cacheable: the preimage pins every dep answer, and the
            // first failing import is determined by them.
            let mut deps: Vec<(FQ, Fingerprint)> = Vec::with_capacity(outcomes.len());
            let mut first_failure: Option<Located<ResolveError>> = None;
            let mut non_terminal = false;
            for (dep_id, outcome) in import_ids.iter().zip(&outcomes) {
                match outcome {
                    Ok((_expr, _table, fp)) => deps.push((dep_id.func_loc, *fp)),
                    Err((e, Some(fp))) => {
                        deps.push((dep_id.func_loc, *fp));
                        if first_failure.is_none() {
                            first_failure = Some(e.clone());
                        }
                    }
                    Err((e, None)) => {
                        non_terminal = true;
                        if first_failure.is_none() {
                            first_failure = Some(e.clone());
                        }
                    }
                }
            }
            if let Some(err) = first_failure {
                if non_terminal {
                    // Some dep aborted rather than answered (cycle, join,
                    // IO): this step has no complete preimage — propagate
                    // uncached (invariant 6).
                    return Err((err, None));
                }
                let key = Self::resolve_key(&sctx, fq, parse_fp, &deps);
                if let Some(entry) = this.store.resolve_peek(key, &this.interner).await {
                    span.cache_hit(key);
                    span.set_fingerprint(|| Some(entry.fingerprint));
                    return this.commit_resolve(id, key, entry);
                }
                let fp = Fingerprint::of_err(&err, &sctx);
                let entry = this.store.resolve_store_terminal(key, ResolveEntry { answer: Err(err), fingerprint: fp }, &this.interner).await;
                span.cache_miss(key);
                span.set_fingerprint(|| Some(entry.fingerprint));
                return this.commit_resolve(id, key, entry);
            }

            let key = Self::resolve_key(&sctx, fq, parse_fp, &deps);
            if let Some(entry) = this.store.resolve_peek(key, &this.interner).await {
                debug!("CoreContext::resolve_one cache hit for {:?}", id);
                span.cache_hit(key);
                span.set_fingerprint(|| Some(entry.fingerprint));
                return this.commit_resolve(id, key, entry);
            }

            debug!("CoreContext::resolve_one resolving {:?}", id);
            // Single-flight the body compute on the content key (Decision 4):
            // concurrent demands for one resolution coalesce onto this one run
            // instead of racing (first-write-wins wasted the loser's work). The
            // whole recompute — including the panic boundary — lives inside the
            // claim's `init`, so it runs exactly once per key across demanders,
            // and `computed_resolves` counts real computes (not coalesced
            // waiters, not disk hits).
            let compute = this.store.resolve_get_or_compute(key, &this.interner, || async {
                this.computed_resolves.fetch_add(1, Ordering::Relaxed);
                let ctx = ResolveContext { current: id, core: &this };
                // The recompute boundary (docs/cache-invalidation-problem.md #8):
                // a panic here is an accident of the run, not an answer. Caught
                // *inside* `init` and surfaced as an abort, it reverts the claim
                // (a waiter re-runs) and writes nothing — the binding keeps
                // whatever state it had (dirty in a marked cone), so the next
                // pull recomputes the affected chain. The injected needle is
                // test support for exactly that property.
                let body_outcome = {
                    let needle = this.panic_on_resolve.lock().unwrap().clone();
                    catch_unwind(AssertUnwindSafe(|| {
                        if let Some(needle) = &needle {
                            let path_str = fq.path_str(&this.interner);
                            if path_str.contains(needle.as_str()) {
                                panic!("injected panic resolving {}", path_str);
                            }
                        }
                        crate::resolve::resolve_body(&ctx, id, pre, &import_funcs)
                    }))
                };
                let body_result = match body_outcome {
                    Ok(result) => result,
                    // Non-terminal (invariant 6): abort the claim, uncached.
                    Err(payload) => return Err(panic_message(payload)),
                };
                let entry = match body_result {
                    Ok((ast, table, registered)) => {
                        let funcs: Vec<(FQ, FuncData)> = registered.into_iter()
                            .map(|f| {
                                let data = this.func_registry.get(&f)
                                    .expect("resolve_body registered this function")
                                    .clone();
                                (f, data)
                            })
                            .collect();
                        let answer = ResolveAnswer { ast, table, funcs };
                        let fingerprint = Fingerprint::of_ok(&answer, &sctx);
                        ResolveEntry { answer: Ok(answer), fingerprint }
                    }
                    // Body errors are deterministic in the inputs the key already
                    // pins (parse answer + import answers), so they are terminal
                    // answers (invariant 6), fingerprinted like successes.
                    // Non-deterministic failures cannot come from the synchronous
                    // body: IO lives in parse, and cycles/joins happen in the dep
                    // recursion above (uncached).
                    Err(e) => {
                        let fingerprint = Fingerprint::of_err(&e, &sctx);
                        ResolveEntry { answer: Err(e), fingerprint }
                    }
                };
                Ok(entry)
            }).await;
            let entry = match compute {
                Ok(entry) => entry,
                Err(msg) => return Err((ResolveError::Panicked(msg).into(), None)),
            };
            span.cache_miss(key);
            span.set_fingerprint(|| Some(entry.fingerprint));
            this.commit_resolve(id, key, entry)
        })
    }

    /// Content key of one resolve step: `hash(kind, args, direct deps'
    /// fingerprints)` — the args are the FQ (the resolved AST embeds
    /// path-based FQs, so identical content at two paths must not share), the
    /// deps are the parse of this file plus the resolve of each direct
    /// import. Dep identity is pinned alongside each fingerprint
    /// (invariant 3), and the count is length-prefixed.
    fn resolve_key(ctx: &StableCtx<'_>, fq: FQ, parse_fp: Fingerprint, deps: &[(FQ, Fingerprint)]) -> ContentKey {
        ContentKey::build(QueryKind::Resolve, |h| {
            fq.stable_hash(ctx, h);
            parse_fp.stable_hash(ctx, h);
            h.write_len(deps.len());
            for (dep_fq, dep_fp) in deps {
                dep_fq.stable_hash(ctx, h);
                dep_fp.stable_hash(ctx, h);
            }
        })
    }

    /// Commit a resolve answer for `id`: replay its definitions into the
    /// positional registry, then rebind the logical id — one whole-record
    /// insert, the atomic last step (invariant 4) — and return the caller's
    /// view. Both arms carry the answer's fingerprint: a terminal error is an
    /// answer, and dependents key on it like any other.
    fn commit_resolve(&self, id: ResolveId, key: ContentKey, entry: ResolveEntry) -> ResolveOutcome {
        if let Ok(answer) = &entry.answer {
            // Overwrite, not or_insert: the position may currently hold
            // data from different content (e.g. before a revert), and the
            // answer under this key is what the position resolves to now.
            // The definer memo rides along: mono steps depend on the
            // *file-level* resolve step that defined their function, and this
            // is where that association is authoritative.
            for (f, data) in &answer.funcs {
                self.func_registry.insert(*f, data.clone());
                self.definer.insert(*f, id);
            }
        }
        // Rebinding is the atomic last step, after the commit's side effects.
        self.bindings.record(StepId::Resolve(id), BindingRecord {
            content_key: Some(key),
            fingerprint: entry.fingerprint,
            input_digest: None,
            dirty: false,
        });
        match entry.answer {
            Ok(answer) => Ok((answer.ast, answer.table, entry.fingerprint)),
            Err(e) => Err((e, Some(entry.fingerprint))),
        }
    }

    /// Type check + monomorphise every instance reachable from `entry`.
    ///
    /// Since a call's result type equals its instantiation type, checking an
    /// instance never needs its callees checked first, so this is a plain
    /// memoized worklist rather than a recursive query: recursion (including
    /// polymorphic recursion between the i32 and i64 instances of a function)
    /// terminates because there are at most two instances per function.
    fn mono_impl(&self, caller: StepId, entry: MonoId, mode: PullMode) -> Result<(), Located<TypeError>> {
        debug!("CoreContext::mono_impl: entry {:?}", entry);
        self.graph.register_dependency(caller, StepId::Mono(entry));

        let mut queue = vec![(entry, true)];
        while let Some((key, is_entry)) = queue.pop() {
            if self.mono_registry.contains_key(&key) {
                continue;
            }
            let step = StepId::Mono(key);
            let mut span = self.trace.span("mono", || format!("{} @ {}", Ctx(&key.func_loc, &self.interner), key.ty));

            // Watch stance: a clean instance is replayed from its memoized
            // key — no registry fingerprinting, no check. Sound because the
            // instance depends only on its definer's resolve step, and that
            // edge is in the cone-marking graph: had the defining file
            // changed (and been announced), this binding would be dirty.
            if mode == PullMode::TrustClean {
                if let Some(record) = self.bindings.get(&step) {
                    if !record.dirty {
                        if let Some(bound_key) = record.content_key {
                            if let Some(answer) = self.store.mono_get(&bound_key, &self.interner) {
                                span.cache_hit(bound_key);
                                span.set_fingerprint(|| Some(record.fingerprint));
                                let (data, needed) = answer?;
                                self.mono_registry.insert(key, data);
                                for callee in needed {
                                    queue.push((callee, false));
                                }
                                continue;
                            }
                        }
                    }
                }
            }

            // Mono consumes the resolved AST of exactly one function; the
            // graph edge points at the *file-level* resolve step that defined
            // it (the definer memo), because `Resolve(function FQ)` is not a
            // step any resolution ever performs — an edge to that phantom
            // node would leave this instance outside the leaf's marking cone
            // and push invalidation would under-mark, serving stale
            // instances. The fallback covers the file-function itself, whose
            // FQ *is* the file-level resolve id.
            let definer = self.definer.get(&key.func_loc)
                .map(|d| *d)
                .unwrap_or(ResolveId { func_loc: key.func_loc });
            let resolve_dep = StepId::Resolve(definer);

            // The cache key chains to the resolve stage via the fingerprint
            // of the one thing `check_function` consumes: this function's
            // resolved data. Fingerprinted from the registry entry we hold
            // right now, so a changed function cannot be served stale — and
            // an *unchanged* function is a hit even if its file changed
            // elsewhere (function-level cutoff).
            let sctx = StableCtx { interner: &self.interner };
            let func_fp = self.func_registry.get(&key.func_loc)
                .map(|f| Fingerprint::of(&*f, &sctx));
            let Some(func_fp) = func_fp else {
                // Nothing resolved at this position: a wave-ordering problem,
                // not a content-addressable answer — surface the phase's
                // standard error, uncached (invariant 6: only terminal
                // deterministic answers are persisted).
                let ctx = MonoContext { core: self };
                crate::typecheck::check_function(&ctx, key, is_entry)?;
                unreachable!("check_function must fail for an unresolved function");
            };
            let preimage = MonoCacheKey { func_fp, func_loc: key.func_loc, ty: key.ty, is_entry };
            let cache_key = preimage.content_key(&sctx);

            let answer: MonoAnswer = match self.store.mono_get(&cache_key, &self.interner) {
                Some(hit) => {
                    span.cache_hit(cache_key);
                    hit
                }
                None => {
                    debug!("CoreContext::mono_impl checking {:?} (func fp {:?})", key, func_fp);
                    self.computed_monos.fetch_add(1, Ordering::Relaxed);
                    let ctx = MonoContext { core: self };
                    // The recompute boundary, like resolve's: a panic is not
                    // an answer, so it returns before the store insert and
                    // the binding commit — nothing cached, node stays dirty.
                    let computed = match catch_unwind(AssertUnwindSafe(|| {
                        // A deterministic `TypeError` is a terminal answer: it is
                        // stored like a success, so re-demanding the same content
                        // reports the same error without re-checking.
                        crate::typecheck::check_function(&ctx, key, is_entry)
                    })) {
                        Ok(computed) => computed,
                        Err(payload) => return Err(TypeError::Panicked(panic_message(payload)).into()),
                    };
                    let stored = self.store.mono_insert(cache_key, computed, &self.interner);
                    span.cache_miss(cache_key);
                    stored
                }
            };

            // The dep set re-derived from the answer we hold replaces this
            // instance's edges wholesale (same discipline as resolve): the
            // definer edge plus the callee instances on success, the definer
            // alone on a terminal type error. Stale callee edges from
            // previous content would otherwise accumulate across runs —
            // harmless-but-growing over-marking the docs say to avoid at the
            // source when the fresh set is already in hand.
            match &answer {
                Ok((_, needed)) => {
                    let deps = std::iter::once(resolve_dep)
                        .chain(needed.iter().map(|callee| StepId::Mono(*callee)));
                    self.graph.replace_dependencies(step, deps);
                }
                Err(_) => {
                    self.graph.replace_dependencies(step, [resolve_dep]);
                }
            }

            if !self.bindings.is_current(&step, cache_key) {
                let sctx = StableCtx { interner: &self.interner };
                let fingerprint = match &answer {
                    Ok(pair) => Fingerprint::of_ok(pair, &sctx),
                    Err(e) => Fingerprint::of_err(e, &sctx),
                };
                self.bindings.record(step, BindingRecord {
                    content_key: Some(cache_key),
                    fingerprint,
                    input_digest: None,
                    dirty: false,
                });
            }
            span.set_fingerprint(|| self.bindings.get(&step).map(|r| r.fingerprint));

            let (data, needed) = answer?;
            self.mono_registry.insert(key, data);

            for callee in needed {
                queue.push((callee, false));
            }
        }
        Ok(())
    }

    /// Execute the program rooted at `id`.
    ///
    /// Exec is deliberately *not* a cached query: its value is its side
    /// effects (`print`), so re-demanding it must re-run them — same content
    /// prints again. What IS cached is everything exec pulls: the compiled
    /// artifact (parse/resolve/mono answers) comes from the content store,
    /// only the interpretation re-runs.
    async fn execute_impl(this: &Arc<Global>, caller: StepId, id: ExecId, mode: PullMode) -> Result<(), ExecuteError> {
        debug!("CoreContext::execute_impl: {:?}", id);
        // Exec is uncached (its value is its side effects), but it is still a
        // step: the span records its duration, with no key and no cache outcome.
        let _span = this.trace.span("exec", || format!("{}", Ctx(&id.main_loc, &this.interner)));
        this.graph.register_dependency(caller, StepId::Exec(id.clone()));
        // The mono *registry* is the positional (FQ + type) view of the
        // current program, so it must be rebuilt per run: a re-run may have
        // re-resolved a changed file under the same FQ. Cross-run reuse
        // happens in `mono_store`, whose keys are content-addressed. Clearing
        // it here is also why runs on one compiler are serialized
        // (`Compiler::run` takes `&mut self`).
        this.mono_registry.clear();
        let ctx = ExecContext {
            current: id.clone(),
            core: this.clone(),
            mode,
        };
        crate::execute::execute(&ctx, id).await
    }

    /// Lower the program rooted at `id` to a Python script instead of running
    /// it. Shares exec's front end (resolve + monomorphise) — including the
    /// per-run rebuild of the positional mono registry — but replaces the
    /// interpreting backend with the compiling one. Like exec it is uncached:
    /// the emitted source is the whole value and is returned, not stored.
    async fn codegen_impl(this: &Arc<Global>, caller: StepId, id: ExecId, mode: PullMode) -> Result<String, crate::codegen::CodegenError> {
        debug!("CoreContext::codegen_impl: {:?}", id);
        this.graph.register_dependency(caller, StepId::Exec(id.clone()));
        this.mono_registry.clear();
        let ctx = ExecContext {
            current: id.clone(),
            core: this.clone(),
            mode,
        };
        crate::codegen::generate_python(&ctx, id).await
    }
}

pub struct RootContext {
    core: Arc<Global>,
}

impl RootContext {
    pub fn new(core: Arc<Global>) -> Self {
        RootContext { core }
    }

    pub fn graph(&self) -> &Graph {
        &self.core.graph
    }

    pub fn interner(&self) -> &Interner {
        &self.core.interner
    }

    pub fn printer(&self) -> &dyn Printer {
        self.core.printer.as_ref()
    }

    pub async fn execute(&self, id: ExecId, mode: PullMode) -> Result<(), ExecuteError> {
        Global::execute_impl(&self.core, StepId::Root, id, mode).await
    }

    pub async fn codegen_python(&self, id: ExecId, mode: PullMode) -> Result<String, crate::codegen::CodegenError> {
        Global::codegen_impl(&self.core, StepId::Root, id, mode).await
    }
}

/// Context handed to the resolver *body*. Deliberately narrow: parsing and
/// import resolution happen before the body runs (in `Global::resolve_one`,
/// which needs their fingerprints to build the step's content key first), so
/// the body can neither recurse nor do IO — it only reads the interner and
/// the function registry. Synchronous and short-lived, so it borrows the core
/// rather than holding a handle.
pub struct ResolveContext<'a> {
    #[allow(dead_code)] // identifies the step; kept for symmetry/debugging
    current: ResolveId,
    core: &'a Global,
}

impl ResolveContext<'_> {
    pub fn interner(&self) -> &Interner {
        &self.core.interner
    }

    pub fn func_registry(&self) -> &DashMap<FQ, FuncData> {
        &self.core.func_registry
    }
}

/// Context handed to the type check / monomorphisation phase. It only reads
/// the resolved function registry; dependency edges are registered by the
/// worklist driver in `Global::mono_impl`. Synchronous and short-lived, so it
/// borrows the core rather than holding a handle.
pub struct MonoContext<'a> {
    core: &'a Global,
}

impl MonoContext<'_> {
    pub fn interner(&self) -> &Interner {
        &self.core.interner
    }

    pub fn func_registry(&self) -> &DashMap<FQ, FuncData> {
        &self.core.func_registry
    }
}

impl Global {
    /// Build (or fetch) the parse **span sidecar** for `source`. Keyed on the
    /// source digest in the `Spans` keyspace, so identical bytes share one
    /// table and it is cached for the process; produced by re-parsing with
    /// span recording on (the fast parse recorded none). A parse failure
    /// yields an empty table — callers only ask for source that already
    /// compiled, so the locator will be present in practice.
    fn spans_for(&self, source: &str) -> Arc<SpanTable> {
        let digest = ContentDigest::of(source);
        let key = ContentKey::build(QueryKind::Spans, |h| {
            digest.stable_hash(&StableCtx { interner: &self.interner }, h);
        });
        self.store.spans_get_or_build(key, || {
            crate::parse::parse_with_spans(source)
                .map(|(_ast, table)| table)
                .unwrap_or_default()
        })
    }
}

pub struct ExecContext {
    current: ExecId,
    core: Arc<Global>,
    /// The reconciliation stance of the wave this exec drives; threaded into
    /// every pull it makes.
    mode: PullMode,
}

impl ExecContext {
    pub fn graph(&self) -> &Graph {
        &self.core.graph
    }

    pub fn interner(&self) -> &Interner {
        &self.core.interner
    }

    pub fn func_registry(&self) -> &DashMap<FQ, FuncData> {
        &self.core.func_registry
    }

    pub fn mono_registry(&self) -> &DashMap<MonoId, MonoFuncData> {
        &self.core.mono_registry
    }

    pub fn printer(&self) -> &dyn Printer {
        self.core.printer.as_ref()
    }

    /// Opt-level of this compile — the backend flavor (roadmap item 15).
    /// `execute` stands in for codegen; a real optimizer/codegen would branch
    /// on this. No cached query reads it, so it changes no content key.
    pub fn opt_level(&self) -> crate::flavors::OptLevel {
        self.core.flavors.opt
    }

    pub async fn resolve_all(&self, ids: &[ResolveId]) -> Result<(Vec<Expr>, SymbolTable), Located<ResolveError>> {
        // Exec is the root of a resolve tree: no resolutions are in progress yet.
        Global::resolve_all_impl(&self.core, StepId::Exec(self.current.clone()), AncestorPath::empty(), ids, self.mode).await
    }

    pub fn mono(&self, entry: MonoId) -> Result<(), Located<TypeError>> {
        self.core.mono_impl(StepId::Exec(self.current.clone()), entry, self.mode)
    }

    /// Upgrade a coarse panic location (`source_location`, a file path) to
    /// `path:line:col` by demanding the span sidecar for that file and mapping
    /// the `(frame, node)` locator through it. This is the in-session
    /// upgrade-on-error of plans/fast-mode.md ("the runtime story"): the happy
    /// path computes no sidecar, and only a fired `panic` pays the leaf-cost
    /// reparse. Degrades to the bare path if the file can't be read or the
    /// locator isn't in the table (e.g. the source changed since compile), so
    /// the message is never worse than the pre-sidecar behaviour.
    pub fn render_panic_location(&self, source_location: &str, frame: u32, node: u32) -> String {
        let Ok(source) = std::fs::read_to_string(source_location) else {
            return source_location.to_string();
        };
        let table = self.core.spans_for(&source);
        match table.get(frame, node) {
            Some(span) => {
                let (line, col) = crate::types::line_col(&source, span.start);
                format!("{}:{}:{}", source_location, line, col)
            }
            None => source_location.to_string(),
        }
    }

    /// Render a located compile error (resolve or type check) for display,
    /// upgrading its structural `(frame, node)` locator to `path:line:col` via
    /// the span sidecar — the same lazy on-error upgrade as a runtime `panic`
    /// (plans/fast-mode.md). A locator-less error (whole-file: IO, cycle,
    /// import placement) renders coarsely, exactly as before. The reparse the
    /// sidecar costs is paid only here, on the error path.
    pub fn render_located<E: std::fmt::Display>(&self, located: &Located<E>) -> String {
        match &located.loc {
            Some(loc) => format!(
                "{}: {}",
                self.render_panic_location(&loc.file, loc.frame, loc.node),
                located.error,
            ),
            None => format!("{}", located.error),
        }
    }
}

// Compile-time assertions to ensure contexts are Send (required for tokio::spawn)
const _: fn() = || {
    fn assert_send<T: Send>() {}
    assert_send::<RootContext>();
    assert_send::<ResolveContext<'_>>();
    assert_send::<MonoContext<'_>>();
    assert_send::<ExecContext>();
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::Path;
    use crate::NoopPrinter;
    use std::fs;
    use tempfile::TempDir;

    fn global() -> Arc<Global> {
        Arc::new(Global::new(Arc::new(NoopPrinter), crate::flavors::Flavors::default()))
    }

    /// After a parse completes, its logical id is bound — atomically, as one
    /// record — to the content key, output fingerprint, and leaf digest; the
    /// binding follows the content when it changes.
    #[tokio::test]
    async fn parse_binds_key_digest_and_fingerprint() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("main.telsb");
        fs::write(&file, "(print 42)\n").unwrap();

        let g = global();
        let id = ParseId { file_path: Path::intern(&g.interner, file.to_str().unwrap()) };
        g.parse_impl(StepId::Root, id, PullMode::Reconcile).await.unwrap();

        let record = g.bindings.get(&StepId::Parse(id)).expect("parse step is bound");
        let key = record.content_key.expect("leaf content key is bound");
        let fingerprint = record.fingerprint;
        assert_eq!(record.input_digest, Some(ContentDigest::of("(print 42)\n")));
        assert!(!record.dirty);

        // Unchanged re-demand: the binding stays current.
        g.parse_impl(StepId::Root, id, PullMode::Reconcile).await.unwrap();
        let again = g.bindings.get(&StepId::Parse(id)).unwrap();
        assert_eq!(again.content_key, Some(key));
        assert_eq!(again.fingerprint, fingerprint);

        // Changed content rebinds the same logical id to a new key (and here
        // a new fingerprint — the AST changed too).
        fs::write(&file, "(print 43)\n").unwrap();
        g.parse_impl(StepId::Root, id, PullMode::Reconcile).await.unwrap();
        let rebound = g.bindings.get(&StepId::Parse(id)).unwrap();
        assert_ne!(rebound.content_key, Some(key));
        assert_ne!(rebound.fingerprint, fingerprint);
    }

    /// A deterministic parse error is a terminal answer: it is stored in the
    /// content store (no recompute on re-demand) and bound in the memo, just
    /// without an output fingerprint.
    #[tokio::test]
    async fn parse_error_is_a_cached_answer() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("broken.telsb");
        fs::write(&file, "(print 42").unwrap();

        let g = global();
        let id = ParseId { file_path: Path::intern(&g.interner, file.to_str().unwrap()) };
        assert!(g.parse_impl(StepId::Root, id, PullMode::Reconcile).await.is_err());
        assert!(g.parse_impl(StepId::Root, id, PullMode::Reconcile).await.is_err());

        assert_eq!(g.computed_parse_count(), 1, "the cached error must be served, not re-parsed");
        let record = g.bindings.get(&StepId::Parse(id)).expect("errored parse is still bound");
        assert!(record.content_key.is_some());

        // The error answer is fingerprinted (tagged apart from successes), so
        // a dependent above it can still derive its own content key.
        fs::write(&file, "(print 42)\n").unwrap();
        g.parse_impl(StepId::Root, id, PullMode::Reconcile).await.unwrap();
        let fixed = g.bindings.get(&StepId::Parse(id)).unwrap();
        assert_ne!(fixed.fingerprint, record.fingerprint, "an Ok answer must never fingerprint like the Err");
    }
}
