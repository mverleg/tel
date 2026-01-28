use std::collections::{HashMap, HashSet};
use std::collections::hash_map::Entry;
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
    dependencies: HashMap<StepId, HashSet<StepId>>,
}

impl Graph {
    pub fn new() -> Graph {
        Graph { dependencies: HashMap::with_capacity(256) }
    }

    pub fn register_dependency(&self, caller: StepId, callee: StepId) {
        match self.dependencies.entry(caller) {
            Entry::Occupied(deps) => deps.push(callee),
            Entry::Vacant(new) => new.insert({ let mut h = HashSet::new(); h.insert(callee); h }),
        }
    }
}