use ::ahash::HashMap;

#[derive(Debug)]
pub struct ScopeEntry {
    pub name: Identifier,
}

#[derive(Debug)]
pub struct LinearScope {
    pub items: Vec<Variable>,
}

#[derive(Debug)]
pub struct MapScope {
    pub items: HashMap<Identifier, Variable>,
}
