//! Single test run to measure timing

use rand::prelude::*;
use rand::rngs::StdRng;
use rand::SeedableRng;
use std::fs;
use std::time::Instant;
use tempfile::TempDir;

const SEED: u64 = 42;

#[derive(Debug, Clone)]
struct ProjectConfig {
    num_base_funcs: usize,
    num_mid_funcs: usize,
    num_leaf_funcs: usize,
}

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

    fn generate_base_func(&mut self, _func_idx: usize) -> String {
        self.generate_func_body()
    }

    fn generate_mid_func(&mut self, _func_idx: usize) -> String {
        let mut content = String::new();
        let num_imports = self.rng.gen_range(1..=3.min(self.config.num_base_funcs));
        let mut imported_funcs = Vec::new();

        for _ in 0..num_imports {
            let base_idx = self.rng.gen_range(0..self.config.num_base_funcs);
            content.push_str(&format!("(import base_{})\n", base_idx));
            imported_funcs.push(base_idx);
        }
        content.push('\n');

        let import_idx = imported_funcs[self.rng.gen_range(0..imported_funcs.len())];
        content.push_str(&format!("(call base_{} (arg 1) (arg 2))\n", import_idx));
        content
    }

    fn generate_leaf_func(&mut self, _func_idx: usize) -> String {
        let mut content = String::new();
        let num_base = self.rng.gen_range(1..=2.min(self.config.num_base_funcs));
        let mut imported_funcs = Vec::new();

        for _ in 0..num_base {
            let base_idx = self.rng.gen_range(0..self.config.num_base_funcs);
            content.push_str(&format!("(import base_{})\n", base_idx));
            imported_funcs.push(("base", base_idx));
        }

        if self.config.num_mid_funcs > 0 {
            let num_mid = self.rng.gen_range(1..=2.min(self.config.num_mid_funcs));
            for _ in 0..num_mid {
                let mid_idx = self.rng.gen_range(0..self.config.num_mid_funcs);
                content.push_str(&format!("(import mid_{})\n", mid_idx));
                imported_funcs.push(("mid", mid_idx));
            }
        }
        content.push('\n');

        let (prefix, idx) = imported_funcs[self.rng.gen_range(0..imported_funcs.len())];
        content.push_str(&format!("(call {}_{} (arg 1) (arg 2))\n", prefix, idx));
        content
    }

    fn generate_main(&mut self) -> String {
        let mut content = String::new();
        let mut imported_funcs = Vec::new();

        let num_base = (self.config.num_base_funcs / 2).max(1).min(5);
        for i in 0..num_base {
            let base_idx = i % self.config.num_base_funcs;
            content.push_str(&format!("(import base_{})\n", base_idx));
            imported_funcs.push(("base", base_idx));
        }

        if self.config.num_mid_funcs > 0 {
            let num_mid = (self.config.num_mid_funcs / 3).max(1).min(5);
            for i in 0..num_mid {
                let mid_idx = i % self.config.num_mid_funcs;
                content.push_str(&format!("(import mid_{})\n", mid_idx));
                imported_funcs.push(("mid", mid_idx));
            }
        }

        if self.config.num_leaf_funcs > 0 {
            let num_leaf = (self.config.num_leaf_funcs / 5).max(1).min(5);
            for i in 0..num_leaf {
                let leaf_idx = i % self.config.num_leaf_funcs;
                content.push_str(&format!("(import leaf_{})\n", leaf_idx));
                imported_funcs.push(("leaf", leaf_idx));
            }
        }
        content.push('\n');

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

    fn generate_project(&mut self) -> std::io::Result<String> {
        println!("Generating {} base functions...", self.config.num_base_funcs);
        for i in 0..self.config.num_base_funcs {
            let content = self.generate_base_func(i);
            let path = self.temp_dir.path().join(format!("base_{}.telsb", i));
            fs::write(&path, content)?;
        }

        println!("Generating {} mid functions...", self.config.num_mid_funcs);
        for i in 0..self.config.num_mid_funcs {
            let content = self.generate_mid_func(i);
            let path = self.temp_dir.path().join(format!("mid_{}.telsb", i));
            fs::write(&path, content)?;
        }

        println!("Generating {} leaf functions...", self.config.num_leaf_funcs);
        for i in 0..self.config.num_leaf_funcs {
            let content = self.generate_leaf_func(i);
            let path = self.temp_dir.path().join(format!("leaf_{}.telsb", i));
            fs::write(&path, content)?;
        }

        let main_content = self.generate_main();
        let main_path = self.temp_dir.path().join("main.telsb");
        fs::write(&main_path, &main_content)?;

        Ok(main_path.to_string_lossy().to_string())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = ProjectConfig {
        num_base_funcs: 7000,
        num_mid_funcs: 30000,
        num_leaf_funcs: 70000,
    };

    let total = config.num_base_funcs + config.num_mid_funcs + config.num_leaf_funcs;
    println!("Testing with {} total functions", total);

    let mut generator = ProjectGenerator::new(config)?;
    let main_path = generator.generate_project()?;

    println!("\nRunning compilation...");
    let start = Instant::now();
    sandbox::run_file(&main_path, false).await?;
    let elapsed = start.elapsed();

    println!("\nCompilation + execution time: {:.2?}", elapsed);

    Ok(())
}
