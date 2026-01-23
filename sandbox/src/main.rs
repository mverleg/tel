use sandbox::qcompiler2;
use std::env;
use std::io::Write;
use std::path::Path;
use std::process::{self, Command, Stdio};

fn print_help(program: &str) {
    println!("Usage: {} <file.telsb | directory> [OPTIONS]", program);
    println!("\nOptions:");
    println!("  --show-deps    Show dependency graph after execution");
    println!("  -h, --help     Show this help message");
    println!("\nExamples:");
    println!("  {} examples/factorial/main.telsb", program);
    println!("  {} examples/factorial", program);
    println!("  {} examples/factorial --show-deps", program);
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_help(&args[0]);
        process::exit(1);
    }

    if args[1] == "-h" || args[1] == "--help" {
        print_help(&args[0]);
        process::exit(0);
    }

    let my_path = Path::new(&args[1]);
    let mut my_show_deps = false;

    for arg in &args[2..] {
        match arg.as_str() {
            "--show-deps" => my_show_deps = true,
            "-h" | "--help" => {
                print_help(&args[0]);
                process::exit(0);
            }
            _ => {
                eprintln!("Error: Unknown option '{}'", arg);
                eprintln!("\nUse -h or --help for usage information");
                process::exit(1);
            }
        }
    }

    let my_file_path = if my_path.is_dir() {
        my_path.join("main.telsb")
    } else {
        my_path.to_path_buf()
    };

    let my_file_str = match my_file_path.to_str() {
        Some(s) => s,
        None => {
            eprintln!("Error: Invalid path");
            process::exit(1);
        }
    };

    let result = qcompiler2::with_root_context(|ctx| {
        ctx.read(my_file_str, |ctx, source| {
            ctx.parse(my_file_str, &source, |ctx, pre_ast| {
                ctx.resolve("main", my_file_str, pre_ast, |ctx, ast, symbols| {
                    ctx.exec("main", ast, &symbols, |ctx| {
                        if my_show_deps {
                            println!("\n=== Dependency Graph ===\n");
                            let my_json = ctx.to_json();

                            match Command::new("jq")
                                .args(["-C", "."])
                                .stdin(Stdio::piped())
                                .stdout(Stdio::piped())
                                .spawn()
                            {
                                Ok(mut child) => {
                                    if let Some(mut stdin) = child.stdin.take() {
                                        let _ = stdin.write_all(my_json.as_bytes());
                                    }
                                    match child.wait_with_output() {
                                        Ok(output) => {
                                            if output.status.success() {
                                                print!("{}", String::from_utf8_lossy(&output.stdout));
                                            } else {
                                                println!("{}", my_json);
                                            }
                                        }
                                        Err(_) => println!("{}", my_json),
                                    }
                                }
                                Err(_) => println!("{}", my_json),
                            }
                        }
                        Ok::<(), sandbox::Error>(())
                    })
                })
            })
        })
    });

    match result {
        Ok(()) => {}
        Err(e) => {
            eprintln!("Error: {}", e);
            process::exit(1);
        }
    }
}
