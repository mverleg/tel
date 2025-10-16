use serde::Serialize;

use crate::assign::AssignmentDest;
use crate::function::{Closure, Invoke};
use crate::op::{BinOpCode, UnaryOpCode};
use crate::Block;
use tel_common::SString;

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

