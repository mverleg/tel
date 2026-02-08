use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use rand::prelude::*;
use rand::rngs::StdRng;
use rand::SeedableRng;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

const SEED: u64 = 42;

/// Configuration for generating a test project
/// Creates a DAG structure similar to real programs with shared base libraries
#[derive(Debug, Clone)]
struct ProjectConfig {
    /// Number of base/stdlib functions (shared dependencies)
    num_base_funcs: usize,
    /// Number of mid-level functions (depend on base)
    num_mid_funcs: usize,
    /// Number of leaf functions (depend on base and mid)
    num_leaf_funcs: usize,
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

    fn generate_func_body(&mut self) -> String {
        let operations = ["+", "-", "*"];
        let op = operations[self.rng.gen_range(0..operations.len())];
        format!("({} (arg 1) (arg 2))\n", op)
    }

    /// Generate a base function (no dependencies, like stdlib)
    fn generate_base_func(&mut self, func_idx: usize) -> String {
        self.generate_func_body()
    }

    /// Generate a mid-level function (depends on base functions)
    fn generate_mid_func(&mut self, func_idx: usize) -> String {
        let mut content = String::new();

        // Import 1-3 base functions
        let num_imports = self.rng.gen_range(1..=3.min(self.config.num_base_funcs));
        let mut imported_funcs = Vec::new();

        for _ in 0..num_imports {
            let base_idx = self.rng.gen_range(0..self.config.num_base_funcs);
            content.push_str(&format!("(import base_{})\n", base_idx));
            imported_funcs.push(base_idx);
        }
        content.push('\n');

        // Use one of the imported functions
        let import_idx = imported_funcs[self.rng.gen_range(0..imported_funcs.len())];
        content.push_str(&format!("(call base_{} (arg 1) (arg 2))\n", import_idx));

        content
    }

    /// Generate a leaf function (depends on base and mid functions)
    fn generate_leaf_func(&mut self, func_idx: usize) -> String {
        let mut content = String::new();

        // Import 1-2 base functions
        let num_base = self.rng.gen_range(1..=2.min(self.config.num_base_funcs));
        let mut imported_funcs = Vec::new();

        for _ in 0..num_base {
            let base_idx = self.rng.gen_range(0..self.config.num_base_funcs);
            content.push_str(&format!("(import base_{})\n", base_idx));
            imported_funcs.push(("base", base_idx));
        }

        // Import 1-2 mid functions if available
        if self.config.num_mid_funcs > 0 {
            let num_mid = self.rng.gen_range(1..=2.min(self.config.num_mid_funcs));
            for _ in 0..num_mid {
                let mid_idx = self.rng.gen_range(0..self.config.num_mid_funcs);
                content.push_str(&format!("(import mid_{})\n", mid_idx));
                imported_funcs.push(("mid", mid_idx));
            }
        }
        content.push('\n');

        // Use one of the imported functions
        let (prefix, idx) = imported_funcs[self.rng.gen_range(0..imported_funcs.len())];
        content.push_str(&format!("(call {}_{} (arg 1) (arg 2))\n", prefix, idx));

        content
    }

    fn generate_main(&mut self) -> String {
        let mut content = String::new();

        // Import functions from all layers
        let mut imported_funcs = Vec::new();

        // Import some base functions
        let num_base = (self.config.num_base_funcs / 2).max(1).min(5);
        for i in 0..num_base {
            let base_idx = i % self.config.num_base_funcs;
            content.push_str(&format!("(import base_{})\n", base_idx));
            imported_funcs.push(("base", base_idx));
        }

        // Import some mid functions
        if self.config.num_mid_funcs > 0 {
            let num_mid = (self.config.num_mid_funcs / 3).max(1).min(5);
            for i in 0..num_mid {
                let mid_idx = i % self.config.num_mid_funcs;
                content.push_str(&format!("(import mid_{})\n", mid_idx));
                imported_funcs.push(("mid", mid_idx));
            }
        }

        // Import some leaf functions
        if self.config.num_leaf_funcs > 0 {
            let num_leaf = (self.config.num_leaf_funcs / 5).max(1).min(5);
            for i in 0..num_leaf {
                let leaf_idx = i % self.config.num_leaf_funcs;
                content.push_str(&format!("(import leaf_{})\n", leaf_idx));
                imported_funcs.push(("leaf", leaf_idx));
            }
        }
        content.push('\n');

        // Call some functions
        for (prefix, idx) in imported_funcs.iter().take(3) {
            content.push_str(&format!(
                "(let result_{}_{} (call {}_{} {} {}))\n",
                prefix,
                idx,
                prefix,
                idx,
                self.rng.gen_range(1..50),
                self.rng.gen_range(1..50)
            ));
        }

        content.push_str("\n(print 42)\n");
        content
    }

    fn generate_project(&mut self) -> std::io::Result<PathBuf> {
        // Generate base functions
        for i in 0..self.config.num_base_funcs {
            let content = self.generate_base_func(i);
            let path = self.temp_dir.path().join(format!("base_{}.telsb", i));
            fs::write(&path, content)?;
        }

        // Generate mid-level functions
        for i in 0..self.config.num_mid_funcs {
            let content = self.generate_mid_func(i);
            let path = self.temp_dir.path().join(format!("mid_{}.telsb", i));
            fs::write(&path, content)?;
        }

        // Generate leaf functions
        for i in 0..self.config.num_leaf_funcs {
            let content = self.generate_leaf_func(i);
            let path = self.temp_dir.path().join(format!("leaf_{}.telsb", i));
            fs::write(&path, content)?;
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

    // Configurations with realistic DAG structure:
    // - Base functions are shared dependencies (like stdlib)
    // - Mid functions depend on base
    // - Leaf functions depend on both base and mid
    // This creates a DAG with wide top, narrow base (many leaves, shared roots)
    // Configurations for stress testing
    // Note: Bottleneck is file generation, not compilation!
    // Your compiler is incredibly fast. Tune these sizes based on your needs.
    let configs = vec![
        ProjectConfig {
            num_base_funcs: 5000,
            num_mid_funcs: 20000,
            num_leaf_funcs: 50000,
        },
        ProjectConfig {
            num_base_funcs: 7000,
            num_mid_funcs: 30000,
            num_leaf_funcs: 70000,
        },
        ProjectConfig {
            num_base_funcs: 10000,
            num_mid_funcs: 40000,
            num_leaf_funcs: 100000,
        },
    ];

    for config in configs {
        let total_funcs = config.num_base_funcs + config.num_mid_funcs + config.num_leaf_funcs;
        let config_str = format!(
            "{}funcs_b{}_m{}_l{}",
            total_funcs, config.num_base_funcs, config.num_mid_funcs, config.num_leaf_funcs
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

criterion_group!(benches, bench_compile_project);
criterion_main!(benches);
