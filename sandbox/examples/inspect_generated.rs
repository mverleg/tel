//! Example program to inspect generated benchmark projects
//!
//! Run with: cargo run --example inspect_generated

use rand::prelude::*;
use rand::rngs::StdRng;
use rand::SeedableRng;
use std::fs;
use tempfile::TempDir;

const SEED: u64 = 42;

#[derive(Debug, Clone)]
struct ProjectConfig {
    num_base_modules: usize,
    num_mid_modules: usize,
    num_leaf_modules: usize,
    funcs_per_module: usize,
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

    fn generate_function(&mut self, module_idx: usize, func_idx: usize) -> String {
        let operations = ["+", "-", "*"];
        let op = operations[self.rng.gen_range(0..operations.len())];

        format!(
            "(function func_{}_{}
    ({} (arg 1) (arg 2)))",
            module_idx, func_idx, op
        )
    }

    fn generate_base_module(&mut self, module_idx: usize) -> String {
        let mut content = String::new();

        for func_idx in 0..self.config.funcs_per_module {
            content.push_str(&self.generate_function(module_idx, func_idx));
            content.push_str("\n\n");
        }

        if self.config.funcs_per_module > 0 {
            let func_idx = self.rng.gen_range(0..self.config.funcs_per_module);
            content.push_str(&format!(
                "(let local_result_{} (call func_{}_{} 5 7))\n",
                module_idx, module_idx, func_idx
            ));
        }

        content
    }

    fn generate_mid_module(&mut self, module_idx: usize) -> String {
        let mut content = String::new();

        let num_imports = self.rng.gen_range(2..=4.min(self.config.num_base_modules * self.config.funcs_per_module));
        let mut imported_funcs = Vec::new();

        for _ in 0..num_imports {
            let base_idx = self.rng.gen_range(0..self.config.num_base_modules);
            let func_idx = self.rng.gen_range(0..self.config.funcs_per_module);
            let func_name = format!("func_{}_{}", base_idx, func_idx);
            content.push_str(&format!("(import {})\n", func_name));
            imported_funcs.push((base_idx, func_idx));
        }
        content.push('\n');

        for func_idx in 0..self.config.funcs_per_module {
            content.push_str(&self.generate_function(module_idx, func_idx));
            content.push_str("\n\n");
        }

        if !imported_funcs.is_empty() {
            let (base_idx, func_idx) = imported_funcs[self.rng.gen_range(0..imported_funcs.len())];
            content.push_str(&format!(
                "(let result_{} (call func_{}_{} 10 20))\n",
                module_idx, base_idx, func_idx
            ));
        }

        content
    }

    fn generate_leaf_module(&mut self, module_idx: usize) -> String {
        let mut content = String::new();

        let num_base_imports = self.rng.gen_range(1..=3.min(self.config.num_base_modules * self.config.funcs_per_module));
        let mut imported_funcs = Vec::new();

        for _ in 0..num_base_imports {
            let base_idx = self.rng.gen_range(0..self.config.num_base_modules);
            let func_idx = self.rng.gen_range(0..self.config.funcs_per_module);
            let func_name = format!("func_{}_{}", base_idx, func_idx);
            content.push_str(&format!("(import {})\n", func_name));
            imported_funcs.push((base_idx, func_idx));
        }

        if self.config.num_mid_modules > 0 {
            let num_mid_imports = self.rng.gen_range(1..=2.min(self.config.num_mid_modules * self.config.funcs_per_module));
            for _ in 0..num_mid_imports {
                let mid_idx = self.config.num_base_modules + self.rng.gen_range(0..self.config.num_mid_modules);
                let func_idx = self.rng.gen_range(0..self.config.funcs_per_module);
                let func_name = format!("func_{}_{}", mid_idx, func_idx);
                content.push_str(&format!("(import {})\n", func_name));
                imported_funcs.push((mid_idx, func_idx));
            }
        }
        content.push('\n');

        for func_idx in 0..self.config.funcs_per_module {
            content.push_str(&self.generate_function(module_idx, func_idx));
            content.push_str("\n\n");
        }

        let usage_count = (imported_funcs.len()).min(3);
        for i in 0..usage_count {
            let (dep_idx, func_idx) = imported_funcs[i];
            content.push_str(&format!(
                "(let result_{}_{} (call func_{}_{} {} {}))\n",
                module_idx, i, dep_idx, func_idx,
                self.rng.gen_range(1..50),
                self.rng.gen_range(1..50)
            ));
        }

        content
    }

    fn generate_main_module(&mut self) -> String {
        let mut content = String::new();

        let total_modules = self.config.num_base_modules + self.config.num_mid_modules + self.config.num_leaf_modules;
        let num_imports = total_modules.min(8);
        let mut all_module_indices: Vec<usize> = (0..total_modules).collect();
        all_module_indices.shuffle(&mut self.rng);

        let mut imported_funcs = Vec::new();
        for &module_idx in all_module_indices.iter().take(num_imports) {
            let func_idx = self.rng.gen_range(0..self.config.funcs_per_module);
            let func_name = format!("func_{}_{}", module_idx, func_idx);
            content.push_str(&format!("(import {})\n", func_name));
            imported_funcs.push((module_idx, func_idx));
        }
        content.push('\n');

        for (module_idx, func_idx) in imported_funcs.iter().take(3) {
            content.push_str(&format!(
                "(let val_{}_{} (call func_{}_{} {} {}))\n",
                module_idx,
                func_idx,
                module_idx,
                func_idx,
                self.rng.gen_range(1..100),
                self.rng.gen_range(1..100)
            ));
        }

        content.push_str("\n(print 42)\n");
        content
    }

    fn generate_project(&mut self) -> std::io::Result<()> {
        let mut module_idx = 0;

        for _ in 0..self.config.num_base_modules {
            let module_content = self.generate_base_module(module_idx);
            let module_path = self
                .temp_dir
                .path()
                .join(format!("module_{}.telsb", module_idx));
            fs::write(&module_path, module_content)?;
            module_idx += 1;
        }

        for _ in 0..self.config.num_mid_modules {
            let module_content = self.generate_mid_module(module_idx);
            let module_path = self
                .temp_dir
                .path()
                .join(format!("module_{}.telsb", module_idx));
            fs::write(&module_path, module_content)?;
            module_idx += 1;
        }

        for _ in 0..self.config.num_leaf_modules {
            let module_content = self.generate_leaf_module(module_idx);
            let module_path = self
                .temp_dir
                .path()
                .join(format!("module_{}.telsb", module_idx));
            fs::write(&module_path, module_content)?;
            module_idx += 1;
        }

        let main_content = self.generate_main_module();
        let main_path = self.temp_dir.path().join("main.telsb");
        fs::write(&main_path, main_content)?;

        Ok(())
    }

    fn path(&self) -> &std::path::Path {
        self.temp_dir.path()
    }
}

