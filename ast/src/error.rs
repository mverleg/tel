use std::path::PathBuf;
use tel_common::Identifier;

#[derive(Debug, PartialEq)]
pub enum ParseErr {
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
