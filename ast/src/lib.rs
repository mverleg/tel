//! Source-level representation.
//! * Supports all valid language inputs.
//! * Does not encode formatting, but keeps debug info.
//! * Useful for linting, IDE integration or fuzzing.
//! * All variable scopes should be correct, but types aren't checked.

use std::mem::size_of;

use serde::Serialize;

pub use self::typ::Type;
pub use self::types::Enum;
pub use self::types::EnumVariant;
pub use self::types::Struct;
pub use self::variable::Variable;
pub use self::variable::VariableData;
pub use self::variable::Variables;
pub use self::error::ParseErr;
pub use self::expr::Expr;
pub use self::block::Block;
pub use self::assign::Assignments;
pub use self::assign::AssignmentKw;
pub use self::assign::AssignmentDest;
pub use self::block::Ast;
pub use self::function::Invoke;
pub use self::function::Closure;
pub use self::op::UnaryOpCode;
pub use self::op::BinOpCode;

pub mod op;
mod expr;
mod block;
mod variable;
mod typ;
mod error;
mod assign;
mod types;
mod function;

//TODO @mark: replace all usize in structs and enums by Ix if ~1kkk is enough
/// Negative indices are used for built-ins
pub type Ix = i32;

const _: () = assert!(size_of::<Ix>() <= size_of::<usize>(), "index is too large for this platform");

#[derive(Debug, Serialize)]
pub struct Assignment {
    pub var: Variable,
    pub value: Expr,
}

#[derive(Debug, Serialize)]
pub struct TelFile {}
