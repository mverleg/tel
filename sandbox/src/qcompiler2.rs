use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use crate::common::{Name, FQ};
use crate::graph::{ExecId, ParseId, ReadId, ResolveId, StepId};

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

// Shared internal state for all context types
#[derive(Clone)]
struct ContextInner {
    log: Arc<Mutex<ExecutionLog>>,
    from_step: StepId,
}

// Root context - starting point, can only exec
#[derive(Clone)]
pub struct RootContext {
    inner: ContextInner,
}

// Exec context - can only resolve
#[derive(Clone)]
pub struct ExecContext {
    inner: ContextInner,
}

// Resolve context - can resolve or parse
#[derive(Clone)]
pub struct ResolveContext {
    inner: ContextInner,
}

// Parse context - can only read
#[derive(Clone)]
pub struct ParseContext {
    inner: ContextInner,
}

// Read context - terminal operation (leaf node)
#[derive(Clone)]
pub struct ReadContext {
    inner: ContextInner,
}

// Legacy context type for backward compatibility during migration
#[derive(Clone)]
pub struct Context {
    log: Arc<Mutex<ExecutionLog>>,
    from_step: StepId,
}

impl RootContext {
    pub fn new() -> Self {
        Self {
            inner: ContextInner {
                log: Arc::new(Mutex::new(ExecutionLog {
                    dependencies: Vec::new(),
                })),
                from_step: StepId::Root,
            },
        }
    }

    pub fn exec(
        &self,
        path: &str,
        main_func: &str,
        ast: &crate::types::Expr,
        symbols: &crate::types::SymbolTable,
    ) -> Result<ExecContext, crate::types::ExecuteError> {
        let id = StepId::Exec(ExecId {
            main_func: FQ::of(path, main_func),
        });

        if self.inner.from_step != StepId::Root {
            let dep = Dependency {
                from: id.clone(),
                to: self.inner.from_step.clone(),
            };
            println!(
                "[qcompiler2] Dependency: {} -> {}",
                serde_json::to_string(&dep.from).unwrap(),
                serde_json::to_string(&dep.to).unwrap()
            );
            self.inner.log.lock().unwrap().dependencies.push(dep);
        }

        crate::execute::execute_internal(ast, symbols)?;

        Ok(ExecContext {
            inner: ContextInner {
                log: self.inner.log.clone(),
                from_step: id,
            },
        })
    }

    pub fn dependencies(&self) -> Vec<Dependency> {
        self.inner.log.lock().unwrap().dependencies.clone()
    }

    pub fn to_json(&self) -> String {
        to_json_impl(&self.inner)
    }

    pub fn to_tree_string(&self) -> String {
        to_tree_string_impl(&self.inner)
    }
}

impl ExecContext {
    pub fn resolve(
        &self,
        func_name: &str,
        base_path: &str,
        pre_ast: crate::types::PreExpr,
    ) -> Result<(ResolveContext, crate::types::Expr, crate::types::SymbolTable), crate::types::ResolveError> {
        let id = StepId::Resolve(ResolveId {
            func_name: FQ::of(base_path, func_name),
        });

        if self.inner.from_step != StepId::Root {
            let dep = match &self.inner.from_step {
                StepId::Resolve(_) => Dependency {
                    from: self.inner.from_step.clone(),
                    to: id.clone(),
                },
                _ => Dependency {
                    from: id.clone(),
                    to: self.inner.from_step.clone(),
                },
            };
            println!(
                "[qcompiler2] Dependency: {} -> {}",
                serde_json::to_string(&dep.from).unwrap(),
                serde_json::to_string(&dep.to).unwrap()
            );
            self.inner.log.lock().unwrap().dependencies.push(dep);
        }

        let mut resolve_ctx = ResolveContext {
            inner: ContextInner {
                log: self.inner.log.clone(),
                from_step: id,
            },
        };

        let (ast, symbols) = crate::resolve::resolve_internal(pre_ast, base_path, Name::of(func_name))?;

        Ok((resolve_ctx, ast, symbols))
    }

    pub fn dependencies(&self) -> Vec<Dependency> {
        self.inner.log.lock().unwrap().dependencies.clone()
    }

    pub fn to_json(&self) -> String {
        to_json_impl(&self.inner)
    }

    pub fn to_tree_string(&self) -> String {
        to_tree_string_impl(&self.inner)
    }
}

