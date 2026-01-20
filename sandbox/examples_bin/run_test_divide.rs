fn main() {
    let result = sandbox::run_file("sandbox/examples/panic_test/test_divide.telsb");
    match result {
        Ok(_) => {}
        Err(e) => eprintln!("Error: {}", e),
    }
}
