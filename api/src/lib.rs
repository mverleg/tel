//! Source-level representation.
//! * Supports all valid language inputs.
//! * Does not encode formatting, but keeps debug info.
//! * Variables have been linked.
//! * Useful for linting, IDE integration or fuzzing.
//! * All variable scopes should be correct, but types aren't checked.

pub use ::log;
use ::serde::Serialize;

pub mod ops;

#[derive(Debug, Serialize)]
pub struct TelFile {
}
