use ::ahash::HashMap;

use ::tel_api::Binding;
use ::tel_api::Identifier;
use ::tel_api::Type;
use ::tel_api::Variable;

use crate::scoping::Scope;

#[derive(Debug)]
pub struct HashScope {
    parent: Option<Box<HashScope>>,
    pub items: HashMap<Identifier, Variable>,
}

impl HashScope {
    fn new_root() -> Self {
        todo!()
    }
}

impl Scope for HashScope {
    fn get_or_insert(
        &mut self, iden: Identifier,
        type_annotation: Option<Type>,
        is_mutable: bool
    ) -> Binding {
        todo!()
    }
}