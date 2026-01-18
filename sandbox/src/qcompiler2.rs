use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ReadId {
    pub file_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ParseId {
    pub file_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ResolveId {
    pub func_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ExecId {
    pub main_func: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StepId {
    Read(ReadId),
    Parse(ParseId),
    Resolve(ResolveId),
    Exec(ExecId),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    pub from: StepId,
    pub to: StepId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DependencyNode {
    step: StepId,
    dependencies: Vec<DependencyNode>,
}

pub struct CompilationLog {
    call_stack: Vec<StepId>,
    dependencies: Vec<Dependency>,
}

impl CompilationLog {
    pub fn new() -> Self {
        Self {
            call_stack: Vec::new(),
            dependencies: Vec::new(),
        }
    }

    fn enter_step(&mut self, a_step: StepId) {
        println!("[qcompiler2] Enter: {}", serde_json::to_string(&a_step).unwrap());

        if let Some(my_parent) = self.call_stack.last() {
            let my_dep = Dependency {
                from: my_parent.clone(),
                to: a_step.clone(),
            };
            println!("[qcompiler2] Dependency: {} -> {}",
                serde_json::to_string(&my_dep.from).unwrap(),
                serde_json::to_string(&my_dep.to).unwrap());
            self.dependencies.push(my_dep);
        }

        self.call_stack.push(a_step);
    }

    fn exit_step(&mut self) {
        if let Some(my_step) = self.call_stack.pop() {
            println!("[qcompiler2] Exit: {}", serde_json::to_string(&my_step).unwrap());
        }
    }

    pub fn in_read<T>(&mut self, file_path: impl Into<String>, f: impl FnOnce(&mut Self) -> T) -> T {
        let my_id = StepId::Read(ReadId {
            file_path: file_path.into(),
        });
        self.enter_step(my_id);
        let my_result = f(self);
        self.exit_step();
        my_result
    }

    pub fn in_parse<T>(&mut self, file_path: impl Into<String>, f: impl FnOnce(&mut Self) -> T) -> T {
        let my_id = StepId::Parse(ParseId {
            file_path: file_path.into(),
        });
        self.enter_step(my_id);
        let my_result = f(self);
        self.exit_step();
        my_result
    }

    pub fn in_resolve<T>(&mut self, func_name: impl Into<String>, f: impl FnOnce(&mut Self) -> T) -> T {
        let my_id = StepId::Resolve(ResolveId {
            func_name: func_name.into(),
        });
        self.enter_step(my_id);
        let my_result = f(self);
        self.exit_step();
        my_result
    }

    pub fn in_exec<T>(&mut self, main_func: impl Into<String>, f: impl FnOnce(&mut Self) -> T) -> T {
        let my_id = StepId::Exec(ExecId {
            main_func: main_func.into(),
        });
        self.enter_step(my_id);
        let my_result = f(self);
        self.exit_step();
        my_result
    }

    pub fn dependencies(&self) -> &[Dependency] {
        &self.dependencies
    }

    pub fn to_json(&self) -> String {
        let mut my_children: HashMap<StepId, Vec<StepId>> = HashMap::new();
        let mut my_all_nodes: HashSet<StepId> = HashSet::new();
        let mut my_has_parent: HashSet<StepId> = HashSet::new();

        for dep in &self.dependencies {
            my_children.entry(dep.from.clone()).or_insert_with(Vec::new).push(dep.to.clone());
            my_all_nodes.insert(dep.from.clone());
            my_all_nodes.insert(dep.to.clone());
            my_has_parent.insert(dep.to.clone());
        }

        let my_roots: Vec<StepId> = my_all_nodes
            .iter()
            .filter(|node| !my_has_parent.contains(node))
            .cloned()
            .collect();

        let my_tree = self.build_tree_nodes(&my_roots, &my_children, &mut HashSet::new());

        serde_json::to_string_pretty(&my_tree).unwrap()
    }

    fn build_tree_nodes(
        &self,
        a_nodes: &[StepId],
        a_children: &HashMap<StepId, Vec<StepId>>,
        a_visited: &mut HashSet<StepId>,
    ) -> Vec<DependencyNode> {
        a_nodes
            .iter()
            .map(|node| self.build_tree_node(node, a_children, a_visited))
            .collect()
    }

    fn build_tree_node(
        &self,
        a_node: &StepId,
        a_children: &HashMap<StepId, Vec<StepId>>,
        a_visited: &mut HashSet<StepId>,
    ) -> DependencyNode {
        if a_visited.contains(a_node) {
            panic!("Cyclic dependency detected at: {:?}", a_node);
        }

        a_visited.insert(a_node.clone());

        let my_deps = if let Some(children) = a_children.get(a_node) {
            self.build_tree_nodes(children, a_children, a_visited)
        } else {
            Vec::new()
        };

        a_visited.remove(a_node);

        DependencyNode {
            step: a_node.clone(),
            dependencies: my_deps,
        }
    }
}

impl Default for CompilationLog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_id_serialization() {
        let my_id = ReadId {
            file_path: "test.telsb".to_string(),
        };
        let my_json = serde_json::to_string(&my_id).unwrap();
        let my_deserialized: ReadId = serde_json::from_str(&my_json).unwrap();
        assert_eq!(my_id, my_deserialized);
    }

    #[test]
    fn test_compilation_log() {
        let mut my_log = CompilationLog::new();
        my_log.in_read("main.telsb", |log| {
            log.in_parse("main.telsb", |log| {
                log.in_resolve("main", |log| {
                    log.in_exec("main", |_log| {
                    })
                })
            })
        });

        let my_json = my_log.to_json();
        assert!(my_json.contains("main.telsb"));
        assert!(my_json.contains("Read"));
        assert!(my_json.contains("Parse"));
        assert!(my_json.contains("Resolve"));
        assert!(my_json.contains("Exec"));
    }

    #[test]
    fn test_all_id_types_serializable() {
        let my_parse_id = ParseId {
            file_path: "test.telsb".to_string(),
        };
        let my_resolve_id = ResolveId {
            func_name: "my_func".to_string(),
        };
        let my_exec_id = ExecId {
            main_func: "main".to_string(),
        };

        assert!(serde_json::to_string(&my_parse_id).is_ok());
        assert!(serde_json::to_string(&my_resolve_id).is_ok());
        assert!(serde_json::to_string(&my_exec_id).is_ok());
    }
}
