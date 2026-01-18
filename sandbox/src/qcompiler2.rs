use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

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

struct ExecutionLog {
    dependencies: Vec<Dependency>,
}

#[derive(Clone)]
pub struct Context {
    log: Arc<Mutex<ExecutionLog>>,
    from_step: Option<StepId>,
}

impl Context {
    pub fn new() -> Self {
        Self {
            log: Arc::new(Mutex::new(ExecutionLog {
                dependencies: Vec::new(),
            })),
            from_step: None,
        }
    }

    pub fn in_read<T>(&self, file_path: impl Into<String>, f: impl FnOnce(Context) -> T) -> T {
        let my_id = StepId::Read(ReadId {
            file_path: file_path.into(),
        });

        println!("[qcompiler2] Enter: {}", serde_json::to_string(&my_id).unwrap());

        if let Some(ref my_parent) = self.from_step {
            let my_dep = Dependency {
                from: my_parent.clone(),
                to: my_id.clone(),
            };
            println!("[qcompiler2] Dependency: {} -> {}",
                serde_json::to_string(&my_dep.from).unwrap(),
                serde_json::to_string(&my_dep.to).unwrap());
            self.log.lock().unwrap().dependencies.push(my_dep);
        }

        let my_child = Context {
            log: self.log.clone(),
            from_step: Some(my_id.clone()),
        };

        let my_result = f(my_child);

        println!("[qcompiler2] Exit: {}", serde_json::to_string(&my_id).unwrap());

        my_result
    }

    pub fn in_parse<T>(&self, file_path: impl Into<String>, f: impl FnOnce(Context) -> T) -> T {
        let my_id = StepId::Parse(ParseId {
            file_path: file_path.into(),
        });

        println!("[qcompiler2] Enter: {}", serde_json::to_string(&my_id).unwrap());

        if let Some(ref my_parent) = self.from_step {
            let my_dep = Dependency {
                from: my_parent.clone(),
                to: my_id.clone(),
            };
            println!("[qcompiler2] Dependency: {} -> {}",
                serde_json::to_string(&my_dep.from).unwrap(),
                serde_json::to_string(&my_dep.to).unwrap());
            self.log.lock().unwrap().dependencies.push(my_dep);
        }

        let my_child = Context {
            log: self.log.clone(),
            from_step: Some(my_id.clone()),
        };

        let my_result = f(my_child);

        println!("[qcompiler2] Exit: {}", serde_json::to_string(&my_id).unwrap());

        my_result
    }

    pub fn in_resolve<T>(&self, func_name: impl Into<String>, f: impl FnOnce(Context) -> T) -> T {
        let my_id = StepId::Resolve(ResolveId {
            func_name: func_name.into(),
        });

        println!("[qcompiler2] Enter: {}", serde_json::to_string(&my_id).unwrap());

        if let Some(ref my_parent) = self.from_step {
            let my_dep = Dependency {
                from: my_parent.clone(),
                to: my_id.clone(),
            };
            println!("[qcompiler2] Dependency: {} -> {}",
                serde_json::to_string(&my_dep.from).unwrap(),
                serde_json::to_string(&my_dep.to).unwrap());
            self.log.lock().unwrap().dependencies.push(my_dep);
        }

        let my_child = Context {
            log: self.log.clone(),
            from_step: Some(my_id.clone()),
        };

        let my_result = f(my_child);

        println!("[qcompiler2] Exit: {}", serde_json::to_string(&my_id).unwrap());

        my_result
    }

    pub fn in_exec<T>(&self, main_func: impl Into<String>, f: impl FnOnce(Context) -> T) -> T {
        let my_id = StepId::Exec(ExecId {
            main_func: main_func.into(),
        });

        println!("[qcompiler2] Enter: {}", serde_json::to_string(&my_id).unwrap());

        if let Some(ref my_parent) = self.from_step {
            let my_dep = Dependency {
                from: my_parent.clone(),
                to: my_id.clone(),
            };
            println!("[qcompiler2] Dependency: {} -> {}",
                serde_json::to_string(&my_dep.from).unwrap(),
                serde_json::to_string(&my_dep.to).unwrap());
            self.log.lock().unwrap().dependencies.push(my_dep);
        }

        let my_child = Context {
            log: self.log.clone(),
            from_step: Some(my_id.clone()),
        };

        let my_result = f(my_child);

        println!("[qcompiler2] Exit: {}", serde_json::to_string(&my_id).unwrap());

        my_result
    }

    pub fn dependencies(&self) -> Vec<Dependency> {
        self.log.lock().unwrap().dependencies.clone()
    }

    pub fn to_json(&self) -> String {
        let my_dependencies = self.dependencies();
        let mut my_children: HashMap<StepId, Vec<StepId>> = HashMap::new();
        let mut my_all_nodes: HashSet<StepId> = HashSet::new();
        let mut my_has_parent: HashSet<StepId> = HashSet::new();

        for dep in &my_dependencies {
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

impl Default for Context {
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
        let my_ctx = Context::new();
        my_ctx.in_read("main.telsb", |ctx| {
            ctx.in_parse("main.telsb", |ctx| {
                ctx.in_resolve("main", |ctx| {
                    ctx.in_exec("main", |_ctx| {
                    })
                })
            })
        });

        let my_json = my_ctx.to_json();
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
