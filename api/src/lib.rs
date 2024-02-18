pub use ::log;

///! Source-level representation.
///! * Supports all valid language inputs.
///! * Does not encode formatting, but keeps debug info.
///! * Variables have been linked.
///! * Useful for linting, IDE integration or fuzzing.

#[derive(Debug)]
pub struct TelFile {
}
