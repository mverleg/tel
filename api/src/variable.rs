use ::std::mem::size_of;
use ::std::ops::Index;

use ::serde::Serialize;

use crate::identifier::Identifier;
use crate::Ix;
use crate::typ::Type;

/// All variables per TelFile are owned by this central buffer.
/// In the tree, lightweight indices are used, and this class is passed explicitly.
#[derive(Debug)]
pub struct Variables {
    data: Vec<VariableData>,
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
    ) -> Variable {
        let new_ix = self.data.len();
        debug_assert!(new_ix < (Ix::MAX as usize), "maximum number of variables per file exceeded ({new_ix})");
        self.data.push(VariableData {
            ix: new_ix as Ix,
            iden,
            type_annotation,
            mutable,
        });
        self.data[new_ix].refer()
    }
}

impl Index<Variable> for Variables {
    type Output = VariableData;

    fn index(&self, var: Variable) -> &Self::Output {
        &self.data[var.ix as usize]
    }
}

//TODO @mark: can I make this unsized by putting type last?
#[derive(Debug)]
pub struct VariableData {
    ix: Ix,
    pub iden: Identifier,
    pub type_annotation: Option<Type>,
    pub mutable: bool,
}

impl VariableData {
    pub fn refer(&self) -> Variable {
        Variable {
            ix: self.ix
        }
    }
}

/// This is implicitly linked to a specific Variables instance by being in the same TelFile.
/// There is no safety check for this, calling code must pass the right Variables around.
/// Note: PartialEq only makes sense within a TelFile
#[derive(Debug, Serialize, PartialEq, Clone, Copy)]
#[serde(transparent)]
pub struct Variable {
    ix: Ix,
}

const _: () = assert!(size_of::<Variable>() == 4);

impl Variable {
    // Indexing without bound check is safe if we don't do anything weird,
    // because `variables` never shrinks and Variable is only created on insertion.

    pub fn iden(self, variables: &Variables) -> &Identifier {
        return &variables[self].iden
    }

    pub fn type_annotation(self, variables: &Variables) -> Option<&Type> {
        return variables[self].type_annotation.as_ref()
        //TODO @mark: does as_ref have overhead here?
    }

    pub fn mutable(self, variables: &Variables) -> &bool {
        return &variables[self].mutable
    }

}