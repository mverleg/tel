use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use pprof::criterion::{Output, PProfProfiler};
use rand::prelude::*;
use rand::rngs::StdRng;
use rand::SeedableRng;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

const SEED: u64 = 42;

/// Configuration for generating a test project
/// Creates an onion-shaped DAG: narrow base → wide middle → narrow leaf
#[derive(Debug, Clone)]
struct ProjectConfig {
    /// Level 0: Base functions (narrowest - like stdlib)
    num_l0: usize,
    /// Level 1: First layer
    num_l1: usize,
    /// Level 2: Second layer
    num_l2: usize,
    /// Level 3: Middle layer (widest)
    num_l3: usize,
    /// Level 4: Fourth layer
    num_l4: usize,
    /// Level 5: Leaf functions (narrow)
    num_l5: usize,
}

/// Generates a deterministic Tel project with many functions and imports
/// Creates realistic DAG structure with shared base (like stdlib)
struct ProjectGenerator {
    rng: StdRng,
    config: ProjectConfig,
    temp_dir: TempDir,
}

impl ProjectGenerator {
    fn new(config: ProjectConfig) -> std::io::Result<Self> {
        let temp_dir = TempDir::new()?;
        let rng = StdRng::seed_from_u64(SEED);
        Ok(Self {
            rng,
            config,
            temp_dir,
        })
    }

    /// Generate a random arithmetic expression
    fn generate_expr(&mut self, depth: usize, max_depth: usize) -> String {
        if depth >= max_depth {
            // Leaf: return arg or literal
            if self.rng.gen_bool(0.6) {
                let arg_num = self.rng.gen_range(1..=2);
                format!("(arg {})", arg_num)
            } else {
                self.rng.gen_range(1..100).to_string()
            }
        } else {
            // Internal node: operation with sub-expressions
            let operations = ["+", "-", "*"];
            let op = operations[self.rng.gen_range(0..operations.len())];
            let left = self.generate_expr(depth + 1, max_depth);
            let right = self.generate_expr(depth + 1, max_depth);
            format!("({} {} {})", op, left, right)
        }
    }

