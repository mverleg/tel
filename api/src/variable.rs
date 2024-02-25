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
    pub fn binding<'a>(&'a self) -> Binding<'a> {
        Binding {
            reference: self,
        }
    }
}

#[derive(Debug)]
pub struct Binding<'a> {
    reference: &'a Variable,
}