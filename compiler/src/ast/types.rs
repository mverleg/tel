use crate::ast::AssignmentDest;
use crate::ast::identifier::Identifier;

#[derive(Debug, PartialEq)]
pub enum Typ {
    Struct(Struct),
    Enum(Enum),
}

#[derive(Debug, PartialEq)]
pub struct Struct {
    pub iden: Identifier,
    pub fields: Vec<(Identifier, Typ)>,
    pub generics: Box<[AssignmentDest]>,
}

#[derive(Debug, PartialEq)]
pub struct Enum {
    pub iden: Identifier,
    pub variants: Box<[Typ]>,
    pub generics: Box<[AssignmentDest]>,
}

#[derive(Debug, PartialEq)]
pub struct EnumVariant {
    //TODO @mark: source may not have tag? but generated code should
    label: Identifier,
    data: Typ,
}
