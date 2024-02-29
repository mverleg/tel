use crate::identifier::Identifier;
use crate::Ix;
use crate::typ::Type;

/// All variables per TelFile are owned by this central buffer.
/// In the tree, lightweight indices are used, and this class is passed explicitly.
#[derive(Debug)]
pub struct Variables {
    data: Vec<Variable>,
}

impl Variables {
    pub fn new() -> Self {
        Variables {
            data: Vec::new()
        }
    }

    pub fn add(
        &mut self,
        iden: Identifier,
        type_annotation: Option<Type>,
        mutable: bool,
    ) {
        let new_ix = self.data.len();
        self.data.push(Variable {
            ix: new_ix as Ix,
            iden,
            type_annotation,
            mutable,
        })
    }
}

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

    pub fn write(&self, variables: &mut Variables) -> VarWrite {
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
