//! Compiled intermediary representation.
//! * Not target specific yet.
//! * Not directly mappable to source constructs, complex constructs have been lowered.
//! * Some optimizations may be done, but most will be during target codegen.
//! * Useful as input for code generation backends.
//! * Type checking has completed.

pub use ::log;
