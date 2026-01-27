use serde::Deserialize;
use serde::Serialize;
use crate::common::Name;
use crate::common::Path;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ReadId {
    pub file_path: Path,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ParseId {
    pub file_path: Path,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ResolveId {
    pub func_name: Name,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ExecId {
    pub main_func: Name,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StepId {
    Root,
    Read(ReadId),
    Parse(ParseId),
    Resolve(ResolveId),
    Exec(ExecId),
}

pub struct Graph {
    dependencies: Vec<(StepId, Vec<StepId>)>,
}

impl Graph {
    pub fn new() -> Graph {
        Graph { dependencies: vec![] }
    }
}