use serde::Serialize;
use crate::ast::{AssignmentDest, Identifier, Type};

#[derive(Debug, PartialEq, Serialize)]
pub struct Struct {
    pub iden: Identifier,
    pub fields: Vec<(Identifier, Type)>,
    pub generics: Box<[AssignmentDest]>,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct Enum {
    pub iden: Identifier,
    pub variants: Box<[EnumVariant]>,
    pub generics: Box<[AssignmentDest]>,
}

#[derive(Debug, PartialEq, Serialize)]
pub enum EnumVariant {
    Struct(Struct),
    Enum(Enum),
    Existing(Type),
}

