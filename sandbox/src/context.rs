use crate::graph::{Graph, ResolveId};
use crate::types::{Expr, ResolveError, SymbolTable};

pub struct Context {
    graph: Graph,
}

impl Context {
    pub fn new() -> Self {
        Context { graph: Graph::new() }
    }

    pub async fn resolve(&self, id: ResolveId) -> Result<(Expr, SymbolTable), ResolveError> {
        crate::resolve::resolve(&id.func_name.as_str()).await
    }
}
