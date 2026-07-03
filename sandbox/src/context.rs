use crate::common::{Interner, FQ};
use crate::graph::{ExecId, Graph, ParseId, ResolveId, StepId};
use crate::types::{ExecuteError, Expr, FuncData, ParseError, PreExpr, ResolveError, SymbolTable};
use crate::Printer;
use async_lazy::Cache;
use dashmap::DashMap;
use log::debug;
use std::time::Instant;

/// State of a resolution process, used for cycle detection
#[derive(Clone)]
pub enum ResolutionState {
    InProgress { started_at: Instant },
    Completed,
}

/// A content-addressed digest of a source file's bytes.
///
/// Parse results are cached under this digest instead of the file path, which
/// is what makes the cache safe across source changes (see
/// docs/cache-invalidation-problem.md):
/// - file bytes change  -> different digest -> different key -> re-parse
///   (no stale result served for the old key), and
/// - file reverted to previously-seen bytes (edit-then-revert, git
///   branch-switch) -> identical digest -> cache hit (Scenario A).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ContentDigest(u64);

impl ContentDigest {
    pub fn of(source: &str) -> ContentDigest {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        source.hash(&mut hasher);
        ContentDigest(hasher.finish())
    }
}

/// Not actually forced to be singleton, but it's leaked so singleton is encouraged.
pub struct Global {
    graph: Graph,
    interner: Interner,
    /// Content-addressed parse cache: `content digest -> parse result`.
    ///
    /// Append-only and immutable, so it is safe to keep across runs: a digest
    /// always maps to the same result, or is absent (recompute). Invalidation
    /// is implicit -- a changed file simply produces a new key.
    content_store: Cache<ContentDigest, PreExpr, ParseError>,
    func_registry: DashMap<FQ, FuncData>,
    resolution_states: DashMap<FQ, ResolutionState>,
    printer: &'static dyn Printer,
}

impl Global {
    pub fn new(printer: &'static dyn Printer) -> Self {
        Global {
            graph: Graph::new(),
            interner: Interner::new(),
            content_store: Cache::new(),
            func_registry: DashMap::new(),
            resolution_states: DashMap::new(),
            printer,
        }
    }

    /// Number of distinct source contents parsed and cached so far (by digest).
    /// Lets callers observe cross-run cache reuse: an unchanged or reverted file
    /// does not grow this count on a subsequent run.
    pub fn cached_parse_count(&self) -> usize {
        self.content_store.len()
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
            // Without content there is no digest to key on, so IO errors are not
            // stored in the content store. Leak a 'static error to satisfy the
            // signature (the compiler already leaks its Global/interner).
            Err(err) => return Err(Box::leak(Box::new(ParseError::from(err)))),
        };

        let digest = ContentDigest::of(&source);
        let result = self.content_store.get(digest, move || async move {
            debug!("CoreContext::parse_impl parsing {:?} (digest {:?})", path, digest);
            crate::parse::tokenize_and_parse(&source, path)
        }).await;

        // Return borrowed reference to cached result
        result.as_ref()
    }

    async fn resolve_all_impl(&'static self, caller: StepId, ids: &[ResolveId]) -> Result<(Vec<Expr>, SymbolTable), ResolveError> {
        debug!("CoreContext::resolve_all_impl x{}: {:?}", ids.len(), ids);

        if ids.is_empty() {
            return Ok((Vec::new(), SymbolTable::new()));
        }

        let n = ids.len();
        if n == 1 {
            let id = ids[0].clone();
            self.graph.register_dependency(caller, StepId::Resolve(id.clone()));
            let ctx = ResolveContext {
                current: id.clone(),
                core: self,
            };
            let (expr, table) = crate::resolve::resolve(&ctx, id).await?;
            return Ok((vec![expr], table));
        }

        // Spawn tasks for items 0..N-1
        let mut handles = Vec::new();
        let core = self;
        for i in 0..n-1 {
            let id = ids[i].clone();
            let handle = tokio::spawn(async move {
                core.graph.register_dependency(StepId::Root, StepId::Resolve(id.clone()));
                let ctx = ResolveContext { current: id.clone(), core };
                crate::resolve::resolve(&ctx, id).await
            });
            handles.push(handle);
        }

        // Use current task for the Nth item
        let last_id = ids[n-1].clone();
        self.graph.register_dependency(caller.clone(), StepId::Resolve(last_id.clone()));
        let ctx = ResolveContext {
            current: last_id.clone(),
            core: self,
        };
        let last_result = crate::resolve::resolve(&ctx, last_id).await?;

        // Wait for all spawned tasks
        let mut all_results = Vec::with_capacity(n);
        for handle in handles {
            let result = handle.await
                .map_err(|e| ResolveError::JoinError(format!("Task join failed: {}", e)))?;
            all_results.push(result?);
        }
        all_results.push(last_result);

        // Build result vectors
        let mut exprs = Vec::with_capacity(n);
        let mut merged_table = SymbolTable::new();

        for (expr, table) in all_results {
            exprs.push(expr);
            merged_table.vars.extend(table.vars);
            // funcs are now in global registry, no need to merge
        }

        Ok((exprs, merged_table))
    }

    async fn execute_impl(&'static self, caller: StepId, id: ExecId) -> Result<(), ExecuteError> {
        debug!("CoreContext::execute_impl: {:?}", id);
        self.graph.register_dependency(caller, StepId::Exec(id.clone()));
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

pub struct ResolveContext {
    current: ResolveId,
    core: &'static Global,
}

impl ResolveContext {
    pub fn graph(&self) -> &Graph {
        &self.core.graph
    }

    pub fn interner(&self) -> &Interner {
        &self.core.interner
    }

    pub fn func_registry(&self) -> &DashMap<FQ, FuncData> {
        &self.core.func_registry
    }

    //TODO @mark: not pub?
    pub fn resolution_states(&self) -> &DashMap<FQ, ResolutionState> {
        &self.core.resolution_states
    }

    pub async fn parse(&self, id: ParseId) -> Result<&'static PreExpr, &'static ParseError> {
        self.core.parse_impl(StepId::Resolve(self.current.clone()), id).await
    }

    pub async fn resolve_all(&self, ids: &[ResolveId]) -> Result<(Vec<Expr>, SymbolTable), ResolveError> {
        self.core.resolve_all_impl(StepId::Resolve(self.current.clone()), ids).await
    }
}

impl Clone for ResolveContext {
    fn clone(&self) -> Self {
        ResolveContext {
            current: self.current.clone(),
            core: self.core,
        }
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

    pub fn printer(&self) -> &dyn Printer {
        self.core.printer
    }

    pub async fn resolve_all(&self, ids: &[ResolveId]) -> Result<(Vec<Expr>, SymbolTable), ResolveError> {
        self.core.resolve_all_impl(StepId::Exec(self.current.clone()), ids).await
    }
}

// Compile-time assertions to ensure contexts are Send (required for tokio::spawn)
const _: fn() = || {
    fn assert_send<T: Send>() {}
    assert_send::<RootContext>();
    assert_send::<ResolveContext>();
    assert_send::<ExecContext>();
};
