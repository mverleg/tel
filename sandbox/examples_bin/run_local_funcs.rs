fn main() {
    let result = sandbox::run_file("sandbox/examples/local_funcs/main.telsb");
    match result {
        Ok(_) => {}
        Err(e) => eprintln!("Error: {}", e),
    }
}
