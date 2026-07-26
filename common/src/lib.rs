//! High level representation
//! * Identifiers linked
//! * Type checked
//! * Easy to do transformations on

mod identifier;
mod error;
pub mod parse_util;
pub mod spec_anchors;

pub use crate::error::TelErr;
pub use crate::identifier::Identifier;
pub use smartstring::alias::String as SString;
