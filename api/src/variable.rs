use crate::identifier::Identifier;
use crate::typ::Type;

//TODO @mark: mvoe
#[derive(Debug)]
pub struct Variable {
    pub name: Identifier,
    pub type_annotation: Option<Type>,
    pub mutable: bool,
}

#[derive(Debug)]
pub struct Binding {
    //TODO @mark:
}