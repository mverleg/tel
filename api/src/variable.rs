use crate::identifier::Identifier;
use crate::typ::Type;

//TODO @mark: mvoe
#[derive(Debug)]
pub struct Variable {
    pub iden: Identifier,
    pub type_annotation: Option<Type>,
    pub mutable: bool,
}

impl Variable {
    pub fn read<'a>(&'a self) -> VarRead<'a> {
        VarRead {
            reference: self,
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
    reference: &'a Variable,
}

#[derive(Debug)]
pub struct VarWrite<'a> {
    reference: &'a Variable,
}