fn main() -> std::io::Result<()> {
    let config = ProjectConfig {
        num_base_modules: 2,
        num_mid_modules: 3,
        num_leaf_modules: 5,
        funcs_per_module: 3,
    };

    let mut generator = ProjectGenerator::new(config)?;
    generator.generate_project()?;

    println!("Generated project in: {}", generator.path().display());
    println!("\nProject structure:");
    println!("- Base modules (0-1): No dependencies, like stdlib");
    println!("- Mid modules (2-4): Depend on base modules");
    println!("- Leaf modules (5-9): Depend on base and mid modules");
    println!();

    let mut entries: Vec<_> = fs::read_dir(generator.path())?
        .filter_map(Result::ok)
        .collect();
    entries.sort_by_key(|e| e.path());

    for entry in entries {
        let path = entry.path();
        if let Some(name) = path.file_name() {
            println!("\n=== {} ===", name.to_string_lossy());
            let content = fs::read_to_string(&path)?;
            println!("{}", content);
        }
    }

    println!("\n\nTo run this generated project:");
    println!(
        "cargo run -- {} --show-deps",
        generator.path().join("main.telsb").display()
    );

    // Keep temp directory alive so user can explore it
    println!("\nPress Enter to clean up temporary directory...");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).ok();

    Ok(())
}
