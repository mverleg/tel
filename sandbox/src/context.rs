use crate::common::{Interner, FQ};
use crate::graph::{ExecId, Graph, MonoId, ParseId, ResolveId, StepId};
use crate::keys::{ContentDigest, ContentKey, Fingerprint, QueryKind, StableCtx, StableHash};
use crate::store::{BindingLayer, BindingRecord, ContentStore, MonoAnswer, ResolveAnswer, ResolveEntry};
use crate::types::{ExecuteError, Expr, FuncData, FuncId, MonoFuncData, ParseError, PreExpr, ResolveError, SymbolTable, Ty, TypeError};
use crate::Printer;
use dashmap::DashMap;
use log::debug;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

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

/// Not actually forced to be singleton, but it's leaked so singleton is encouraged.
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
    /// Monomorphised instances of the *current* run, keyed by function +
    /// numeric type. This is the positional view the interpreter executes
    /// from; it is rebuilt per run (from content-store hits where possible).
    mono_registry: DashMap<MonoId, MonoFuncData>,
    /// Parse computations actually executed (content-store misses).
    computed_parses: AtomicUsize,
    /// Resolve computations actually executed (content-store misses).
    computed_resolves: AtomicUsize,
    /// Mono checks actually executed (content-store misses).
    computed_monos: AtomicUsize,
    printer: &'static dyn Printer,
}

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
    pub fn new(printer: &'static dyn Printer) -> Self {
        Global {
            graph: Graph::new(),
            interner: Interner::new(),
            store: ContentStore::new(),
            bindings: BindingLayer::new(),
            func_registry: DashMap::new(),
            mono_registry: DashMap::new(),
            computed_parses: AtomicUsize::new(0),
            computed_resolves: AtomicUsize::new(0),
            computed_monos: AtomicUsize::new(0),
            printer,
        }
    }

    /// Number of distinct source contents parsed and cached so far (by digest).
    /// Lets callers observe cross-run cache reuse: an unchanged or reverted file
    /// does not grow this count on a subsequent run.
    pub fn cached_parse_count(&self) -> usize {
        self.store.parse_len()
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
}

impl Global {
    async fn parse_impl(&'static self, caller: StepId, id: ParseId) -> Result<&'static PreExpr, &'static ParseError> {
        debug!("CoreContext::parse_impl: {:?}", id);

        // Always register dependency, regardless of cache hit/miss. The graph
        // node stays keyed on the file *position* (path); the content digest is
        // an implementation detail of the parse cache below.
        self.graph.register_dependency(caller, StepId::Parse(id));

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
            // store or the binding layer. Leak a 'static error to satisfy the
            // signature (the compiler already leaks its Global/interner).
            Err(err) => return Err(Box::leak(Box::new(ParseError::from(err)))),
        };

        let digest = ContentDigest::of(&source);
        // Parse is the leaf query, so its content key hashes the external
        // input (the byte digest) — schema version and kind tag are folded in
        // by construction.
        let key = ContentKey::build(QueryKind::Parse, |h| {
            digest.stable_hash(&StableCtx { interner: &self.interner }, h);
        });
        let result = self.store.parse_get(key, move || async move {
            debug!("CoreContext::parse_impl parsing {:?} (digest {:?})", path, digest);
            self.computed_parses.fetch_add(1, Ordering::Relaxed);
            crate::parse::tokenize_and_parse(&source, path)
        }).await;

        // Rebind this position to what its content resolves to now: content
        // key, output fingerprint, and the leaf digest later stages chain
        // their keys to. One whole-record insert (memo updates are atomic per
        // query); skipped when the binding is already current, so an
        // unchanged re-demand does not re-fingerprint the AST.
        let step = StepId::Parse(id);
        if !self.bindings.is_current(&step, key) {
            let ctx = StableCtx { interner: &self.interner };
            self.bindings.record(step, BindingRecord {
                content_key: Some(key),
                // A deterministic parse error is a terminal answer and is
                // cached above, but carries no output fingerprint yet (error
                // fingerprints arrive with early cutoff).
                fingerprint: result.as_ref().ok().map(|pre| Fingerprint::of(pre, &ctx)),
                input_digest: Some(digest),
                dirty: false,
            });
        }

