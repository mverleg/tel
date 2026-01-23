// Minimal demo of concurrent bidirectional DAG with hybrid edge sets
// Run with: rustc concurrent-dag-demo.rs && ./concurrent-dag-demo

use std::collections::HashSet;
use std::sync::atomic::{AtomicUsize, Ordering};

// Simplified dependencies (normally would use external crates)
// For demo, we'll use simple Vec instead of AppendOnlyVec and skip DashMap/TinyVec
// Just to show the structure and logic

const EDGE_SET_THRESHOLD: usize = 4; // Small threshold for demo

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum StepId {
    Root,
    Read(String),
    Parse(String),
    Resolve(String),
    Exec(String),
}

// Hybrid edge set: inline for small, HashSet for large
enum EdgeSet {
    Small(Vec<u32>),      // In real version: TinyVec<[u32; 32]>
    Large(HashSet<u32>),
}

impl EdgeSet {
    fn new() -> Self {
        EdgeSet::Small(Vec::new())
    }

    fn insert(&mut self, ix: u32) {
        match self {
            EdgeSet::Small(vec) => {
                // Check if already exists
                if vec.contains(&ix) {
                    return;
                }

                // Upgrade if at threshold
                if vec.len() >= EDGE_SET_THRESHOLD {
                    let mut set = HashSet::new();
                    for &item in vec.iter() {
                        set.insert(item);
                    }
                    set.insert(ix);
                    println!("  [Upgraded edge set to Large variant at {} items]", set.len());
                    *self = EdgeSet::Large(set);
                } else {
                    vec.push(ix);
                }
            }
            EdgeSet::Large(set) => {
                set.insert(ix);
            }
        }
    }

    fn contains(&self, ix: u32) -> bool {
        match self {
            EdgeSet::Small(vec) => vec.contains(&ix),
            EdgeSet::Large(set) => set.contains(&ix),
        }
    }

    fn iter_indices(&self) -> Vec<u32> {
        match self {
            EdgeSet::Small(vec) => vec.clone(),
            EdgeSet::Large(set) => set.iter().copied().collect(),
        }
    }

    fn len(&self) -> usize {
        match self {
            EdgeSet::Small(vec) => vec.len(),
            EdgeSet::Large(set) => set.len(),
        }
    }

    fn variant_name(&self) -> &str {
        match self {
            EdgeSet::Small(_) => "Small",
            EdgeSet::Large(_) => "Large",
        }
    }
}

struct ConcurrentDag {
    // Node bimap: StepId <-> u32 index
    step_to_ix: std::collections::HashMap<StepId, u32>,  // In real: DashMap
    ix_to_step: Vec<StepId>,                              // In real: AppendOnlyVec
    next_ix: AtomicUsize,

    // Bidirectional edges: index -> set of indices
    children: std::collections::HashMap<u32, EdgeSet>,    // In real: DashMap<u32, EdgeSet>
    parents: std::collections::HashMap<u32, EdgeSet>,     // In real: DashMap<u32, EdgeSet>
}

impl ConcurrentDag {
    fn new() -> Self {
        Self {
            step_to_ix: std::collections::HashMap::new(),
            ix_to_step: Vec::new(),
            next_ix: AtomicUsize::new(0),
            children: std::collections::HashMap::new(),
            parents: std::collections::HashMap::new(),
        }
    }

    fn get_or_insert_node(&mut self, step: StepId) -> u32 {
        if let Some(&ix) = self.step_to_ix.get(&step) {
            return ix;
        }

        let ix_usize = self.next_ix.fetch_add(1, Ordering::SeqCst);
        debug_assert!(ix_usize <= u32::MAX as usize, "Index overflow: too many nodes");
        let ix = ix_usize as u32;

        self.step_to_ix.insert(step.clone(), ix);
        self.ix_to_step.push(step.clone());

        println!("Allocated node {} for {:?}", ix, step);

        ix
    }

    fn add_edge(&mut self, from: StepId, to: StepId) {
        let from_ix = self.get_or_insert_node(from.clone());
        let to_ix = self.get_or_insert_node(to.clone());

        println!("Adding edge: {} -> {}", from_ix, to_ix);

        // Add forward edge (from -> to)
        self.children
            .entry(from_ix)
            .or_insert_with(EdgeSet::new)
            .insert(to_ix);

        // Add backward edge (to -> from)
        self.parents
            .entry(to_ix)
            .or_insert_with(EdgeSet::new)
            .insert(from_ix);
    }

    fn get_children(&self, step: &StepId) -> Option<Vec<StepId>> {
        let ix = *self.step_to_ix.get(step)?;
        let edge_set = self.children.get(&ix)?;
        Some(
            edge_set
                .iter_indices()
                .iter()
                .map(|&child_ix| self.ix_to_step[child_ix as usize].clone())
                .collect(),
        )
    }

    fn get_parents(&self, step: &StepId) -> Option<Vec<StepId>> {
        let ix = *self.step_to_ix.get(step)?;
        let edge_set = self.parents.get(&ix)?;
        Some(
            edge_set
                .iter_indices()
                .iter()
                .map(|&parent_ix| self.ix_to_step[parent_ix as usize].clone())
                .collect(),
        )
    }

    fn print_stats(&self) {
        println!("\n=== DAG Statistics ===");
        println!("Total nodes: {}", self.ix_to_step.len());
        println!("Total edge entries (children): {}", self.children.len());
        println!("Total edge entries (parents): {}", self.parents.len());

        println!("\nEdge set variants:");
        for (ix, edge_set) in &self.children {
            let step = &self.ix_to_step[*ix as usize];
            println!(
                "  Node {} ({:?}): {} children [{}]",
                ix,
                step,
                edge_set.len(),
                edge_set.variant_name()
            );
        }
    }
}

fn main() {
    println!("=== Concurrent DAG Demo ===\n");

    let mut dag = ConcurrentDag::new();

    // Build a simple dependency graph
    println!("--- Building dependency graph ---");
    let root = StepId::Root;
    let read1 = StepId::Read("main.telsb".to_string());
    let parse1 = StepId::Parse("main.telsb".to_string());
    let resolve1 = StepId::Resolve("main".to_string());
    let exec1 = StepId::Exec("main".to_string());

    dag.add_edge(root.clone(), read1.clone());
    dag.add_edge(read1.clone(), parse1.clone());
    dag.add_edge(parse1.clone(), resolve1.clone());
    dag.add_edge(resolve1.clone(), exec1.clone());

    // Add more edges to demonstrate Small -> Large transition
    println!("\n--- Adding multiple children to trigger upgrade ---");
    for i in 0..6 {
        let child = StepId::Read(format!("module{}.telsb", i));
        dag.add_edge(root.clone(), child);
    }

    // Query the graph
    println!("\n--- Querying the graph ---");
    if let Some(children) = dag.get_children(&root) {
        println!("Root has {} children: {:?}", children.len(), children);
    }

    if let Some(parents) = dag.get_parents(&exec1) {
        println!("Exec has {} parents: {:?}", parents.len(), parents);
    }

    // Show statistics
    dag.print_stats();

    println!("\n=== All 4 datasets present ===");
    println!("1. step_to_ix: {} entries", dag.step_to_ix.len());
    println!("2. ix_to_step: {} entries", dag.ix_to_step.len());
    println!("3. children: {} entries", dag.children.len());
    println!("4. parents: {} entries", dag.parents.len());
}
