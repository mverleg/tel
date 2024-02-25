//! Scope contains the variables in a block, like a file, function or loop.
//! It is a tree structure, where each scope refers to the parent.
//!
//! This implementation stores the values linearly. There are alternative ways,
//! most obviously a hashmap for fast lookup, but also a mix of the two.

use ::tel_api::VarRead;
use ::tel_api::Identifier;
use ::tel_api::Type;
use ::tel_api::Variable;

#[derive(Debug)]
pub struct Scope {
    //TODO @mark: scopes have their own tree structure, even though it matches the AST
    //TODO @mark: for now it seems ot make the code easier (and possibly faster), but might reconsider
    parent: Option<Box<Scope>>,
    items: Vec<Variable>,
}

impl Scope {
    pub(crate) fn new_root() -> Self {
        Scope {
            parent: None,
            items: vec![]
        }
    }
}

impl Scope {
    fn get_or_insert(
        &mut self,
        iden: &Identifier,
        type_annotation: Option<&Type>,
        mutable: bool
    ) -> VarRead {
        if let Some(_parent) = &self.parent {
            todo!()
        }
        for known in &mut self.items {
            if known.iden == *iden {
                return known.read()
            }
        }
        let new_var = Variable {
            iden: iden.clone(),
            type_annotation: type_annotation.cloned(),
            mutable,
        };
        self.items.push(new_var);
        self.items.last().expect("just added, cannot fail").read()
    }
}