        // Return borrowed reference to cached result
        result.as_ref()
    }

    async fn resolve_all_impl(&'static self, caller: StepId, ancestors: AncestorPath, ids: &[ResolveId]) -> Result<(Vec<Expr>, SymbolTable), ResolveError> {
        let (results, table) = self.resolve_all_fp(caller, ancestors, ids).await?;
        Ok((results.into_iter().map(|(expr, _fp)| expr).collect(), table))
    }

    /// Resolve every id (concurrently for more than one), returning each
    /// answer *with its result fingerprint* — the ingredient a dependent needs
    /// to build its own content key.
    async fn resolve_all_fp(&'static self, caller: StepId, ancestors: AncestorPath, ids: &[ResolveId]) -> Result<(Vec<(Expr, Fingerprint)>, SymbolTable), ResolveError> {
        debug!("CoreContext::resolve_all_fp x{}: {:?}", ids.len(), ids);

        // Deadlock-safe cycle detection (docs/cycle-detection.md): a requested
        // resolution that is already on this task's in-progress ancestor chain
        // closes an import cycle. Checked before any await/spawn, so the
        // wait-for cycle — two parked tasks awaiting each other — never forms.
        for id in ids {
            if ancestors.contains(id.func_loc) {
                return Err(ResolveError::CyclicDependency {
                    cycle: ancestors.cycle_strings(id.func_loc, &self.interner),
                });
            }
        }

        if ids.is_empty() {
            return Ok((Vec::new(), SymbolTable::new()));
        }

        let n = ids.len();
        if n == 1 {
            let id = ids[0];
            self.graph.register_dependency(caller, StepId::Resolve(id));
            let (expr, table, fp) = self.resolve_one(ancestors, id).await?;
            return Ok((vec![(expr, fp)], table));
        }

        // Spawn tasks for items 0..N-1
        let mut handles = Vec::new();
        let core = self;
        for i in 0..n-1 {
            let id = ids[i];
            let ancestors = ancestors.clone();
            let handle = tokio::spawn(async move {
                core.graph.register_dependency(caller, StepId::Resolve(id));
                core.resolve_one(ancestors, id).await
            });
            handles.push(handle);
        }

        // Use current task for the Nth item
        let last_id = ids[n-1];
        self.graph.register_dependency(caller, StepId::Resolve(last_id));
        let last_result = self.resolve_one(ancestors, last_id).await?;

        // Wait for all spawned tasks
        let mut all_results = Vec::with_capacity(n);
        for handle in handles {
            let result = handle.await
                .map_err(|e| ResolveError::JoinError(format!("Task join failed: {}", e)))?;
            all_results.push(result?);
        }
        all_results.push(last_result);

        // Build result vectors
        let mut results = Vec::with_capacity(n);
        let mut merged_table = SymbolTable::new();

        for (expr, table, fp) in all_results {
            results.push((expr, fp));
            merged_table.vars.extend(table.vars);
            // funcs are now in global registry, no need to merge
        }

        Ok((results, merged_table))
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
    /// Boxed because it is recursive through `resolve_all_fp` (import chains
    /// recurse; spawning breaks the cycle for all but the inline last id).
    fn resolve_one(&'static self, ancestors: AncestorPath, id: ResolveId) -> Pin<Box<dyn Future<Output = Result<(Expr, SymbolTable, Fingerprint), ResolveError>> + Send>> {
        Box::pin(async move {
            let fq = id.func_loc;
            debug!("CoreContext::resolve_one: {:?}", id);

            // Leaf first: (re-)demand the parse. A content-store hit costs a
            // read + digest, not a parse.
            let pre = match self.parse_impl(StepId::Resolve(id), ParseId { file_path: fq.path() }).await {
                Ok(pre) => pre,
                // The parse error itself is the cached (terminal) answer;
                // this wrapper is cheap re-derivation on every demand. It is
                // not stored: an error output has no fingerprint yet, so a
                // resolve above it cannot be keyed (error fingerprints arrive
                // with early cutoff).
                Err(e) => {
                    let path_str = fq.path_str(&self.interner).to_string();
                    return Err(ResolveError::ParseError(path_str, e.clone()));
                }
            };
            // Fingerprint the answer we actually hold (not a memo), so the
            // key derivation can never pair a stale fingerprint with fresh
            // content.
            let sctx = StableCtx { interner: &self.interner };
            let parse_fp = Fingerprint::of(pre, &sctx);

            let context_str = fq.name_str(&self.interner);
            let imports = match crate::resolve::extract_import_names(pre, context_str) {
                Ok(imports) => imports,
                // Deterministic in the parse answer alone, so cacheable under
                // a key with an empty dep list. That preimage cannot alias a
                // successful no-import resolution: same parse fingerprint
                // means same PreExpr means the same extraction outcome.
                Err(e) => {
                    let key = Self::resolve_key(&sctx, fq, parse_fp, &[]);
                    let entry = self.store.resolve_insert(key, ResolveEntry { answer: Err(e), fingerprint: None });
                    return self.commit_resolve(id, key, entry);
                }
            };

            // Map import names to their file-level resolutions, as
            // process_imports did: sibling `.telsb` files, callable by name.
            let base_dir = std::path::Path::new(fq.path_str(&self.interner))
                .parent()
                .unwrap_or(std::path::Path::new("."))
                .to_path_buf();
            let mut import_ids = Vec::with_capacity(imports.len());
            let mut import_funcs = Vec::with_capacity(imports.len());
            for name in &imports {
                let full_path = base_dir.join(format!("{}.telsb", name));
                let import_fq = FQ::intern(&self.interner, &full_path.to_string_lossy(), name);
                import_ids.push(ResolveId { func_loc: import_fq });
                import_funcs.push((name.clone(), FuncId(import_fq)));
            }

            // Recurse into the deps. The chain now includes this resolution,
            // so an import that re-enters it errors as a cycle before any
            // wait-for edge can form. A dep failure propagates uncached: the
            // failing node cached its own answer, everything above is cheap
            // re-derivation.
            let chain = ancestors.extended(fq);
            let (dep_results, _tables) = self.resolve_all_fp(StepId::Resolve(id), chain, &import_ids).await?;

            let deps: Vec<(FQ, Fingerprint)> = import_ids.iter()
                .zip(dep_results.iter())
                .map(|(dep_id, (_expr, fp))| (dep_id.func_loc, *fp))
                .collect();
            let key = Self::resolve_key(&sctx, fq, parse_fp, &deps);

            if let Some(entry) = self.store.resolve_get(&key) {
                debug!("CoreContext::resolve_one cache hit for {:?}", id);
                return self.commit_resolve(id, key, entry);
            }

            debug!("CoreContext::resolve_one resolving {:?}", id);
            self.computed_resolves.fetch_add(1, Ordering::Relaxed);
            let ctx = ResolveContext { current: id, core: self };
            let entry = match crate::resolve::resolve_body(&ctx, id, pre, &import_funcs) {
                Ok((ast, table, registered)) => {
                    let funcs: Vec<(FQ, FuncData)> = registered.into_iter()
                        .map(|f| {
                            let data = self.func_registry.get(&f)
                                .expect("resolve_body registered this function")
                                .clone();
                            (f, data)
                        })
                        .collect();
                    let answer = ResolveAnswer { ast, table, funcs };
                    let fingerprint = Fingerprint::of(&answer, &sctx);
                    ResolveEntry { answer: Ok(answer), fingerprint: Some(fingerprint) }
                }
                // Body errors are deterministic in the inputs the key already
                // pins (parse answer + import answers), so they are terminal
                // answers (invariant 6). Non-deterministic failures cannot
                // come from the synchronous body: IO lives in parse, and
                // cycles/joins happen in the dep recursion above (uncached).
                Err(e) => ResolveEntry { answer: Err(e), fingerprint: None },
            };
            let entry = self.store.resolve_insert(key, entry);
            self.commit_resolve(id, key, entry)
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
    /// view.
    fn commit_resolve(&self, id: ResolveId, key: ContentKey, entry: ResolveEntry) -> Result<(Expr, SymbolTable, Fingerprint), ResolveError> {
        match entry.answer {
            Ok(answer) => {
                // Overwrite, not or_insert: the position may currently hold
                // data from different content (e.g. before a revert), and the
                // answer under this key is what the position resolves to now.
                for (f, data) in &answer.funcs {
                    self.func_registry.insert(*f, data.clone());
                }
                let fingerprint = entry.fingerprint
                    .expect("successful resolve answers are stored with their fingerprint");
                self.bindings.record(StepId::Resolve(id), BindingRecord {
                    content_key: Some(key),
                    fingerprint: Some(fingerprint),
                    input_digest: None,
                    dirty: false,
                });
                Ok((answer.ast, answer.table, fingerprint))
            }
            Err(e) => {
                // A cached error is a terminal answer, bound like a success
                // but (for now) without an output fingerprint.
                self.bindings.record(StepId::Resolve(id), BindingRecord {
                    content_key: Some(key),
                    fingerprint: None,
                    input_digest: None,
                    dirty: false,
                });
                Err(e)
            }
        }
    }

    /// Type check + monomorphise every instance reachable from `entry`.
    ///
    /// Since a call's result type equals its instantiation type, checking an
    /// instance never needs its callees checked first, so this is a plain
    /// memoized worklist rather than a recursive query: recursion (including
    /// polymorphic recursion between the i32 and i64 instances of a function)
    /// terminates because there are at most two instances per function.
    fn mono_impl(&'static self, caller: StepId, entry: MonoId) -> Result<(), TypeError> {
        debug!("CoreContext::mono_impl: entry {:?}", entry);
        self.graph.register_dependency(caller, StepId::Mono(entry));

        let mut queue = vec![(entry, true)];
        while let Some((key, is_entry)) = queue.pop() {
            if self.mono_registry.contains_key(&key) {
                continue;
            }
            // Mono consumes the resolved AST that Resolve put in the registry.
            self.graph.register_dependency(StepId::Mono(key), StepId::Resolve(ResolveId { func_loc: key.func_loc }));

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

            let answer: MonoAnswer = match self.store.mono_get(&cache_key) {
                Some(hit) => hit,
                None => {
                    debug!("CoreContext::mono_impl checking {:?} (func fp {:?})", key, func_fp);
                    self.computed_monos.fetch_add(1, Ordering::Relaxed);
                    let ctx = MonoContext { core: self };
                    // A deterministic `TypeError` is a terminal answer: it is
                    // stored like a success, so re-demanding the same content
                    // reports the same error without re-checking.
                    let computed = crate::typecheck::check_function(&ctx, key, is_entry);
                    self.store.mono_insert(cache_key, computed)
                }
            };

            let step = StepId::Mono(key);
            if !self.bindings.is_current(&step, cache_key) {
                let sctx = StableCtx { interner: &self.interner };
                self.bindings.record(step, BindingRecord {
                    content_key: Some(cache_key),
                    fingerprint: answer.as_ref().ok().map(|pair| Fingerprint::of(pair, &sctx)),
                    input_digest: None,
                    dirty: false,
                });
            }

            let (data, needed) = answer?;
            self.mono_registry.insert(key, data);

            for callee in needed {
                self.graph.register_dependency(StepId::Mono(key), StepId::Mono(callee));
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
    async fn execute_impl(&'static self, caller: StepId, id: ExecId) -> Result<(), ExecuteError> {
        debug!("CoreContext::execute_impl: {:?}", id);
        self.graph.register_dependency(caller, StepId::Exec(id.clone()));
        // The mono *registry* is the positional (FQ + type) view of the
        // current program, so it must be rebuilt per run: a re-run may have
        // re-resolved a changed file under the same FQ. Cross-run reuse
        // happens in `mono_store`, whose keys are content-addressed.
        self.mono_registry.clear();
        let ctx = ExecContext {
            current: id.clone(),
            core: self,
        };
        crate::execute::execute(&ctx, id).await
    }
}

pub struct RootContext {
    core: &'static Global,
}

impl RootContext {
    pub fn new(core: &'static Global) -> Self {
        RootContext { core }
    }

    pub fn graph(&self) -> &Graph {
        &self.core.graph
    }

    pub fn interner(&self) -> &Interner {
        &self.core.interner
    }

    pub fn printer(&self) -> &dyn Printer {
        self.core.printer
    }

    pub async fn execute(&self, id: ExecId) -> Result<(), ExecuteError> {
        self.core.execute_impl(StepId::Root, id).await
    }
}

/// Context handed to the resolver *body*. Deliberately narrow: parsing and
/// import resolution happen before the body runs (in `Global::resolve_one`,
/// which needs their fingerprints to build the step's content key first), so
/// the body can neither recurse nor do IO — it only reads the interner and
/// the function registry.
pub struct ResolveContext {
    #[allow(dead_code)] // identifies the step; kept for symmetry/debugging
    current: ResolveId,
    core: &'static Global,
}

impl ResolveContext {
    pub fn interner(&self) -> &Interner {
        &self.core.interner
    }

    pub fn func_registry(&self) -> &DashMap<FQ, FuncData> {
        &self.core.func_registry
    }
}

/// Context handed to the type check / monomorphisation phase. It only reads
/// the resolved function registry; dependency edges are registered by the
/// worklist driver in `Global::mono_impl`.
pub struct MonoContext {
    core: &'static Global,
}

impl MonoContext {
    pub fn interner(&self) -> &Interner {
        &self.core.interner
    }

    pub fn func_registry(&self) -> &DashMap<FQ, FuncData> {
        &self.core.func_registry
    }
}

pub struct ExecContext {
    current: ExecId,
    core: &'static Global,
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
        self.core.printer
    }

    pub async fn resolve_all(&self, ids: &[ResolveId]) -> Result<(Vec<Expr>, SymbolTable), ResolveError> {
        // Exec is the root of a resolve tree: no resolutions are in progress yet.
        self.core.resolve_all_impl(StepId::Exec(self.current.clone()), AncestorPath::empty(), ids).await
    }

    pub fn mono(&self, entry: MonoId) -> Result<(), TypeError> {
        self.core.mono_impl(StepId::Exec(self.current.clone()), entry)
    }
}

// Compile-time assertions to ensure contexts are Send (required for tokio::spawn)
const _: fn() = || {
    fn assert_send<T: Send>() {}
    assert_send::<RootContext>();
    assert_send::<ResolveContext>();
    assert_send::<MonoContext>();
    assert_send::<ExecContext>();
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::Path;
    use crate::NoopPrinter;
    use std::fs;
    use tempfile::TempDir;

    fn global() -> &'static Global {
        Box::leak(Box::new(Global::new(&NoopPrinter)))
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
        g.parse_impl(StepId::Root, id).await.unwrap();

        let record = g.bindings.get(&StepId::Parse(id)).expect("parse step is bound");
        let key = record.content_key.expect("leaf content key is bound");
        let fingerprint = record.fingerprint.expect("output fingerprint is bound");
        assert_eq!(record.input_digest, Some(ContentDigest::of("(print 42)\n")));
        assert!(!record.dirty);

        // Unchanged re-demand: the binding stays current.
        g.parse_impl(StepId::Root, id).await.unwrap();
        let again = g.bindings.get(&StepId::Parse(id)).unwrap();
        assert_eq!(again.content_key, Some(key));
        assert_eq!(again.fingerprint, Some(fingerprint));

        // Changed content rebinds the same logical id to a new key (and here
        // a new fingerprint — the AST changed too).
        fs::write(&file, "(print 43)\n").unwrap();
        g.parse_impl(StepId::Root, id).await.unwrap();
        let rebound = g.bindings.get(&StepId::Parse(id)).unwrap();
        assert_ne!(rebound.content_key, Some(key));
        assert_ne!(rebound.fingerprint, Some(fingerprint));
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
        assert!(g.parse_impl(StepId::Root, id).await.is_err());
        assert!(g.parse_impl(StepId::Root, id).await.is_err());

        assert_eq!(g.computed_parse_count(), 1, "the cached error must be served, not re-parsed");
        let record = g.bindings.get(&StepId::Parse(id)).expect("errored parse is still bound");
        assert!(record.content_key.is_some());
        assert!(record.fingerprint.is_none());
    }
}
