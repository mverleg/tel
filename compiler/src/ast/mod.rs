use ::serde::Serialize;
use ::smartstring::alias::String as SString;

pub use crate::ast::assign::AssignmentDest;
pub use crate::ast::assign::AssignmentKw;
pub use crate::ast::assign::Assignments;
pub use crate::ast::function::Closure;
pub use crate::ast::function::Invoke;
use crate::ast::op::{BinOpCode, UnaryOpCode};
pub use crate::ast::types::Enum;
pub use crate::ast::types::EnumVariant;
pub use crate::ast::types::Struct;

mod assign;
mod types;
mod function;
mod op;
mod identifier;

#[derive(Debug, PartialEq)]
pub struct Ast {
    pub blocks: Box<[Block]>,
}

#[derive(Debug, PartialEq)]
pub enum Block {
    Assigns(Assignments),
    Expression(Expr),
    Return(Expr),
    Struct(Struct),
    Enum(Enum),
}

#[derive(Debug, PartialEq)]
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

pub fn vec_and<T>(mut items: Vec<T>, addition: Option<T>) -> Vec<T> {
    if let Some(addition) = addition {
        items.push(addition);
    }
    items
}
