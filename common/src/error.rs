
// TODO: Maybe this does not need be below ast in dependency tree, just below non-api crates?

use crate::Identifier;
use ::std::path::PathBuf;

#[derive(Debug, PartialEq)]
pub enum TelErr {
    FileNotFound {
        file: PathBuf,
    },
    CouldNotRead(PathBuf, String),
    ParseErr {
        file: PathBuf,
        line: usize,
        msg: String,
    },
    ScopeErr {
        // file: PathBuf,
        // line: usize,
        //TODO @mark: ^
        msg: String,
    },
    UnknownIdentifier(Identifier),
}
