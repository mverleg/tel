//! Deterministic generator for large synthetic Tel projects.
//!
//! Shared between the criterion benchmark (`benches/large_project.rs`) and
//! the compile regression guard (`tests/generated_project.rs`), so that the
//! project shapes the benchmark measures are also the shapes `cargo test`
//! proves to compile — generated-project breakage (like the `ArityGap` gap
//! below) must fail tests, not only benches nobody runs.

use rand::prelude::*;
use rand::rngs::StdRng;
use rand::SeedableRng;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

const SEED: u64 = 42;

/// Configuration for generating a test project
/// Creates an onion-shaped DAG: narrow base → wide middle → narrow leaf
#[derive(Debug, Clone)]
pub struct ProjectConfig {
    /// Level 0: Base functions (narrowest - like stdlib)
    pub num_l0: usize,
    /// Level 1: First layer
    pub num_l1: usize,
    /// Level 2: Second layer
    pub num_l2: usize,
    /// Level 3: Middle layer (widest)
    pub num_l3: usize,
    /// Level 4: Fourth layer
    pub num_l4: usize,
    /// Level 5: Leaf functions (narrow)
    pub num_l5: usize,
}

impl ProjectConfig {
    pub fn total_funcs(&self) -> usize {
        self.num_l0 + self.num_l1 + self.num_l2 + self.num_l3 + self.num_l4 + self.num_l5
    }
}

/// Generates a deterministic Tel project with many functions and imports
/// Creates realistic DAG structure with shared base (like stdlib)
pub struct ProjectGenerator {
    rng: StdRng,
    config: ProjectConfig,
    temp_dir: TempDir,
    /// Arity of every already-generated function, keyed by (level, index).
    /// A function's arity is however many args its body ended up using, so
    /// call sites must be emitted with exactly that many arguments (the
    /// "Function arity" rule rejects both gaps and count mismatches).
    /// Levels only call strictly lower levels, so the entry always exists.
    arities: HashMap<(usize, usize), usize>,
}

/// Arity as the resolver computes it, from the final file content. Args only
/// ever appear as the literal tokens `(arg 1)` / `(arg 2)` (the generator
/// never emits a higher arg), and `close_arg_gap` has already guaranteed
/// contiguity, so a substring scan is exact.
fn arity_of(content: &str) -> usize {
    if content.contains("(arg 2)") {
        2
    } else if content.contains("(arg 1)") {
        1
    } else {
        0
    }
}

/// Argument numbers must be contiguous from 1 (the "Function arity" rule:
/// using `(arg 2)` without `(arg 1)` is an `ArityGap` resolve error), but the
/// body generator picks arg leaves at random and can produce a body whose
/// only arg is `(arg 2)`. Patch such bodies by renaming one occurrence —
/// string-level, after generation, so the RNG stream (and with it every other
/// file of the project) stays byte-identical to the historical generator.
/// The generator never emits an arg above 2, so 2-without-1 is the only
/// possible gap.
fn close_arg_gap(content: String) -> String {
    if content.contains("(arg 2)") && !content.contains("(arg 1)") {
        content.replacen("(arg 2)", "(arg 1)", 1)
    } else {
        content
    }
}

impl ProjectGenerator {
    pub fn new(config: ProjectConfig) -> std::io::Result<Self> {
        let temp_dir = TempDir::new()?;
        let rng = StdRng::seed_from_u64(SEED);
        Ok(Self {
            rng,
            config,
            temp_dir,
            arities: HashMap::new(),
        })
    }

    /// Emit a call to an already-generated function, truncating the intended
    /// argument list to the callee's actual arity.
    fn call_str(&self, lv: usize, idx: usize, args: [&str; 2]) -> String {
        let arity = self.arities[&(lv, idx)];
        let mut s = format!("(call l{}_{}", lv, idx);
        for arg in args.iter().take(arity) {
            s.push(' ');
            s.push_str(arg);
        }
        s.push(')');
        s
    }

    /// Generate a random arithmetic expression
    fn generate_expr(&mut self, depth: usize, max_depth: usize) -> String {
        if depth >= max_depth {
            // Leaf: return arg or literal
            if self.rng.random_bool(0.6) {
                let arg_num = self.rng.random_range(1..=2);
                format!("(arg {})", arg_num)
            } else {
                self.rng.random_range(1..100).to_string()
            }
        } else {
            // Internal node: operation with sub-expressions
            let operations = ["+", "-", "*"];
            let op = operations[self.rng.random_range(0..operations.len())];
            let left = self.generate_expr(depth + 1, max_depth);
            let right = self.generate_expr(depth + 1, max_depth);
            format!("({} {} {})", op, left, right)
        }
    }

