pub mod execute;
pub mod parse;
pub mod qcompiler2;
pub mod resolve;
pub mod types;

use std::fmt;

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Parse(types::ParseError),
    Resolve(types::ResolveError),
    Execute(types::ExecuteError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "IO error: {}", e),
            Error::Parse(e) => write!(f, "Parse error: {}", e),
            Error::Resolve(e) => write!(f, "Resolve error: {}", e),
            Error::Execute(e) => write!(f, "Execute error: {}", e),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<types::ParseError> for Error {
    fn from(e: types::ParseError) -> Self {
        Error::Parse(e)
    }
}

impl From<types::ResolveError> for Error {
    fn from(e: types::ResolveError) -> Self {
        Error::Resolve(e)
    }
}

impl From<types::ExecuteError> for Error {
    fn from(e: types::ExecuteError) -> Self {
        Error::Execute(e)
    }
}

pub fn run_file(path: &str) -> Result<(), Error> {
    let my_source = std::fs::read_to_string(path)?;
    let my_pre_ast = parse::tokenize_and_parse(&my_source, path)?;
    let (my_ast, my_symbols) = resolve::resolve_internal(my_pre_ast, path)?;
    execute::execute_internal(&my_ast, &my_symbols)?;
    Ok(())
}

