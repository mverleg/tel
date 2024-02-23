use ::ahash::HashMap;

use crate::ast::Identifier;

//TODO @mark: mvoe
#[derive(Debug)]
pub struct Variable {
    pub name: Identifier,
    pub mutable: bool,
}

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