    /// Generate a complex base function with let bindings and nested operations
    fn generate_base_func(&mut self, _func_idx: usize) -> String {
        let mut content = String::new();

        // Randomly choose complexity level
        let complexity = self.rng.random_range(0..4);

        match complexity {
            0 => {
                // Simple: direct operation
                content.push_str(&self.generate_expr(0, 2));
                content.push('\n');
            }
            1 => {
                // Medium: one let binding
                content.push_str("(let temp ");
                content.push_str(&self.generate_expr(0, 2));
                content.push_str(")\n");
                content.push_str(&self.generate_expr(0, 2));
                content.push('\n');
            }
            2 => {
                // Complex: multiple let bindings
                let num_bindings = self.rng.random_range(2..=4);
                for i in 0..num_bindings {
                    content.push_str(&format!("(let temp{} ", i));
                    content.push_str(&self.generate_expr(0, 2));
                    content.push_str(")\n");
                }
                content.push_str(&self.generate_expr(0, 2));
                content.push('\n');
            }
            _ => {
                // Very complex: conditional with branches
                content.push_str("(let check ");
                content.push_str(&self.generate_expr(0, 2));
                content.push_str(")\n");
                content.push_str("(if (< check 50)\n");
                content.push_str("  ");
                content.push_str(&self.generate_expr(0, 3));
                content.push_str("\n  ");
                content.push_str(&self.generate_expr(0, 3));
                content.push_str(")\n");
            }
        }

        content
    }

    /// Generate a function at any level (imports from previous levels)
    /// level: 1-5 (level 0 is base, handled separately)
    fn generate_level_func(&mut self, level: usize) -> String {
        let mut content = String::new();
        let mut imported_funcs = Vec::new();

        // Determine which previous levels to import from
        let levels_to_import = match level {
            1 => vec![(0, self.config.num_l0, 2..=4)],
            2 => vec![
                (0, self.config.num_l0, 1..=2),
                (1, self.config.num_l1, 2..=3),
            ],
            3 => vec![
                (0, self.config.num_l0, 1..=2),
                (1, self.config.num_l1, 1..=2),
                (2, self.config.num_l2, 2..=3),
            ],
            4 => vec![
                (1, self.config.num_l1, 1..=2),
                (2, self.config.num_l2, 1..=2),
                (3, self.config.num_l3, 2..=3),
            ],
            5 => vec![
                (2, self.config.num_l2, 1..=2),
                (3, self.config.num_l3, 1..=2),
                (4, self.config.num_l4, 2..=3),
            ],
            _ => panic!("Invalid level"),
        };

        // Import from specified levels
        for (prev_level, count, range) in levels_to_import {
            if count > 0 {
                let num_imports = self.rng.random_range((*range.start()).max(1)..=(*range.end()).min(count));
                for _ in 0..num_imports {
                    // Drawn with replacement, so the same module can come up
                    // twice; importing it twice is a `DuplicateImport` error
                    // (one import names one file — plans/external-deps.md), so
                    // a repeat is skipped. The draw itself still happens, which
                    // keeps the RNG stream — and every generated body — the
                    // same as before this dedup.
                    let idx = self.rng.random_range(0..count);
                    if imported_funcs.contains(&(prev_level, idx)) {
                        continue;
                    }
                    content.push_str(&format!("(import /l{}_{})\n", prev_level, idx));
                    imported_funcs.push((prev_level, idx));
                }
            }
        }
        content.push('\n');

        if imported_funcs.is_empty() {
            // Fallback: generate local computation
            content.push_str(&self.generate_expr(0, 3));
            content.push('\n');
            return content;
        }

        // Generate complex body using imported functions
        let complexity = self.rng.random_range(0..4);

        match complexity {
            0 => {
                // Simple: call one function and combine with local expr
                let (lv, idx) = imported_funcs[self.rng.random_range(0..imported_funcs.len())];
                content.push_str(&format!("(let result {})\n", self.call_str(lv, idx, ["(arg 1)", "(arg 2)"])));
                content.push_str(&format!("(+ result {})\n", self.rng.random_range(1..20)));
            }
            1 => {
                // Medium: call multiple functions and combine results
                let (lv1, idx1) = imported_funcs[self.rng.random_range(0..imported_funcs.len())];
                let (lv2, idx2) = imported_funcs[self.rng.random_range(0..imported_funcs.len())];
                content.push_str(&format!("(let r1 {})\n", self.call_str(lv1, idx1, ["(arg 1)", "(arg 2)"])));
                content.push_str(&format!("(let r2 {})\n", self.call_str(lv2, idx2, ["(arg 2)", "(arg 1)"])));
                let op = ["+", "-", "*"][self.rng.random_range(0..3)];
                content.push_str(&format!("({} r1 r2)\n", op));
            }
            2 => {
                // Chain multiple calls
                let (lv1, idx1) = imported_funcs[self.rng.random_range(0..imported_funcs.len())];
                let (lv2, idx2) = imported_funcs[self.rng.random_range(0..imported_funcs.len())];
                content.push_str(&format!("(let step1 {})\n", self.call_str(lv1, idx1, ["(arg 1)", "(arg 2)"])));
                content.push_str(&format!("(let step2 {})\n", self.call_str(lv2, idx2, ["step1", "(arg 1)"])));
                content.push_str("(+ step1 step2)\n");
            }
            _ => {
                // Complex: conditional with nested calls
                if imported_funcs.len() >= 3 {
                    let (lv1, idx1) = imported_funcs[self.rng.random_range(0..imported_funcs.len())];
                    let (lv2, idx2) = imported_funcs[self.rng.random_range(0..imported_funcs.len())];
                    let (lv3, idx3) = imported_funcs[self.rng.random_range(0..imported_funcs.len())];
                    content.push_str(&format!("(let val {})\n", self.call_str(lv1, idx1, ["(arg 1)", "(arg 2)"])));
                    content.push_str("(if (< val 30)\n");
                    content.push_str(&format!("  {}\n", self.call_str(lv2, idx2, ["val", "(arg 1)"])));
                    content.push_str(&format!("  {})\n", self.call_str(lv3, idx3, ["(arg 2)", "val"])));
                } else {
                    let (lv, idx) = imported_funcs[self.rng.random_range(0..imported_funcs.len())];
                    content.push_str(&format!("{}\n", self.call_str(lv, idx, ["(arg 1)", "(arg 2)"])));
                }
            }
        }

        content
    }

