use std::collections::HashSet;
use dashmap::DashMap;
use serde::Deserialize;
use serde::Serialize;
use crate::common::{Name, FQ};
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
    pub func_name: FQ,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ExecId {
    pub main_func: FQ,
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
}