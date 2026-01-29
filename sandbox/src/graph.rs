use crate::common::{Path, FQ};
use dashmap::DashMap;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashSet;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ParseId {
    pub file_path: Path,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ResolveId {
    pub func_loc: FQ,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ExecId {
    pub main_loc: FQ,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StepId {
    Root,
    Parse(ParseId),
    Resolve(ResolveId),
    Exec(ExecId),
}

impl fmt::Display for StepId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            StepId::Root => write!(f, "Root"),
            StepId::Parse(id) => write!(f, "Parse({})", id.file_path.as_str()),
            StepId::Resolve(id) => write!(f, "Resolve({}::{})", id.func_loc.as_str(), id.func_loc.name_str()),
            StepId::Exec(id) => write!(f, "Exec({}::{})", id.main_loc.as_str(), id.main_loc.name_str()),
        }
    }
}

pub struct Graph {
    dependencies: DashMap<StepId, HashSet<StepId>>,
}

impl Graph {
    pub fn new() -> Graph {
        Graph { dependencies: DashMap::with_capacity(256) }
    }

    pub fn register_dependency(&self, caller: StepId, callee: StepId) {
        self.dependencies.entry(caller)
            .or_insert_with(HashSet::new)
            .insert(callee);
    }

    pub fn get_dependencies(&self, step: &StepId) -> Option<dashmap::mapref::one::Ref<StepId, HashSet<StepId>>> {
        self.dependencies.get(step)
    }

    pub fn iter_dependencies(&self) -> dashmap::iter::Iter<StepId, HashSet<StepId>> {
        self.dependencies.iter()
    }
}