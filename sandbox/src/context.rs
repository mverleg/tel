use crate::graph::Graph;

pub struct Context {
    graph: Graph,
}

impl Context {
    pub fn new() -> Self {
        Context { graph: Graph::new() }
    }
}