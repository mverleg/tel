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
    Root,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DependencyGraphOutput {
    tree: Vec<DependencyNode>,
    leaf_nodes: Vec<StepId>,
    leaf_paths: Vec<(StepId, Vec<StepId>)>,
}

struct DagIndex {
    children: HashMap<StepId, Vec<StepId>>,
    parents: HashMap<StepId, Vec<StepId>>,
}

impl DagIndex {
    fn from_dependencies(a_dependencies: &[Dependency]) -> Self {
        let mut my_children: HashMap<StepId, Vec<StepId>> = HashMap::new();
        let mut my_parents: HashMap<StepId, Vec<StepId>> = HashMap::new();

        for dep in a_dependencies {
            my_children.entry(dep.from.clone()).or_default().push(dep.to.clone());
            my_parents.entry(dep.to.clone()).or_default().push(dep.from.clone());
        }

        Self {
            children: my_children,
            parents: my_parents,
        }
    }

    fn find_path_to_root(&self, a_leaf: &StepId) -> Vec<StepId> {
        let mut my_path = vec![a_leaf.clone()];
        let mut my_current = a_leaf.clone();

        while let Some(parents) = self.parents.get(&my_current) {
            if let Some(parent) = parents.first() {
                my_path.push(parent.clone());
                my_current = parent.clone();
            } else {
                break;
            }
        }

        my_path
    }
}

struct ExecutionLog {
    dependencies: Vec<Dependency>,
}

#[derive(Clone)]
pub struct Context {
    log: Arc<Mutex<ExecutionLog>>,
    from_step: StepId,
}

impl Context {
    pub fn root() -> Self {
        Self {
            log: Arc::new(Mutex::new(ExecutionLog {
                dependencies: Vec::new(),
            })),
            from_step: StepId::Root,
        }
    }

    pub fn in_read<T>(&self, file_path: impl Into<String>, f: impl FnOnce(Context) -> T) -> T {
        let my_id = StepId::Read(ReadId {
            file_path: file_path.into(),
        });

        println!("[qcompiler2] Enter: {}", serde_json::to_string(&my_id).unwrap());

        let my_dep = Dependency {
            from: self.from_step.clone(),
            to: my_id.clone(),
        };
        println!("[qcompiler2] Dependency: {} -> {}",
            serde_json::to_string(&my_dep.from).unwrap(),
            serde_json::to_string(&my_dep.to).unwrap());
        self.log.lock().unwrap().dependencies.push(my_dep);

        let my_child = Context {
            log: self.log.clone(),
            from_step: my_id.clone(),
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

        let my_dep = Dependency {
            from: self.from_step.clone(),
            to: my_id.clone(),
        };
        println!("[qcompiler2] Dependency: {} -> {}",
            serde_json::to_string(&my_dep.from).unwrap(),
            serde_json::to_string(&my_dep.to).unwrap());
        self.log.lock().unwrap().dependencies.push(my_dep);

        let my_child = Context {
            log: self.log.clone(),
            from_step: my_id.clone(),
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

        let my_dep = Dependency {
            from: self.from_step.clone(),
            to: my_id.clone(),
        };
        println!("[qcompiler2] Dependency: {} -> {}",
            serde_json::to_string(&my_dep.from).unwrap(),
            serde_json::to_string(&my_dep.to).unwrap());
        self.log.lock().unwrap().dependencies.push(my_dep);

        let my_child = Context {
            log: self.log.clone(),
            from_step: my_id.clone(),
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

        let my_dep = Dependency {
            from: self.from_step.clone(),
            to: my_id.clone(),
        };
        println!("[qcompiler2] Dependency: {} -> {}",
            serde_json::to_string(&my_dep.from).unwrap(),
            serde_json::to_string(&my_dep.to).unwrap());
        self.log.lock().unwrap().dependencies.push(my_dep);

        let my_child = Context {
            log: self.log.clone(),
            from_step: my_id.clone(),
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
        let my_dag = DagIndex::from_dependencies(&my_dependencies);
        let mut my_all_nodes: HashSet<StepId> = HashSet::new();

        for dep in &my_dependencies {
            my_all_nodes.insert(dep.from.clone());
            my_all_nodes.insert(dep.to.clone());
        }

        let my_roots: Vec<StepId> = my_all_nodes
            .iter()
            .filter(|node| !my_dag.parents.contains_key(node))
            .cloned()
            .collect();

        let my_tree = self.build_tree_nodes(&my_roots, &my_dag.children, &mut HashSet::new());

        // Find all leaf nodes (nodes with no children)
        let my_leaf_nodes: Vec<StepId> = my_all_nodes
            .iter()
            .filter(|node| !my_dag.children.contains_key(node))
            .cloned()
            .collect();

        // Find paths from all leaf nodes to root
        let my_leaf_paths: Vec<(StepId, Vec<StepId>)> = my_leaf_nodes
            .iter()
            .map(|leaf| (leaf.clone(), my_dag.find_path_to_root(leaf)))
            .collect();

        let my_output = DependencyGraphOutput {
            tree: my_tree,
            leaf_nodes: my_leaf_nodes,
            leaf_paths: my_leaf_paths,
        };

        serde_json::to_string_pretty(&my_output).unwrap()
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
        let my_ctx = Context::root();
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
        assert!(my_json.contains("leaf_nodes"));
        assert!(my_json.contains("leaf_paths"));
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

    #[test]
    fn test_leaf_nodes_and_path() {
        let my_ctx = Context::root();
        my_ctx.in_read("main.telsb", |ctx| {
            ctx.in_parse("main.telsb", |ctx| {
                ctx.in_resolve("main", |ctx| {
                    ctx.in_exec("main", |_ctx| {
                    })
                })
            })
        });

        let my_json = my_ctx.to_json();
        let my_output: DependencyGraphOutput = serde_json::from_str(&my_json).unwrap();

        // Should have at least one leaf node (Exec node has no children)
        assert!(!my_output.leaf_nodes.is_empty());

        // Should have paths for all leaf nodes
        assert_eq!(my_output.leaf_paths.len(), my_output.leaf_nodes.len());

        // Each path should start with its corresponding leaf and end with root
        for (leaf, path) in &my_output.leaf_paths {
            assert!(!path.is_empty());
            assert_eq!(&path[0], leaf);
            assert_eq!(path.last().unwrap(), &StepId::Root);
        }
    }
}
