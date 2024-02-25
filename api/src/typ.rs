use ::serde::Serialize;

use crate::identifier::Identifier;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Type {
    //TODO @mark:
    pub iden: Identifier,
    pub generics: Box<[Type]>,
}
