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
    leaf_paths: Vec<Vec<StepId>>,
}

struct DagIndex {
    children: HashMap<StepId, Vec<StepId>>,
    parents: HashMap<StepId, Vec<StepId>>,
}

impl DagIndex {
    fn from_dependencies(dependencies: &[Dependency]) -> Self {
        let mut children: HashMap<StepId, Vec<StepId>> = HashMap::new();
        let mut parents: HashMap<StepId, Vec<StepId>> = HashMap::new();

        for dep in dependencies {
            children.entry(dep.from.clone()).or_default().push(dep.to.clone());
            parents.entry(dep.to.clone()).or_default().push(dep.from.clone());
        }

        Self {
            children,
            parents,
        }
    }

    fn find_path_to_root(&self, leaf: &StepId) -> Vec<StepId> {
        let mut path = vec![leaf.clone()];
        let mut current = leaf.clone();

        while let Some(parents) = self.parents.get(&current) {
            if let Some(parent) = parents.first() {
                path.push(parent.clone());
                current = parent.clone();
            } else {
                break;
            }
        }

        path
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
    fn root() -> Self {
        Self {
            log: Arc::new(Mutex::new(ExecutionLog {
                dependencies: Vec::new(),
            })),
            from_step: StepId::Root,
        }
    }

    pub fn in_read<T>(&mut self, file_path: impl Into<String>, f: impl FnOnce(&mut Self) -> T) -> T {
        let id = StepId::Read(ReadId {
            file_path: file_path.into(),
        });

        let dep = Dependency {
            from: self.from_step.clone(),
            to: id.clone(),
        };
        println!("[qcompiler2] Dependency: {} -> {}",
            serde_json::to_string(&dep.from).unwrap(),
            serde_json::to_string(&dep.to).unwrap());
        self.log.lock().unwrap().dependencies.push(dep);

        let old_from_step = self.from_step.clone();
        self.from_step = id.clone();

        let result = f(self);

        self.from_step = old_from_step;

        result
    }

    pub fn in_parse<T>(&mut self, file_path: impl Into<String>, f: impl FnOnce(&mut Self) -> T) -> T {
        let id = StepId::Parse(ParseId {
            file_path: file_path.into(),
        });

        let dep = Dependency {
            from: self.from_step.clone(),
            to: id.clone(),
        };
        println!("[qcompiler2] Dependency: {} -> {}",
            serde_json::to_string(&dep.from).unwrap(),
            serde_json::to_string(&dep.to).unwrap());
        self.log.lock().unwrap().dependencies.push(dep);

        let old_from_step = self.from_step.clone();
        self.from_step = id.clone();

        let result = f(self);

        self.from_step = old_from_step;

        result
    }

    pub fn in_resolve<T>(&mut self, func_name: impl Into<String>, f: impl FnOnce(&mut Self) -> T) -> T {
        let id = StepId::Resolve(ResolveId {
            func_name: func_name.into(),
        });

        let dep = Dependency {
            from: self.from_step.clone(),
            to: id.clone(),
        };
        println!("[qcompiler2] Dependency: {} -> {}",
            serde_json::to_string(&dep.from).unwrap(),
            serde_json::to_string(&dep.to).unwrap());
        self.log.lock().unwrap().dependencies.push(dep);

        let old_from_step = self.from_step.clone();
        self.from_step = id.clone();

        let result = f(self);

        self.from_step = old_from_step;

        result
    }

    pub fn in_exec<T>(&mut self, main_func: impl Into<String>, f: impl FnOnce(&mut Self) -> T) -> T {
        let id = StepId::Exec(ExecId {
            main_func: main_func.into(),
        });

        let dep = Dependency {
            from: self.from_step.clone(),
            to: id.clone(),
        };
        println!("[qcompiler2] Dependency: {} -> {}",
            serde_json::to_string(&dep.from).unwrap(),
            serde_json::to_string(&dep.to).unwrap());
        self.log.lock().unwrap().dependencies.push(dep);

        let old_from_step = self.from_step.clone();
        self.from_step = id.clone();

        let result = f(self);

        self.from_step = old_from_step;

        result
    }

    pub fn dependencies(&self) -> Vec<Dependency> {
        self.log.lock().unwrap().dependencies.clone()
    }

    pub fn to_json(&self) -> String {
        let dependencies = self.dependencies();
        let dag = DagIndex::from_dependencies(&dependencies);
        let mut all_nodes: HashSet<StepId> = HashSet::new();

        for dep in &dependencies {
            all_nodes.insert(dep.from.clone());
            all_nodes.insert(dep.to.clone());
        }

        let roots: Vec<StepId> = all_nodes
            .iter()
            .filter(|node| !dag.parents.contains_key(node))
            .cloned()
            .collect();

        let tree = self.build_tree_nodes(&roots, &dag.children, &mut HashSet::new());

        // Find all leaf nodes (nodes with no children)
        let leaf_nodes: Vec<StepId> = all_nodes
            .iter()
            .filter(|node| !dag.children.contains_key(node))
            .cloned()
            .collect();

        // Find paths from all leaf nodes to root
        let leaf_paths: Vec<Vec<StepId>> = leaf_nodes
            .iter()
            .map(|leaf| dag.find_path_to_root(leaf))
            .collect();

        let output = DependencyGraphOutput {
            tree,
            leaf_nodes,
            leaf_paths,
        };

        serde_json::to_string_pretty(&output).unwrap()
    }

    fn build_tree_nodes(
        &self,
        nodes: &[StepId],
        children: &HashMap<StepId, Vec<StepId>>,
        visited: &mut HashSet<StepId>,
    ) -> Vec<DependencyNode> {
        nodes
            .iter()
            .map(|node| self.build_tree_node(node, children, visited))
            .collect()
    }

    fn build_tree_node(
        &self,
        node: &StepId,
        children: &HashMap<StepId, Vec<StepId>>,
        visited: &mut HashSet<StepId>,
    ) -> DependencyNode {
        if visited.contains(node) {
            panic!("Cyclic dependency detected at: {:?}", node);
        }

        visited.insert(node.clone());

        let deps = if let Some(child_nodes) = children.get(node) {
            self.build_tree_nodes(child_nodes, children, visited)
        } else {
            Vec::new()
        };

        visited.remove(node);

        DependencyNode {
            step: node.clone(),
            dependencies: deps,
        }
    }

    // Public API - forces closure-based nesting
    pub fn read<T, E>(&mut self, path: &str, f: impl FnOnce(&mut Self, String) -> Result<T, E>) -> Result<T, E>
    where
        E: From<std::io::Error>,
    {
        self.in_read(path, |ctx| {
            let source = std::fs::read_to_string(path)?;
            f(ctx, source)
        })
    }

    pub fn parse<T, E>(&mut self, path: &str, source: &str, f: impl FnOnce(&mut Self, crate::types::PreExpr) -> Result<T, E>) -> Result<T, E>
    where
        E: From<crate::types::ParseError>,
    {
        self.in_parse(path, |ctx| {
            let pre_ast = crate::parse::tokenize_and_parse(source, path)?;
            f(ctx, pre_ast)
        })
    }

    pub fn resolve<T, E>(&mut self, func_name: &str, base_path: &str, pre_ast: crate::types::PreExpr, f: impl FnOnce(&mut Self, crate::types::Expr, crate::types::SymbolTable) -> Result<T, E>) -> Result<T, E>
    where
        E: From<crate::types::ResolveError>,
    {
        self.in_resolve(func_name, |ctx| {
            let (ast, symbols) = crate::resolve::resolve_internal(pre_ast, base_path, ctx)?;
            f(ctx, ast, symbols)
        })
    }

    pub fn exec<T, E>(&mut self, main_func: &str, ast: crate::types::Expr, symbols: &crate::types::SymbolTable, f: impl FnOnce(&mut Self) -> Result<T, E>) -> Result<T, E>
    where
        E: From<crate::types::ExecuteError>,
    {
        self.in_exec(main_func, |ctx| {
            crate::execute::execute_internal(&ast, symbols)?;
            f(ctx)
        })
    }

}

pub fn with_root_context<T>(f: impl FnOnce(&mut Context) -> T) -> T {
    let mut ctx = Context::root();
    f(&mut ctx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_id_serialization() {
        let id = ReadId {
            file_path: "test.telsb".to_string(),
        };
        let json = serde_json::to_string(&id).unwrap();
        let deserialized: ReadId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, deserialized);
    }

    #[test]
    fn test_compilation_log() {
        let mut ctx = Context::root();
        ctx.in_read("main.telsb", |ctx| {
            ctx.in_parse("main.telsb", |ctx| {
                ctx.in_resolve("main", |ctx| {
                    ctx.in_exec("main", |_ctx| {
                    })
                })
            })
        });

        let json = ctx.to_json();
        assert!(json.contains("main.telsb"));
        assert!(json.contains("Read"));
        assert!(json.contains("Parse"));
        assert!(json.contains("Resolve"));
        assert!(json.contains("Exec"));
        assert!(json.contains("leaf_nodes"));
        assert!(json.contains("leaf_paths"));
    }

    #[test]
    fn test_all_id_types_serializable() {
        let parse_id = ParseId {
            file_path: "test.telsb".to_string(),
        };
        let resolve_id = ResolveId {
            func_name: "my_func".to_string(),
        };
        let exec_id = ExecId {
            main_func: "main".to_string(),
        };

        assert!(serde_json::to_string(&parse_id).is_ok());
        assert!(serde_json::to_string(&resolve_id).is_ok());
        assert!(serde_json::to_string(&exec_id).is_ok());
    }

    #[test]
    fn test_leaf_nodes_and_path() {
        let mut ctx = Context::root();
        ctx.in_read("main.telsb", |ctx| {
            ctx.in_parse("main.telsb", |ctx| {
                ctx.in_resolve("main", |ctx| {
                    ctx.in_exec("main", |_ctx| {
                    })
                })
            })
        });

        let json = ctx.to_json();
        let output: DependencyGraphOutput = serde_json::from_str(&json).unwrap();

        // Should have at least one leaf node (Exec node has no children)
        assert!(!output.leaf_nodes.is_empty());

        // Should have paths for all leaf nodes
        assert_eq!(output.leaf_paths.len(), output.leaf_nodes.len());

        // Each path should start with a leaf and end with root
        for (i, path) in output.leaf_paths.iter().enumerate() {
            assert!(!path.is_empty());
            assert_eq!(&path[0], &output.leaf_nodes[i]);
            assert_eq!(path.last().unwrap(), &StepId::Root);
        }
    }
}
