#![feature(lazy_cell)]

//! Source-level representation.
//! * Supports all valid language inputs.
//! * Does not encode formatting, but keeps debug info.
//! * Variables have been linked.
//! * Useful for linting, IDE integration or fuzzing.
//! * All variable scopes should be correct, but types aren't checked.

use ::std::mem::size_of;

use ::log;
use ::serde::Serialize;

pub use self::identifier::Identifier;
pub use self::typ::Type;
pub use self::variable::Variables;
pub use self::variable::VariableData;
pub use self::variable::Variable;

pub mod op;
mod variable;
mod identifier;
mod typ;

//TODO @mark: replace all usize in structs and enums by Ix if ~1kkk is enough
pub type Ix = u32;

const _: () = assert!(size_of::<Ix>() <= size_of::<usize>(), "index is too large for this platform");

#[derive(Debug, Serialize)]
pub enum Expr {
    Num(f64),
    Read(Variable),
    Invoke { iden: Variable, args: Vec<Expr> },
}

#[derive(Debug, Serialize)]
pub struct Assignment {
    pub var: Variable,
    pub value: Expr,
}

#[derive(Debug, Serialize)]
pub struct TelFile {}
