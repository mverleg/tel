use ::serde::Serialize;
use ::smartstring::alias::String as SString;

pub use ::tel_api::ops::BinOpCode;
pub use ::tel_api::ops::UnaryOpCode;

pub use crate::ast::assign::AssignmentDest;
pub use crate::ast::assign::AssignmentKw;
pub use crate::ast::assign::Assignments;
pub use crate::ast::common::Type;

pub use self::identifier::Identifier;

mod identifier;
mod common;
mod assign;

#[derive(Debug, PartialEq, Serialize)]
pub struct Ast {
    pub blocks: Box<[Block]>,
}

#[derive(Debug, PartialEq, Serialize)]
pub enum Block {
    Assigns(Assignments),
    Expression(Expr),
    Struct(Struct),
    Enum(Enum),
}

#[derive(Debug, PartialEq, Serialize)]
pub enum Expr {
    Num(f64),
    Text(SString),
    /// Binary operation, e.g. 'x+y', 'x==y', 'x or y'. Parser handled precedence.
    BinOp(BinOpCode, Box<Expr>, Box<Expr>),
    /// Unary operation, '!x' or '-x'
    UnaryOp(UnaryOpCode, Box<Expr>),
    /// Variable read or function call.
    Invoke(Invoke),
    /// Dot-access a field or method, like x.a or x.f(a)
    Dot(Box<Expr>, Invoke),
    Closure(Closure),
    /// If, then, else (empty else is same as no else)
    If(Box<[(Expr, Box<[Block]>)]>, Option<Box<[Block]>>),
    While(Box<Expr>, Box<[Block]>),
    ForEach(AssignmentDest, Box<Expr>, Box<[Block]>),
}

/// Can be a variable read or a function call. A function call without () cannot be differentiated from
/// a function call by the parser, this must be done later.
#[derive(Debug, PartialEq, Serialize)]
pub struct Invoke {
    pub iden: Identifier,
    //TODO @mark: to smallvec or something:
    pub args: Box<[Expr]>,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct Closure {
    pub blocks: Box<[Block]>,
    pub params: Box<[AssignmentDest]>,
}

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

pub fn vec_and<T>(mut items: Vec<T>, addition: Option<T>) -> Vec<T> {
    if let Some(addition) = addition {
        items.push(addition);
    }
    items
}
