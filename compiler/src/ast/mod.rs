use serde::Serialize;

pub use crate::ast::assign::AssignmentDest;
pub use crate::ast::assign::AssignmentKw;
pub use crate::ast::assign::Assignments;
pub use crate::ast::function::Closure;
pub use crate::ast::function::Invoke;
pub use crate::ast::types::Enum;
pub use crate::ast::types::Struct;
use tel_ast::op::{BinOpCode, UnaryOpCode};
use tel_common::SString;

mod assign;
mod types;
mod function;

#[derive(Debug, PartialEq, Serialize)]
pub struct Ast {
    pub blocks: Box<[Block]>,
}

#[derive(Debug, PartialEq, Serialize)]
pub enum Block {
    Assigns(Assignments),
    Expression(Expr),
    Return(Expr),
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

pub fn vec_and<T>(mut items: Vec<T>, addition: Option<T>) -> Vec<T> {
    if let Some(addition) = addition {
        items.push(addition);
    }
    items
}
