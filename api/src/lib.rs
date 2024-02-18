pub use ::log;

/// Source-level representation.
/// * Supports all valid language inputs.
/// * Does not encode formatting, but keeps debug info.
/// * Variables have been linked.
/// * Useful for linting, IDE integration or fuzzing.
pub mod lang;

/// Compiled intermediary representation.
/// * Not target specific.
/// * Not directly mappable to source constructs, complex constructs have been lowered.
/// * Some minimal optimizations may be done (but most will be during target codegen).
/// * Useful as input for code generation backends.
pub mod ir;
