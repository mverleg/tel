use ::tel_api::Binding;
use ::tel_api::Identifier;
use ::tel_api::Type;
use ::tel_api::Variable;

use crate::scoping::Scope;

#[derive(Debug)]
pub struct LinearScope {
    //TODO @mark: scopes have their own tree structure, even though it matches the AST
    //TODO @mark: for now it seems ot make the code easier (and possibly faster), but might reconsider
    parent: Option<Box<LinearScope>>,
    items: Vec<Variable>,
}

impl LinearScope {
    pub(crate) fn new_root() -> Self {
        LinearScope {
            parent: None,
            items: vec![]
        }
    }
}

impl Scope for LinearScope {
    fn get_or_insert(
        &mut self,
        iden: Identifier,
        type_annotation: Option<Type>,
        mutable: bool
    ) -> Binding {
        if let Some(_parent) = &self.parent {
            todo!()
        }
        for known in &mut self.items {
            if known.iden == iden {
                return known.binding()
            }
        }
        let new_var = Variable {
            iden,
            type_annotation,
            mutable,
        };
        self.items.push(new_var);
        self.items.last().expect("just added, cannot fail").binding()
    }
}
