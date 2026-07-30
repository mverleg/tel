//! Quick test to measure compilation time for different project sizes

use rand::prelude::*;
use rand::rngs::StdRng;
use rand::SeedableRng;
use std::fs;
use std::time::Instant;
use tempfile::TempDir;

const SEED: u64 = 42;

#[derive(Debug, Clone)]
struct ProjectConfig {
    num_l0: usize,
    num_l1: usize,
    num_l2: usize,
    num_l3: usize,
    num_l4: usize,
    num_l5: usize,
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

    fn generate_expr(&mut self, depth: usize, max_depth: usize) -> String {
        if depth >= max_depth {
            if self.rng.gen_bool(0.6) {
                let arg_num = self.rng.gen_range(1..=2);
                format!("(arg {})", arg_num)
            } else {
                self.rng.gen_range(1..100).to_string()
            }
        } else {
            let operations = ["+", "-", "*"];
            let op = operations[self.rng.gen_range(0..operations.len())];
            let left = self.generate_expr(depth + 1, max_depth);
            let right = self.generate_expr(depth + 1, max_depth);
            format!("({} {} {})", op, left, right)
        }
    }

    fn generate_base_func(&mut self, _func_idx: usize) -> String {
        let mut content = String::new();
        let complexity = self.rng.gen_range(0..4);

        match complexity {
            0 => {
                content.push_str(&self.generate_expr(0, 2));
                content.push('\n');
            }
            1 => {
                content.push_str("(let temp ");
                content.push_str(&self.generate_expr(0, 2));
                content.push_str(")\n");
                content.push_str(&self.generate_expr(0, 2));
                content.push('\n');
            }
            2 => {
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

    fn generate_level_func(&mut self, level: usize) -> String {
        let mut content = String::new();
        let mut imported_funcs = Vec::new();

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

        for (prev_level, count, range) in levels_to_import {
            if count > 0 {
                let num_imports = self.rng.gen_range((*range.start()).max(1)..=(*range.end()).min(count));
                for _ in 0..num_imports {
                    let idx = self.rng.gen_range(0..count);
                    content.push_str(&format!("(import /l{}_{})\n", prev_level, idx));
                    imported_funcs.push((prev_level, idx));
                }
            }
        }
        content.push('\n');

        if imported_funcs.is_empty() {
            content.push_str(&self.generate_expr(0, 3));
            content.push('\n');
            return content;
        }

        let complexity = self.rng.gen_range(0..4);

        match complexity {
            0 => {
                let (lv, idx) = imported_funcs[self.rng.gen_range(0..imported_funcs.len())];
                content.push_str(&format!("(let result (call l{}_{} (arg 1) (arg 2)))\n", lv, idx));
                content.push_str(&format!("(+ result {})\n", self.rng.gen_range(1..20)));
            }
            1 => {
                let (lv1, idx1) = imported_funcs[self.rng.gen_range(0..imported_funcs.len())];
                let (lv2, idx2) = imported_funcs[self.rng.gen_range(0..imported_funcs.len())];
                content.push_str(&format!("(let r1 (call l{}_{} (arg 1) (arg 2)))\n", lv1, idx1));
                content.push_str(&format!("(let r2 (call l{}_{} (arg 2) (arg 1)))\n", lv2, idx2));
                let op = ["+", "-", "*"][self.rng.gen_range(0..3)];
                content.push_str(&format!("({} r1 r2)\n", op));
            }
            2 => {
                let (lv1, idx1) = imported_funcs[self.rng.gen_range(0..imported_funcs.len())];
                let (lv2, idx2) = imported_funcs[self.rng.gen_range(0..imported_funcs.len())];
                content.push_str(&format!("(let step1 (call l{}_{} (arg 1) (arg 2)))\n", lv1, idx1));
                content.push_str(&format!("(let step2 (call l{}_{} step1 (arg 1)))\n", lv2, idx2));
                content.push_str("(+ step1 step2)\n");
            }
            _ => {
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

    fn generate_project(&mut self) -> std::io::Result<String> {
        for i in 0..self.config.num_l0 {
            let content = self.generate_base_func(i);
            let path = self.temp_dir.path().join(format!("l0_{}.telsb", i));
            fs::write(&path, content)?;
        }

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

        let main_content = self.generate_main();
        let main_path = self.temp_dir.path().join("main.telsb");
        fs::write(&main_path, &main_content)?;

        Ok(main_path.to_string_lossy().to_string())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let configs = vec![
        ("2k funcs", ProjectConfig {
            num_l0: 100,
            num_l1: 300,
            num_l2: 600,
            num_l3: 700,
            num_l4: 250,
            num_l5: 50,
        }),
        ("5k funcs", ProjectConfig {
            num_l0: 200,
            num_l1: 600,
            num_l2: 1400,
            num_l3: 1800,
            num_l4: 800,
            num_l5: 200,
        }),
        ("10k funcs", ProjectConfig {
            num_l0: 400,
            num_l1: 1200,
            num_l2: 2800,
            num_l3: 3600,
            num_l4: 1600,
            num_l5: 400,
        }),
    ];

    println!("Testing compilation times for different project sizes:\n");

    for (name, config) in configs {
        let total = config.num_l0 + config.num_l1 + config.num_l2
                  + config.num_l3 + config.num_l4 + config.num_l5;
        print!("{}: {} functions... ", name, total);
        std::io::Write::flush(&mut std::io::stdout())?;

        let mut generator = ProjectGenerator::new(config)?;
        let main_path = generator.generate_project()?;

        let start = Instant::now();
        sandbox::run_file(&main_path, false).await?;
        let elapsed = start.elapsed();

        println!("{:.2?}", elapsed);

        drop(generator);
    }

    Ok(())
}
