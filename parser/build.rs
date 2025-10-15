extern crate lalrpop;

fn main() {
    println!("cargo:rerun-if-changed=./build.rs");
    println!("cargo:rerun-if-changed=./src/grammar.lalrpop");
    lalrpop::process_root().unwrap();
}
