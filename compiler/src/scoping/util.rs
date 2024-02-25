use ::ahash::HashMap;

use ::tel_api::Binding;
use ::tel_api::Identifier;
use ::tel_api::Variable;
use ::tel_api::Type;

//TODO @mark: compare impl performances
pub trait Scope {
    fn new() -> LinearScope {
        return LinearScope::new_child()
    }

    fn get_or_insert(
        &mut self,
        iden: &Identifier,
        typ_annotation: &Option<Type>,
        is_mutable: bool,
    ) -> Binding;
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

impl Scope for LinearScope {
    fn get_or_insert(
        &mut self,
        iden: &Identifier,
        typ_annotation: &Option<Type>,
        is_mutable: bool
    ) -> Binding {
        todo!()
    }
}

#[derive(Debug)]
pub struct MapScope {
    pub items: HashMap<Identifier, Variable>,
}

impl MapScope {
    fn new_child() -> Self {
        todo!()
    }
}

impl Scope for MapScope {
    fn get_or_insert(
        &mut self, iden: &Identifier,
        typ_annotation: &Option<Type>,
        is_mutable: bool
    ) -> Binding {
        todo!()
    }
}
