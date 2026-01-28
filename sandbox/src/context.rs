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
        //TODO @mark: lot of clone
        self.graph.register_dependency(self.current.clone(), step.clone());
        Context { current: step, graph: self.graph.clone() }
    }

    pub async fn execute(&self, id: ExecId) -> Result<(), ExecuteError> {
        //TODO @mark: avoid clone?
        crate::execute::execute(&self.dependent(StepId::Exec(id.clone())), id).await
    }

    pub async fn resolve(&self, id: ResolveId) -> Result<(Expr, SymbolTable), ResolveError> {
        crate::resolve::resolve(&self.dependent(StepId::Resolve(id.clone())), id).await
    }
}
