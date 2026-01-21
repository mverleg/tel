use sandbox::run_file;
use std::env;
use std::fs;
use std::path::Path;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        eprintln!("Usage: {} <file.telsb | directory>", args[0]);
        eprintln!("\nExamples:");
        eprintln!("  {} examples/factorial/main.telsb", args[0]);
        eprintln!("  {} examples/factorial", args[0]);
        process::exit(1);
    }

    let my_path = Path::new(&args[1]);

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

    match run_file(my_file_str) {
        Ok(()) => {}
        Err(e) => {
            eprintln!("Error: {}", e);
            process::exit(1);
        }
    }
}
