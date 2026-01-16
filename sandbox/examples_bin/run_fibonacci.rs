fn main() {
    let result = sandbox::run_file("sandbox/examples/fibonacci/main.lisp");
    match result {
        Ok(_) => {}
        Err(e) => eprintln!("Error: {}", e),
    }
}