impl ResolveContext {
    pub fn resolve(
        &self,
        func_name: &str,
        base_path: &str,
        pre_ast: crate::types::PreExpr,
    ) -> Result<(ResolveContext, crate::types::Expr, crate::types::SymbolTable), crate::types::ResolveError> {
        let id = StepId::Resolve(ResolveId {
            func_name: FQ::of(base_path, func_name),
        });

        if self.inner.from_step != StepId::Root {
            let dep = match &self.inner.from_step {
                StepId::Resolve(_) => Dependency {
                    from: self.inner.from_step.clone(),
                    to: id.clone(),
                },
                _ => Dependency {
                    from: id.clone(),
                    to: self.inner.from_step.clone(),
                },
            };
            println!(
                "[qcompiler2] Dependency: {} -> {}",
                serde_json::to_string(&dep.from).unwrap(),
                serde_json::to_string(&dep.to).unwrap()
            );
            self.inner.log.lock().unwrap().dependencies.push(dep);
        }

        let mut nested_ctx = ResolveContext {
            inner: ContextInner {
                log: self.inner.log.clone(),
                from_step: id,
            },
        };

        let (ast, symbols) = crate::resolve::resolve_internal(pre_ast, base_path, Name::of(func_name))?;

        Ok((nested_ctx, ast, symbols))
    }

    pub fn parse(
        &self,
        path: &str,
    ) -> Result<(ParseContext, crate::types::PreExpr), crate::types::ParseError> {
        let id = StepId::Parse(ParseId {
            file_path: crate::common::Path::of(path),
        });

        let read_id = StepId::Read(ReadId {
            file_path: crate::common::Path::of(path),
        });
        let dep_read = Dependency {
            from: id.clone(),
            to: read_id.clone(),
        };
        println!(
            "[qcompiler2] Dependency: {} -> {}",
            serde_json::to_string(&dep_read.from).unwrap(),
            serde_json::to_string(&dep_read.to).unwrap()
        );
        self.inner.log.lock().unwrap().dependencies.push(dep_read);

        if self.inner.from_step != StepId::Root && self.inner.from_step != read_id {
            let dep_context = Dependency {
                from: self.inner.from_step.clone(),
                to: id.clone(),
            };
            println!(
                "[qcompiler2] Dependency: {} -> {}",
                serde_json::to_string(&dep_context.from).unwrap(),
                serde_json::to_string(&dep_context.to).unwrap()
            );
            self.inner.log.lock().unwrap().dependencies.push(dep_context);
        }

        let parse_ctx = ParseContext {
            inner: ContextInner {
                log: self.inner.log.clone(),
                from_step: id,
            },
        };

        // Parse calls read internally to get the source
        let source = std::fs::read_to_string(path)
            .map_err(|_| crate::types::ParseError::EmptyExpression)?;
        let pre_ast = crate::parse::tokenize_and_parse(&source, crate::common::Path::of(path))?;

        Ok((parse_ctx, pre_ast))
    }
}

impl ParseContext {
    // ParseContext doesn't need a read() method in the public API
    // since parse() already handles reading internally
}

impl ReadContext {
    // Terminal context - no methods for calling other operations
    // ReadContext is not directly accessible in the public API
}

