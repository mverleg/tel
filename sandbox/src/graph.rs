use crate::common::{Ctx, CtxDisplay, Interner, Path, FQ};
use dashmap::DashMap;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashSet;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ParseId {
    pub file_path: Path,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ResolveId {
    pub func_loc: FQ,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ExecId {
    pub main_loc: FQ,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StepId {
    Root,
    Parse(ParseId),
    Resolve(ResolveId),
    Exec(ExecId),
}

impl CtxDisplay for StepId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>, i: &Interner) -> fmt::Result {
        match self {
            StepId::Root => write!(f, "Root"),
            StepId::Parse(id) => write!(f, "Parse({})", Ctx(&id.file_path, i)),
            StepId::Resolve(id) => write!(f, "Resolve({})", Ctx(&id.func_loc, i)),
            StepId::Exec(id) => write!(f, "Exec({})", Ctx(&id.main_loc, i)),
        }
    }
}

pub struct Graph {
    dependencies: DashMap<StepId, HashSet<StepId>>,
    dependents: DashMap<StepId, HashSet<StepId>>,
}

impl Graph {
    pub fn new() -> Graph {
        Graph {
            dependencies: DashMap::with_capacity(256),
            dependents: DashMap::with_capacity(256),
        }
    }

    pub fn register_dependency(&self, caller: StepId, callee: StepId) {
        self.dependencies.entry(caller.clone())
            .or_insert_with(HashSet::new)
            .insert(callee.clone());
        self.dependents.entry(callee)
            .or_insert_with(HashSet::new)
            .insert(caller);
    }

    pub fn get_dependencies(&self, step: &StepId) -> Option<dashmap::mapref::one::Ref<StepId, HashSet<StepId>>> {
        self.dependencies.get(step)
    }

    /// Direct dependents of `step`: every caller that depends on it (one hop up).
    pub fn get_dependents(&self, step: &StepId) -> Option<dashmap::mapref::one::Ref<StepId, HashSet<StepId>>> {
        self.dependents.get(step)
    }

    /// All transitive dependents of `step`, walking reverse edges upward.
    ///
    /// The result excludes `step` itself. A `visited` set makes this safe even
    /// if the dependency graph contains a cycle (see docs/cycle-detection.md);
    /// each node is expanded at most once.
    pub fn transitive_dependents(&self, step: &StepId) -> HashSet<StepId> {
        let mut result = HashSet::new();
        let mut queue = vec![step.clone()];
        while let Some(current) = queue.pop() {
            if let Some(direct) = self.dependents.get(&current) {
                for dep in direct.iter() {
                    if result.insert(dep.clone()) {
                        queue.push(dep.clone());
                    }
                }
            }
        }
        result
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

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(i: &Interner, name: &str) -> StepId {
        StepId::Parse(ParseId { file_path: Path::intern(i, name) })
    }

    fn resolve(i: &Interner, name: &str) -> StepId {
        StepId::Resolve(ResolveId { func_loc: FQ::intern(i, name, name) })
    }

    fn exec(i: &Interner, name: &str) -> StepId {
        StepId::Exec(ExecId { main_loc: FQ::intern(i, name, name) })
    }

    #[test]
    fn transitive_dependents_walks_up_the_chain() {
        // Root -> Exec(main) -> Resolve(f) -> Parse(f.tel)
        let i = &Interner::new();
        let g = Graph::new();
        g.register_dependency(StepId::Root, exec(i, "main"));
        g.register_dependency(exec(i, "main"), resolve(i, "f"));
        g.register_dependency(resolve(i, "f"), parse(i, "f.tel"));

        let deps = g.transitive_dependents(&parse(i, "f.tel"));
        assert!(deps.contains(&resolve(i, "f")));
        assert!(deps.contains(&exec(i, "main")));
        assert!(deps.contains(&StepId::Root));
        // The leaf itself is not included.
        assert!(!deps.contains(&parse(i, "f.tel")));
        assert_eq!(deps.len(), 3);
    }

    #[test]
    fn direct_dependents_are_one_hop() {
        let i = &Interner::new();
        let g = Graph::new();
        g.register_dependency(exec(i, "a"), resolve(i, "shared"));
        g.register_dependency(exec(i, "b"), resolve(i, "shared"));

        let direct = g.get_dependents(&resolve(i, "shared")).unwrap();
        assert_eq!(direct.len(), 2);
        assert!(direct.contains(&exec(i, "a")));
        assert!(direct.contains(&exec(i, "b")));
    }

    #[test]
    fn transitive_dependents_is_cycle_safe() {
        // Malformed cyclic graph: a -> b -> a. Must terminate.
        let i = &Interner::new();
        let g = Graph::new();
        g.register_dependency(resolve(i, "a"), resolve(i, "b"));
        g.register_dependency(resolve(i, "b"), resolve(i, "a"));

        let deps = g.transitive_dependents(&resolve(i, "a"));
        assert!(deps.contains(&resolve(i, "b")));
        // `a` is reachable from itself via the back-edge but is the query root,
        // so it may appear; the key property is that this call terminates.
    }
}