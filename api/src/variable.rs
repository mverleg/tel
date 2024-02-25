use ::std::rc::Rc;

use crate::identifier::Identifier;
use crate::typ::Type;

//TODO @mark: can I make this unsized by putting type last?
#[derive(Debug)]
pub struct Variable {
    pub iden: Identifier,
    pub type_annotation: Option<Type>,
    pub mutable: bool,
}

impl Variable {
    pub fn read(&self, scope: Rc<Scope>) -> VarRead {
        VarRead {
            scope,
            ix,
        }
    }

    pub fn write<'a>(&'a self) -> VarWrite<'a> {
        VarWrite {
            reference: self,
        }
    }
}

#[derive(Debug)]
pub struct VarRead<'a> {
    scope: Rc<Scope>,
    ix: usize,
}

#[derive(Debug)]
pub struct VarWrite<'a> {
    scope: Rc<Scope>,
    ix: usize,
}