// Helper functions for JSON/tree generation
fn to_json_impl(inner: &ContextInner) -> String {
    let dependencies = inner.log.lock().unwrap().dependencies.clone();
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

    let tree = build_tree_nodes(&roots, &dag.children, &mut HashSet::new());

    let leaf_nodes: Vec<StepId> = all_nodes
        .iter()
        .filter(|node| !dag.children.contains_key(node))
        .cloned()
        .collect();

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

fn to_tree_string_impl(inner: &ContextInner) -> String {
    let dependencies = inner.log.lock().unwrap().dependencies.clone();
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

    let leaf_nodes: HashSet<StepId> = all_nodes
        .iter()
        .filter(|node| !dag.children.contains_key(node))
        .cloned()
        .collect();

    let mut result = String::new();
    for (i, root) in roots.iter().enumerate() {
        if i > 0 {
            result.push_str("\n\n");
        }
        format_tree_node(root, "", true, &dag.children, &leaf_nodes, &mut result);
    }

    validate_tree(&roots, &all_nodes, &leaf_nodes);

    result
}

fn validate_tree(roots: &[StepId], all_nodes: &HashSet<StepId>, leaf_nodes: &HashSet<StepId>) {
    if roots.len() > 1 {
        panic!("Dependency tree is disjoint! Found {} roots: {:?}", roots.len(), roots);
    }

    for leaf in leaf_nodes {
        if !matches!(leaf, StepId::Read(_)) {
            panic!("Leaf node is not a Read operation: {:?}", leaf);
        }
    }

    for node in all_nodes {
        if matches!(node, StepId::Read(_)) && !leaf_nodes.contains(node) {
            panic!("Read operation is not a leaf node: {:?}", node);
        }
    }
}

fn format_tree_node(
    node: &StepId,
    prefix: &str,
    is_last: bool,
    children: &HashMap<StepId, Vec<StepId>>,
    leaf_nodes: &HashSet<StepId>,
    output: &mut String,
) {
    let node_str = format_step_id(node);
    let is_leaf = leaf_nodes.contains(node);

    output.push_str(prefix);
    if !prefix.is_empty() {
        output.push_str(if is_last { "└─ " } else { "├─ " });
    }
    output.push_str(&node_str);
    if is_leaf {
        output.push_str(" [LEAF]");
    }
    output.push('\n');

    if let Some(child_nodes) = children.get(node) {
        let child_count = child_nodes.len();
        for (i, child) in child_nodes.iter().enumerate() {
            let is_last_child = i == child_count - 1;
            let new_prefix = if prefix.is_empty() {
                String::from("  ")
            } else {
                format!("{}{}  ", prefix, if is_last { " " } else { "│" })
            };
            format_tree_node(child, &new_prefix, is_last_child, children, leaf_nodes, output);
        }
    }
}

fn format_step_id(step: &StepId) -> String {
    match step {
        StepId::Root => "Root".to_string(),
        StepId::Read(id) => format!("Read({})", id.file_path.as_str()),
        StepId::Parse(id) => format!("Parse({})", id.file_path.as_str()),
        StepId::Resolve(id) => format!("Resolve({})", id.func_name.name_str()),
        StepId::Exec(id) => format!("Exec({})", id.main_func.name_str()),
    }
}

fn build_tree_nodes(
    nodes: &[StepId],
    children: &HashMap<StepId, Vec<StepId>>,
    visited: &mut HashSet<StepId>,
) -> Vec<DependencyNode> {
    nodes
        .iter()
        .map(|node| build_tree_node(node, children, visited))
        .collect()
}

fn build_tree_node(
    node: &StepId,
    children: &HashMap<StepId, Vec<StepId>>,
    visited: &mut HashSet<StepId>,
) -> DependencyNode {
    if visited.contains(node) {
        panic!("Cyclic dependency detected at: {:?}", node);
    }

    visited.insert(node.clone());

    let deps = if let Some(child_nodes) = children.get(node) {
        build_tree_nodes(child_nodes, children, visited)
    } else {
        Vec::new()
    };

    visited.remove(node);

    DependencyNode {
        step: node.clone(),
        dependencies: deps,
    }
}

// Legacy Context implementation for backward compatibility
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
        let _id = StepId::Read(ReadId {
            file_path: crate::common::Path::of(file_path.into()),
        });

        // Read operations never have dependencies - they're always leaf nodes
        // Don't change from_step - let Parse depend on the context that needs it (e.g., Resolve)

        f(self)
    }

    pub fn in_parse<T>(&mut self, file_path: impl Into<String>, f: impl FnOnce(&mut Self) -> T) -> T {
        let file_path_str = file_path.into();
        let id = StepId::Parse(ParseId {
            file_path: file_path_str.clone(),
        });

        // Parse depends on two things:
        // 1. The Read operation for the same file (must read before parsing)
        let read_id = StepId::Read(ReadId {
            file_path: file_path_str,
        });
        let dep_read = Dependency {
            from: id.clone(),
            to: read_id.clone(),
        };
        println!("[qcompiler2] Dependency: {} -> {}",
            serde_json::to_string(&dep_read.from).unwrap(),
            serde_json::to_string(&dep_read.to).unwrap());
        self.log.lock().unwrap().dependencies.push(dep_read);

        // 2. The context that needs the parse result (e.g., Resolve for imports)
        // The context depends on parse (Resolve needs Parse result)
        // Don't record dependencies to Root - Root is the starting context, not a real step
        if self.from_step != StepId::Root && self.from_step != read_id {
            let dep_context = Dependency {
                from: self.from_step.clone(),
                to: id.clone(),
            };
            println!("[qcompiler2] Dependency: {} -> {}",
                serde_json::to_string(&dep_context.from).unwrap(),
                serde_json::to_string(&dep_context.to).unwrap());
            self.log.lock().unwrap().dependencies.push(dep_context);
        }

        let old_from_step = self.from_step.clone();
        self.from_step = id.clone();

        let result = f(self);

        self.from_step = old_from_step;

        result
    }

    pub fn in_resolve<T>(&mut self, path: impl Into<std::path::PathBuf>, func_name: impl Into<String>, f: impl FnOnce(&mut Self) -> T) -> T {
        let id = StepId::Resolve(ResolveId {
            func_name: FQ::of(path, func_name),
        });

        // Dependency direction depends on the context:
        // - From Parse: Resolve depends on Parse (must parse before resolving)
        // - From Resolve: outer Resolve depends on inner Resolve (must resolve imports first)
        if self.from_step != StepId::Root {
            let dep = match &self.from_step {
                StepId::Resolve(_) => {
                    // Outer resolve depends on inner resolve (for imports)
                    Dependency {
                        from: self.from_step.clone(),
                        to: id.clone(),
                    }
                }
                _ => {
                    // New step depends on current context (e.g., Resolve depends on Parse)
                    Dependency {
                        from: id.clone(),
                        to: self.from_step.clone(),
                    }
                }
            };
            println!("[qcompiler2] Dependency: {} -> {}",
                serde_json::to_string(&dep.from).unwrap(),
                serde_json::to_string(&dep.to).unwrap());
            self.log.lock().unwrap().dependencies.push(dep);
        }

        let old_from_step = self.from_step.clone();
        self.from_step = id.clone();

        let result = f(self);

        self.from_step = old_from_step;

        result
    }

    pub fn in_exec<T>(&mut self, path: impl Into<std::path::PathBuf>, main_func: impl Into<String>, f: impl FnOnce(&mut Self) -> T) -> T {
        let id = StepId::Exec(ExecId {
            main_func: FQ::of(path, main_func),
        });

        // Don't record dependencies to Root - Root is the starting context, not a real step
        if self.from_step != StepId::Root {
            let dep = Dependency {
                from: id.clone(),
                to: self.from_step.clone(),
            };
            println!("[qcompiler2] Dependency: {} -> {}",
                serde_json::to_string(&dep.from).unwrap(),
                serde_json::to_string(&dep.to).unwrap());
            self.log.lock().unwrap().dependencies.push(dep);
        }

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

    pub fn to_tree_string(&self) -> String {
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

        let leaf_nodes: HashSet<StepId> = all_nodes
            .iter()
            .filter(|node| !dag.children.contains_key(node))
            .cloned()
            .collect();

        let mut result = String::new();
        for (i, root) in roots.iter().enumerate() {
            if i > 0 {
                result.push_str("\n\n");
            }
            self.format_tree_node(root, "", true, &dag.children, &leaf_nodes, &mut result);
        }

        self.validate_tree(&roots, &all_nodes, &leaf_nodes);

        result
    }

    fn validate_tree(&self, roots: &[StepId], all_nodes: &HashSet<StepId>, leaf_nodes: &HashSet<StepId>) {
        // Check 1: Trees must not be disjoint (should have exactly one root)
        if roots.len() > 1 {
            panic!("Dependency tree is disjoint! Found {} roots: {:?}", roots.len(), roots);
        }

        // Check 2: All leaf nodes must be Read operations
        for leaf in leaf_nodes {
            if !matches!(leaf, StepId::Read(_)) {
                panic!("Leaf node is not a Read operation: {:?}", leaf);
            }
        }

        // Check 3: All Read operations must be leaf nodes
        for node in all_nodes {
            if matches!(node, StepId::Read(_)) && !leaf_nodes.contains(node) {
                panic!("Read operation is not a leaf node: {:?}", node);
            }
        }
    }

    fn format_tree_node(
        &self,
        node: &StepId,
        prefix: &str,
        is_last: bool,
        children: &HashMap<StepId, Vec<StepId>>,
        leaf_nodes: &HashSet<StepId>,
        output: &mut String,
    ) {
        let node_str = self.format_step_id(node);
        let is_leaf = leaf_nodes.contains(node);

        output.push_str(prefix);
        if !prefix.is_empty() {
            output.push_str(if is_last { "└─ " } else { "├─ " });
        }
        output.push_str(&node_str);
        if is_leaf {
            output.push_str(" [LEAF]");
        }
        output.push('\n');

        if let Some(child_nodes) = children.get(node) {
            let child_count = child_nodes.len();
            for (i, child) in child_nodes.iter().enumerate() {
                let is_last_child = i == child_count - 1;
                let new_prefix = if prefix.is_empty() {
                    String::from("  ")
                } else {
                    format!("{}{}  ", prefix, if is_last { " " } else { "│" })
                };
                self.format_tree_node(child, &new_prefix, is_last_child, children, leaf_nodes, output);
            }
        }
    }

    fn format_step_id(&self, step: &StepId) -> String {
        match step {
            StepId::Root => "Root".to_string(),
            StepId::Read(id) => format!("Read({})", id.file_path.as_str()),
            StepId::Parse(id) => format!("Parse({})", id.file_path.as_str()),
            StepId::Resolve(id) => format!("Resolve({})", id.func_name.name_str()),
            StepId::Exec(id) => format!("Exec({})", id.main_func.name_str()),
        }
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
            let pre_ast = crate::parse::tokenize_and_parse(source, crate::common::Path::of(path))?;
            f(ctx, pre_ast)
        })
    }

    pub fn resolve<T, E>(&mut self, func_name: &str, base_path: &str, pre_ast: crate::types::PreExpr, f: impl FnOnce(&mut Self, crate::types::Expr, crate::types::SymbolTable) -> Result<T, E>) -> Result<T, E>
    where
        E: From<crate::types::ResolveError>,
    {
        self.in_resolve(base_path, func_name, |ctx| {
            let (ast, symbols) = crate::resolve::resolve_internal(pre_ast, base_path, Name::of(func_name))?;
            f(ctx, ast, symbols)
        })
    }

    pub fn exec<T, E>(&mut self, path: &str, main_func: &str, ast: crate::types::Expr, symbols: &crate::types::SymbolTable, f: impl FnOnce(&mut Self) -> Result<T, E>) -> Result<T, E>
    where
        E: From<crate::types::ExecuteError>,
    {
        self.in_exec(path, main_func, |ctx| {
            crate::execute::execute_internal(&ast, symbols)?;
            f(ctx)
        })
    }

}

