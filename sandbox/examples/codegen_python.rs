//! Compile a Tel program to a standalone, executable Python script.
//!
//! This drives the Python backend (`src/codegen.rs`) instead of the
//! interpreter: it runs the shared front end (parse → resolve → monomorphise)
//! and then emits one `.py` file with a `#!/usr/bin/env python3` shebang, made
//! executable, so the result runs on its own with no Tel runtime.
//!
//! ```bash
//! cargo run --example codegen_python -- examples/factorial/main.telsb
//! cargo run --example codegen_python -- examples/factorial/main.telsb /tmp/factorial.py
//! ./out.py   # the generated script is directly runnable
//! ```

use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let input = args.next().unwrap_or_else(|| {
        "examples/factorial/main.telsb".to_string()
    });
    // Default output: the input's stem with a .py extension in the CWD.
    let output: PathBuf = args.next().map(PathBuf::from).unwrap_or_else(|| {
        let stem = std::path::Path::new(&input)
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())
            .unwrap_or("out");
        PathBuf::from(format!("{}.py", stem))
    });

    let python = sandbox::codegen_python_file(&input).await?;
    std::fs::write(&output, &python)?;
    make_executable(&output)?;

    println!("Wrote {} ({} bytes) — run it with ./{}",
        output.display(), python.len(), output.display());
    Ok(())
}

#[cfg(unix)]
fn make_executable(path: &std::path::Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path)?.permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(path, perms)
}

#[cfg(not(unix))]
fn make_executable(_path: &std::path::Path) -> std::io::Result<()> {
    Ok(())
}
