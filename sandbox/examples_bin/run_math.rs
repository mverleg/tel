fn main() {
    let result = sandbox::run_file("sandbox/examples/math/main.lisp");
    match result {
        Ok(_) => {}
        Err(e) => eprintln!("Error: {}", e),
    }
}
