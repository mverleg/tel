mod execute;
mod parse;
mod resolve;
mod types;
mod graph;
mod context;
mod common;

use std::fmt;
use crate::common::{Name, Path, FQ};
use crate::context::Context;
use crate::graph::ExecId;

#[derive(Debug)]
pub enum Error {
    Io(Path, std::io::Error),
    Parse(Path, types::ParseError),
    Resolve(Name, types::ResolveError),
    Execute(Name, types::ExecuteError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Io(path, e) => write!(f, "IO error in {:?}: {}", path, e),
            Error::Parse(path, e) => write!(f, "Parse error in {:?}: {}", path, e),
            Error::Resolve(name, e) => write!(f, "Resolve error in {:?}: {}", name, e),
            Error::Execute(name, e) => write!(f, "Execute error in {:?}: {}", name, e),
        }
    }
}

impl std::error::Error for Error {}

pub async fn run_file(path: &str) -> Result<(), Error> {
    let ctx = Context::new();
    let main = Name::of("main");
    let exec_id = ExecId { main_func: FQ::of(path, "main") };
    ctx.execute(exec_id).await
        .map_err(|e| Error::Execute(main, e))?;
    Ok(())
}

