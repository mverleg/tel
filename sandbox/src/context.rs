use crate::graph::{ExecId, Graph, ParseId, ResolveId, StepId};
use crate::types::{ExecuteError, Expr, ParseError, PreExpr, ResolveError, SymbolTable};
use log::debug;
use ::std::sync::Arc;

#[derive(Clone)]
pub struct Context {
    current: StepId,
    graph: Arc<Graph>,
}

impl Context {
    pub fn new() -> Self {
        Context { current: StepId::Root, graph: Arc::new(Graph::new()) }
    }

    pub fn graph(&self) -> &Graph {
        &self.graph
    }

    fn dependent(&self, step: StepId) -> Context {
        //TODO @mark: lot of clone
        self.graph.register_dependency(self.current.clone(), step.clone());
        Context { current: step, graph: self.graph.clone() }
    }

    pub async fn parse(&self, id: ParseId) -> Result<PreExpr, ParseError> {
        debug!("Context::parse: {:?}", id);
        crate::parse::parse(&self.dependent(StepId::Parse(id.clone())), id).await
    }

    pub async fn execute(&self, id: ExecId) -> Result<(), ExecuteError> {
        debug!("Context::execute: {:?}", id);
        //TODO @mark: avoid clone?
        crate::execute::execute(&self.dependent(StepId::Exec(id.clone())), id).await
    }

    pub async fn resolve(&self, id: ResolveId) -> Result<(Expr, SymbolTable), ResolveError> {
        debug!("Context::resolve: {:?}", id);
        crate::resolve::resolve(&self.dependent(StepId::Resolve(id.clone())), id).await
    }

    pub async fn resolve_all(&self, ids: &[ResolveId]) -> Result<(Vec<Expr>, SymbolTable), ResolveError> {
        debug!("Context::resolve_all x{}: {:?}", ids.len(), ids);

        if ids.is_empty() {
            return Ok((Vec::new(), SymbolTable::new()));
        }

        let n = ids.len();
        if n == 1 {
            // Single item - just resolve it directly
            let (expr, table) = self.resolve(ids[0].clone()).await?;
            return Ok((vec![expr], table));
        }

        // Spawn tasks for items 0..N-1
        let mut handles = Vec::new();
        for i in 0..n-1 {
            let id = ids[i].clone();
            let ctx = self.clone();
            let handle = tokio::spawn(async move {
                ctx.resolve(id).await
            });
            handles.push(handle);
        }

        // Use current task for the Nth item
        let last_result = self.resolve(ids[n-1].clone()).await?;

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

// Compile-time assertion to ensure Context is Send (required for tokio::spawn)
const _: fn() = || {
    fn assert_send<T: Send>() {}
    assert_send::<Context>();
};
