use std::env;
use std::fs;
use std::io::Write;
use std::path::Path;

fn main() {
    let my_out_dir = env::var("OUT_DIR").unwrap();
    let my_dest_path = Path::new(&my_out_dir).join("generated_example_tests.rs");
    let mut my_file = fs::File::create(&my_dest_path).unwrap();

    let my_examples_dir = Path::new("examples");

    if !my_examples_dir.exists() {
        return;
    }

    writeln!(my_file, "// Auto-generated tests for examples").unwrap();
    writeln!(my_file).unwrap();

    let mut my_entries: Vec<_> = fs::read_dir(my_examples_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .collect();

    my_entries.sort_by_key(|e| e.file_name());

    for my_entry in my_entries {
        let my_example_name = my_entry.file_name();
        let my_example_name_str = my_example_name.to_string_lossy();

        if my_example_name_str == "README.md" {
            continue;
        }

        // Skip examples that are meant to test error conditions
        if my_example_name_str.ends_with("_test") {
            continue;
        }

        let my_example_dir = my_entry.path();

        // Find all .telsb files that look like test entry points (main.telsb or test_*.telsb)
        if let Ok(my_files) = fs::read_dir(&my_example_dir) {
            let mut my_test_files: Vec<_> = my_files
                .filter_map(|f| f.ok())
                .filter(|f| {
                    let my_name = f.file_name();
                    let my_name_str = my_name.to_string_lossy();
                    my_name_str.ends_with(".telsb") &&
                    (my_name_str == "main.telsb" || my_name_str.starts_with("test_"))
                })
                .collect();

            my_test_files.sort_by_key(|f| f.file_name());

            for my_test_file in my_test_files {
                let my_file_name = my_test_file.file_name();
                let my_file_name_str = my_file_name.to_string_lossy();
                let my_file_stem = my_file_name_str.strip_suffix(".telsb").unwrap();

                let my_test_name = if my_file_stem == "main" {
                    my_example_name_str.to_string()
                } else {
                    format!("{}_{}", my_example_name_str, my_file_stem)
                };

                let my_rel_path = format!("examples/{}/{}", my_example_name_str, my_file_name_str);

                writeln!(my_file, "#[tokio::test]").unwrap();
                writeln!(my_file, "async fn test_example_{}() {{", my_test_name).unwrap();
                writeln!(my_file, "    let my_result = sandbox::run_file(\"{}\").await;", my_rel_path).unwrap();
                writeln!(my_file, "    assert!(my_result.is_ok(), \"Example {} failed: {{:?}}\", my_result.err());", my_test_name).unwrap();
                writeln!(my_file, "}}").unwrap();
                writeln!(my_file).unwrap();
            }
        }
    }

    println!("cargo:rerun-if-changed=examples");
}