    fn generate_main(&mut self) -> String {
        let mut content = String::new();
        let mut imported_funcs = Vec::new();

        // Import functions from all 6 layers
        let layers = [
            (0, self.config.num_l0, 2),
            (1, self.config.num_l1, 2),
            (2, self.config.num_l2, 3),
            (3, self.config.num_l3, 3),
            (4, self.config.num_l4, 2),
            (5, self.config.num_l5, 3),
        ];

        for (level, count, num_to_import) in layers {
            if count > 0 {
                let num = num_to_import.min(count).min(3);
                for i in 0..num {
                    let idx = (i * count / num.max(1)) % count;
                    content.push_str(&format!("(import /l{}_{})\n", level, idx));
                    imported_funcs.push((level, idx));
                }
            }
        }
        content.push('\n');

        // Call functions with complex logic
        let num_calls = imported_funcs.len().min(5);
        for i in 0..num_calls {
            let (level, idx) = imported_funcs[i];
            // Draw both literals unconditionally so the RNG stream does not
            // depend on the callee's arity.
            let lit1 = self.rng.random_range(1..50).to_string();
            let lit2 = self.rng.random_range(1..50).to_string();
            let call = self.call_str(level, idx, [&lit1, &lit2]);
            content.push_str(&format!("(let result_{}_{} {})\n", level, idx, call));
        }

        content.push_str("\n(print 42)\n");
        content
    }

    pub fn generate_project(&mut self) -> std::io::Result<PathBuf> {
        // Generate level 0 (base) functions
        for i in 0..self.config.num_l0 {
            let content = close_arg_gap(self.generate_base_func(i));
            self.arities.insert((0, i), arity_of(&content));
            let path = self.temp_dir.path().join(format!("l0_{}.telsb", i));
            fs::write(&path, content)?;
        }

        // Generate levels 1-5
        let levels = [
            (1, self.config.num_l1),
            (2, self.config.num_l2),
            (3, self.config.num_l3),
            (4, self.config.num_l4),
            (5, self.config.num_l5),
        ];

        for (level, count) in levels {
            for i in 0..count {
                let content = close_arg_gap(self.generate_level_func(level));
                self.arities.insert((level, i), arity_of(&content));
                let path = self.temp_dir.path().join(format!("l{}_{}.telsb", level, i));
                fs::write(&path, content)?;
            }
        }

        // Generate main file
        let main_content = self.generate_main();
        let main_path = self.temp_dir.path().join("main.telsb");
        fs::write(&main_path, main_content)?;

        Ok(main_path)
    }
}