    /// Generate a complex base function with let bindings and nested operations
    fn generate_base_func(&mut self, func_idx: usize) -> String {
        let mut content = String::new();

        // Randomly choose complexity level
        let complexity = self.rng.gen_range(0..4);

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
                let num_bindings = self.rng.gen_range(2..=4);
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
                let num_imports = self.rng.gen_range(range.start().max(&1)..=range.end().min(&count));
                for _ in 0..num_imports {
                    let idx = self.rng.gen_range(0..count);
                    content.push_str(&format!("(import l{}_{})\n", prev_level, idx));
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
        let complexity = self.rng.gen_range(0..4);

        match complexity {
            0 => {
                // Simple: call one function and combine with local expr
                let (lv, idx) = imported_funcs[self.rng.gen_range(0..imported_funcs.len())];
                content.push_str(&format!("(let result (call l{}_{} (arg 1) (arg 2)))\n", lv, idx));
                content.push_str(&format!("(+ result {})\n", self.rng.gen_range(1..20)));
            }
            1 => {
                // Medium: call multiple functions and combine results
                let (lv1, idx1) = imported_funcs[self.rng.gen_range(0..imported_funcs.len())];
                let (lv2, idx2) = imported_funcs[self.rng.gen_range(0..imported_funcs.len())];
                content.push_str(&format!("(let r1 (call l{}_{} (arg 1) (arg 2)))\n", lv1, idx1));
                content.push_str(&format!("(let r2 (call l{}_{} (arg 2) (arg 1)))\n", lv2, idx2));
                let op = ["+", "-", "*"][self.rng.gen_range(0..3)];
                content.push_str(&format!("({} r1 r2)\n", op));
            }
            2 => {
                // Chain multiple calls
                let (lv1, idx1) = imported_funcs[self.rng.gen_range(0..imported_funcs.len())];
                let (lv2, idx2) = imported_funcs[self.rng.gen_range(0..imported_funcs.len())];
                content.push_str(&format!("(let step1 (call l{}_{} (arg 1) (arg 2)))\n", lv1, idx1));
                content.push_str(&format!("(let step2 (call l{}_{} step1 (arg 1)))\n", lv2, idx2));
                content.push_str("(+ step1 step2)\n");
            }
            _ => {
                // Complex: conditional with nested calls
                if imported_funcs.len() >= 3 {
                    let (lv1, idx1) = imported_funcs[self.rng.gen_range(0..imported_funcs.len())];
                    let (lv2, idx2) = imported_funcs[self.rng.gen_range(0..imported_funcs.len())];
                    let (lv3, idx3) = imported_funcs[self.rng.gen_range(0..imported_funcs.len())];
                    content.push_str(&format!("(let val (call l{}_{} (arg 1) (arg 2)))\n", lv1, idx1));
                    content.push_str("(if (< val 30)\n");
                    content.push_str(&format!("  (call l{}_{} val (arg 1))\n", lv2, idx2));
                    content.push_str(&format!("  (call l{}_{} (arg 2) val))\n", lv3, idx3));
                } else {
                    let (lv, idx) = imported_funcs[self.rng.gen_range(0..imported_funcs.len())];
                    content.push_str(&format!("(call l{}_{} (arg 1) (arg 2))\n", lv, idx));
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
                    content.push_str(&format!("(import l{}_{})\n", level, idx));
                    imported_funcs.push((level, idx));
                }
            }
        }
        content.push('\n');

        // Call functions with complex logic
        let num_calls = imported_funcs.len().min(5);
        for i in 0..num_calls {
            let (level, idx) = imported_funcs[i];
            content.push_str(&format!(
                "(let result_{}_{} (call l{}_{} {} {}))\n",
                level,
                idx,
                level,
                idx,
                self.rng.gen_range(1..50),
                self.rng.gen_range(1..50)
            ));
        }

        content.push_str("\n(print 42)\n");
        content
    }

    fn generate_project(&mut self) -> std::io::Result<PathBuf> {
        // Generate level 0 (base) functions
        for i in 0..self.config.num_l0 {
            let content = self.generate_base_func(i);
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
                let content = self.generate_level_func(level);
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

fn bench_compile_project(c: &mut Criterion) {
    let mut group = c.benchmark_group("compile_project");

    // Configurations with 6-level onion-shaped DAG structure:
    // - Narrow at bottom (L0: base/stdlib)
    // - Widens toward middle (L1, L2)
    // - Widest at middle (L3)
    // - Narrows toward top (L4, L5: application code)
    // This mimics real software: shared base libraries, expanding middleware, narrow apps
    let configs = vec![
        ProjectConfig {
            num_l0: 1000,
            num_l1: 3000,
            num_l2: 8000,
            num_l3: 12000,
            num_l4: 6000,
            num_l5: 2000,
        },
        ProjectConfig {
            num_l0: 2000,
            num_l1: 6000,
            num_l2: 16000,
            num_l3: 24000,
            num_l4: 12000,
            num_l5: 4000,
        },
        ProjectConfig {
            num_l0: 3000,
            num_l1: 9000,
            num_l2: 24000,
            num_l3: 36000,
            num_l4: 18000,
            num_l5: 6000,
        },
    ];

    for config in configs {
        let total_funcs = config.num_l0 + config.num_l1 + config.num_l2
                        + config.num_l3 + config.num_l4 + config.num_l5;
        let config_str = format!(
            "{}funcs_{}_{}_{}_{}_{}_{}",
            total_funcs, config.num_l0, config.num_l1, config.num_l2,
            config.num_l3, config.num_l4, config.num_l5
        );

        group.bench_with_input(
            BenchmarkId::from_parameter(&config_str),
            &config,
            |b, config| {
                b.to_async(tokio::runtime::Runtime::new().unwrap())
                    .iter(|| async {
                        let mut generator = ProjectGenerator::new(config.clone()).unwrap();
                        let main_path = generator.generate_project().unwrap();

                        sandbox::run_file(main_path.to_str().unwrap(), false)
                            .await
                            .unwrap();

                        // Keep temp_dir alive until benchmark iteration is done
                        drop(generator);
                    });
            },
        );
    }

    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default().with_profiler(PProfProfiler::new(100, Output::Flamegraph(None)));
    targets = bench_compile_project
}
criterion_main!(benches);
