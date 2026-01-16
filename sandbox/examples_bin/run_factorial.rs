fn main() {
    let result = sandbox::run_file("sandbox/examples/factorial/main.telsb");
    match result {
        Ok(_) => {}
        Err(e) => eprintln!("Error: {}", e),
    }
}
