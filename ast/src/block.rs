use serde::Serialize;

use crate::assign::Assignments;
use crate::types::{Enum, Struct};
use crate::Expr;

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
