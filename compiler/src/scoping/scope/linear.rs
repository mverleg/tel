use ::tel_api::Binding;
use ::tel_api::Identifier;
use ::tel_api::Type;
use ::tel_api::Variable;

use crate::scoping::Scope;

#[derive(Debug)]
pub struct LinearScope {
    pub items: Vec<Variable>,
}

impl LinearScope {
    pub(crate) fn new_root() -> Self {
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