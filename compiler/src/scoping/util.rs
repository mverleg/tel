use ::ahash::HashMap;

use ::tel_api::Identifier;
use ::tel_api::Variable;

//TODO @mark: compare impl performances
pub trait Scope {
    fn new() -> LinearScope {
        return LinearScope::new_child()
    }
}

#[derive(Debug)]
pub struct ScopeEntry {
    pub name: Identifier,
}

#[derive(Debug)]
pub struct LinearScope {
    pub items: Vec<Variable>,
}

impl LinearScope {
    fn new_child() -> Self {
        LinearScope {
            items: vec![]
        }
    }
}

impl Scope for LinearScope {}

#[derive(Debug)]
pub struct MapScope {
    pub items: HashMap<Identifier, Variable>,
}

impl MapScope {
    fn new_child() -> Self {
        todo!()
    }
}

impl Scope for MapScope {}
