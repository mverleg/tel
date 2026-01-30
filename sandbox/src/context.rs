use crate::graph::{ExecId, Graph, ParseId, ResolveId, StepId};
use crate::types::{ExecuteError, Expr, ParseError, PreExpr, ResolveError, SymbolTable};
use dashmap::DashMap;
use log::debug;
use ::std::sync::Arc;

pub struct CoreContext {
    graph: Graph,
    parse_cache: DashMap<ParseId, Vec<u8>>,
}

impl CoreContext {
    pub fn new() -> Self {
        CoreContext {
            graph: Graph::new(),
            parse_cache: DashMap::new(),
        }
    }
}

#[derive(Clone)]
pub struct RefContext {
    current: StepId,
    core: Arc<CoreContext>,
}

impl RefContext {
    pub fn new() -> Self {
        RefContext {
            current: StepId::Root,
            core: Arc::new(CoreContext::new()),
        }
    }

    pub fn graph(&self) -> &Graph {
        &self.core.graph
    }

    fn dependent(&self, step: StepId) -> RefContext {
        self.core.graph.register_dependency(self.current.clone(), step.clone());
        RefContext {
            current: step,
            core: self.core.clone(),
        }
    }

    pub async fn parse(&self, id: ParseId) -> Result<PreExpr, ParseError> {
        debug!("RefContext::parse: {:?}", id);

        if let Some(cached_bytes) = self.core.parse_cache.get(&id) {
            debug!("RefContext::parse cache hit: {:?}", id);
            return postcard::from_bytes(&cached_bytes)
                .map_err(|e| ParseError::IoError(
                    std::io::Error::new(std::io::ErrorKind::InvalidData, format!("Cache deserialization failed: {}", e))
                ));
        }

        debug!("RefContext::parse cache miss: {:?}", id);
        let result = crate::parse::parse(&self.dependent(StepId::Parse(id.clone())), id.clone()).await?;

        let serialized = postcard::to_allocvec(&result)
            .map_err(|e| ParseError::IoError(
                std::io::Error::new(std::io::ErrorKind::InvalidData, format!("Cache serialization failed: {}", e))
            ))?;

        self.core.parse_cache.insert(id, serialized);

        Ok(result)
    }

    pub async fn execute(&self, id: ExecId) -> Result<(), ExecuteError> {
        debug!("RefContext::execute: {:?}", id);
        crate::execute::execute(&self.dependent(StepId::Exec(id.clone())), id).await
    }

    pub async fn resolve_all(&self, ids: &[ResolveId]) -> Result<(Vec<Expr>, SymbolTable), ResolveError> {
        debug!("RefContext::resolve_all x{}: {:?}", ids.len(), ids);

        if ids.is_empty() {
            return Ok((Vec::new(), SymbolTable::new()));
        }

        let n = ids.len();
        if n == 1 {
            // Single item - just resolve it directly
            let id = ids[0].clone();
            let (expr, table) = crate::resolve::resolve(&self.dependent(StepId::Resolve(id.clone())), id).await?;
            return Ok((vec![expr], table));
        }

        // Spawn tasks for items 0..N-1
        let mut handles = Vec::new();
        for i in 0..n-1 {
            let id = ids[i].clone();
            let ctx = self.clone();
            let handle = tokio::spawn(async move {
                crate::resolve::resolve(&ctx.dependent(StepId::Resolve(id.clone())), id).await
            });
            handles.push(handle);
        }

        // Use current task for the Nth item
        let last_id = ids[n-1].clone();
        let last_result = crate::resolve::resolve(&self.dependent(StepId::Resolve(last_id.clone())), last_id).await?;

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
            // Merge symbol tables by appending
            merged_table.vars.extend(table.vars);
            merged_table.funcs.extend(table.funcs);
        }

        Ok((exprs, merged_table))
    }
}

// Compile-time assertion to ensure RefContext is Send (required for tokio::spawn)
const _: fn() = || {
    fn assert_send<T: Send>() {}
    assert_send::<RefContext>();
};
