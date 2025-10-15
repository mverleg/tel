use crate::ast::op::BinOpCode;
use crate::ast::Expr;
use crate::ast::identifier::Identifier;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignmentKw {
    None,
    /// For functions, assigning to a name that exists outside the function creates a local
    /// shadow instead. Using the 'outer' keyword changes this to reuse the outer name.
    /// (It will have the same mutability as the outer variable).
    Outer,
    /// For local blocks like loops, assigning to a name from the outer scope reuses that
    /// variable by default. Using the 'local' keyword changes this to immutably shadow
    /// the outer variable instead.
    Local,
    /// Like 'local', but creates a mutable variable.
    Mut,
}

#[derive(Debug, PartialEq)]
pub struct Assignments {
    pub dest: Box<[AssignmentDest]>,
    pub op: Option<BinOpCode>,
    pub value: Box<Expr>,
}

#[derive(Debug, PartialEq)]
pub struct AssignmentDest {
    pub kw: AssignmentKw,
    pub target: Identifier,
    pub typ: Option<Type>,
}
