use crate::graph::{ExecId, Graph, ResolveId, StepId};
use crate::types::{ExecuteError, Expr, ResolveError, SymbolTable};
use ::std::rc::Rc;

pub struct Context {
    current: StepId,
    graph: Rc<Graph>,
}

impl Context {
    pub fn new() -> Self {
        Context { current: StepId::Root, graph: Rc::new(Graph::new()) }
    }

    fn dependent(&self, step: StepId) -> Context {
        self.graph.register_dependency(self.current, step.clone());
        Context { current: step, graph: self.graph.clone() }
    }

    pub async fn execute(&self, id: ExecId) -> Result<(), ExecuteError> {
        crate::execute::execute(self.dependent(), &id.main_func.as_str()).await
    }

    pub async fn resolve(&self, id: ResolveId) -> Result<(Expr, SymbolTable), ResolveError> {
        crate::resolve::resolve(&id.func_name.as_str()).await
    }
}