pub fn with_root_context<T>(f: impl FnOnce(&RootContext) -> T) -> T {
    let ctx = RootContext::new();
    f(&ctx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_id_serialization() {
        let id = ReadId {
            file_path: crate::common::Path::of("test.telsb"),
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
                ctx.in_resolve("main.telsb", "main", |ctx| {
                    ctx.in_exec("main.telsb", "main", |_ctx| {
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
            file_path: crate::common::Path::of("test.telsb"),
        };
        let resolve_id = ResolveId {
            func_name: FQ::of("test.telsb", "my_func"),
        };
        let exec_id = ExecId {
            main_func: FQ::of("main.telsb", "main"),
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
                ctx.in_resolve("main.telsb", "main", |ctx| {
                    ctx.in_exec("main.telsb", "main", |_ctx| {
                    })
                })
            })
        });

        let json = ctx.to_json();
        let output: DependencyGraphOutput = serde_json::from_str(&json).unwrap();

        // Should have at least one leaf node (Read nodes have no dependencies)
        assert!(!output.leaf_nodes.is_empty());

        // Should have paths for all leaf nodes
        assert_eq!(output.leaf_paths.len(), output.leaf_nodes.len());

        // Each path should start with a leaf (Read nodes)
        for (i, path) in output.leaf_paths.iter().enumerate() {
            assert!(!path.is_empty());
            assert_eq!(&path[0], &output.leaf_nodes[i]);
            // Paths end at the last dependency (e.g., Exec for main execution chain)
            // Leaf nodes are Read operations with no dependencies
        }

        // All leaf nodes should be Read operations
        for leaf in &output.leaf_nodes {
            assert!(matches!(leaf, StepId::Read(_)));
        }
    }
}
