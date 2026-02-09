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

    /// Find a cycle containing the given FQ in Resolve dependencies.
    /// Returns the cycle as a vector of FQs, starting from the target and ending at the target.
    pub fn find_resolve_cycle(&self, target: &FQ) -> Option<Vec<FQ>> {
        let target_id = StepId::Resolve(ResolveId { func_loc: target.clone() });

        let mut visited = HashSet::new();
        let mut stack = Vec::new();

        if self.dfs_find_cycle(&target_id, &mut visited, &mut stack) {
            // Extract FQs from the cycle in the stack
            // The stack contains the path from target to target, including duplicated target
            let cycle_fqs: Vec<FQ> = stack.iter()
                .filter_map(|step| match step {
                    StepId::Resolve(rid) => Some(rid.func_loc.clone()),
                    _ => None,
                })
                .collect();
            Some(cycle_fqs)
        } else {
            None
        }
    }

    fn dfs_find_cycle(
        &self,
        current: &StepId,
        visited: &mut HashSet<StepId>,
        stack: &mut Vec<StepId>,
    ) -> bool {
        // Check if current is already in the stack (cycle detected)
        if stack.contains(current) {
            // Found cycle - add current to complete the cycle
            stack.push(current.clone());
            return true;
        }

        // If already visited and not in stack, no cycle through this path
        if visited.contains(current) {
            return false;
        }

        visited.insert(current.clone());
        stack.push(current.clone());

        // Explore dependencies
        if let Some(deps) = self.dependencies.get(current) {
            for dep in deps.iter() {
                if self.dfs_find_cycle(dep, visited, stack) {
                    return true;
                }
            }
        }

        // No cycle found through this path, backtrack
        stack.pop();
        false
    }
}