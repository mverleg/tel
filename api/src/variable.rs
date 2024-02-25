use crate::identifier::Identifier;
use crate::Ix;
use crate::typ::Type;

//TODO @mark: can I make this unsized by putting type last?
#[derive(Debug)]
pub struct Variable {
    ix: Ix,
    pub iden: Identifier,
    pub type_annotation: Option<Type>,
    pub mutable: bool,
}

impl Variable {
    pub fn read(&self) -> VarRead {
        VarRead {
            ix: self.ix
        }
    }

    pub fn write(&self) -> VarWrite {
        VarWrite {
            ix: self.ix
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct VarRead {
    ix: Ix,
}

#[derive(Debug, Clone, Copy)]
pub struct VarWrite {
    ix: Ix,
}
