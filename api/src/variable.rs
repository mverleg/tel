use std::mem::size_of;
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
    ) -> VarRef {
        let new_ix = self.data.len();
        self.data.push(Variable {
            ix: new_ix as Ix,
            iden,
            type_annotation,
            mutable,
        });
        self.data.last_mut()
            .expect("just inserted, always has value")
            .refer()
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
    pub fn refer(&self) -> VarRef {
        VarRef {
            ix: self.ix
        }
    }
}

/// This is implicitly linked to a specific Variables instance by being in the same TelFile.
/// There is no safety check for this, calling code must pass the right Variables around.
#[derive(Debug, Clone, Copy)]
pub struct VarRef {
    ix: Ix,
}

const _: () = assert!(size_of::<VarRef>() == 4);
