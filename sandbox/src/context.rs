use crate::common::FQ;
use crate::graph::{ExecId, Graph, ParseId, ResolveId, StepId};
use crate::types::{ExecuteError, Expr, FuncData, ParseError, PreExpr, ResolveError, SymbolTable};
use async_lazy::Cache;
use dashmap::DashMap;
use log::debug;

/// Not actually forced to be singleton, but it's leaked so singleton is encouraged.
pub struct Global {
    graph: Graph,
    parse_cache: Cache<ParseId, PreExpr, ParseError>,
    func_registry: DashMap<FQ, FuncData>,
}

impl Global {
    pub fn new() -> Self {
        Global {
            graph: Graph::new(),
            parse_cache: Cache::new(),
            func_registry: DashMap::new(),
        }
    }
}

impl Global {
    async fn parse_impl(&'static self, caller: StepId, id: ParseId) -> Result<&'static PreExpr, &'static ParseError> {
        debug!("CoreContext::parse_impl: {:?}", id);

        // Always register dependency, regardless of cache hit/miss
        self.graph.register_dependency(caller, StepId::Parse(id.clone()));

        // Clone once for closure (only used on cache miss)
        let id_for_init = id.clone();

        // Get or initialize the cached parse result
        // On cache hit: id is borrowed (no clone), id_for_init is dropped
        // On cache miss: id is consumed, id_for_init is used in closure
        let result = self.parse_cache.get(id, move || async move {
            debug!("CoreContext::parse_impl initializing: {:?}", id_for_init);
            let ctx = ParseContext {
                current: id_for_init.clone(),
                core: self,
            };
            crate::parse::parse(&ctx, id_for_init).await
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

    pub async fn execute(&self, id: ExecId) -> Result<(), ExecuteError> {
        self.core.execute_impl(StepId::Root, id).await
    }
}

pub struct ParseContext {
    current: ParseId,
    core: &'static Global,
}

impl ParseContext {
    pub fn graph(&self) -> &Graph {
        &self.core.graph
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

    pub fn func_registry(&self) -> &DashMap<FQ, FuncData> {
        &self.core.func_registry
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

    pub fn func_registry(&self) -> &DashMap<FQ, FuncData> {
        &self.core.func_registry
    }

    pub async fn resolve_all(&self, ids: &[ResolveId]) -> Result<(Vec<Expr>, SymbolTable), ResolveError> {
        self.core.resolve_all_impl(StepId::Exec(self.current.clone()), ids).await
    }
}

// Compile-time assertions to ensure contexts are Send (required for tokio::spawn)
const _: fn() = || {
    fn assert_send<T: Send>() {}
    assert_send::<RootContext>();
    assert_send::<ParseContext>();
    assert_send::<ResolveContext>();
    assert_send::<ExecContext>();
};
