use crate::graph::{ExecId, Graph, ResolveId};
use crate::types::{ExecuteError, Expr, ResolveError, SymbolTable};

pub struct Context {
    graph: Graph,
}

impl Context {
    pub fn new() -> Self {
        Context { graph: Graph::new() }
    }

    pub async fn execute(&self, id: ExecId) -> Result<(), ExecuteError> {
        crate::execute::execute(&id.main_func.as_str()).await
    }

    pub async fn resolve(&self, id: ResolveId) -> Result<(Expr, SymbolTable), ResolveError> {
        crate::resolve::resolve(&id.func_name.as_str()).await
    }
}
