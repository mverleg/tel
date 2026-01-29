use crate::graph::{ExecId, Graph, ParseId, ResolveId, StepId};
use crate::types::{ExecuteError, Expr, ParseError, PreExpr, ResolveError, SymbolTable};
use log::debug;
use ::std::rc::Rc;

pub struct Context {
    current: StepId,
    graph: Rc<Graph>,
}

impl Context {
    pub fn new() -> Self {
        Context { current: StepId::Root, graph: Rc::new(Graph::new()) }
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
}
