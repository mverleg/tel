use ::ahash::HashMap;

use ::tel_api::Binding;
use ::tel_api::Identifier;
use ::tel_api::Type;
use ::tel_api::Variable;

use crate::scoping::Scope;

#[derive(Debug)]
pub struct HashScope {
    pub items: HashMap<Identifier, Variable>,
}

impl HashScope {
    fn new_child() -> Self {
        todo!()
    }
}

impl Scope for HashScope {
    fn get_or_insert(
        &mut self, iden: &Identifier,
        typ_annotation: &Option<Type>,
        is_mutable: bool
    ) -> Binding {
        todo!()
    }
}
