extern crate lalrpop;

use ::std::env;
use ::std::fs;
use ::std::path::PathBuf;
use ::std::fmt::Write;

fn main() {
    println!("cargo:rerun-if-changed=./build.rs");
    lalrpop::process_root().unwrap();
    generate_example_parse_tests();
}

fn generate_example_parse_tests() {
    let examples = PathBuf::from("./examples");
    let mut test_code = "// Generated
#[cfg(test)]
mod tests {
    use super::*;
    use ::std::path::PathBuf;
    use ::std::fs::read_to_string;\n\n".to_owned();
    let mut test_cnt = 0;
    for pth in fs::read_dir(&examples).unwrap() {
        //TODO @mark: should just refer to directory (https://github.com/rust-lang/cargo/issues/2599#issuecomment-1119059540) but that trigger refresh each time for some reason
        let pth = pth.unwrap().path();
        println!("cargo:rerun-if-changed={}", pth.to_str().unwrap());
        if ! pth.is_file() || pth.extension() != Some("steel".as_ref()) {
            println!("skipping test generation for '{}' in examples dir", pth.to_string_lossy());
            continue
        }
        test_cnt += 1;
        let name = pth.file_stem().unwrap().to_str().unwrap();
        write!(test_code, "#[test]
fn parse_{name}() {{
    let pth = PathBuf::from(\"{}\");
    let code = read_to_string(&pth).unwrap();
    // parse_str should be available in the context where this is included
    let res = parse_str(pth, &code).unwrap();
}}\n\n", pth.to_str().unwrap()).unwrap();
        //eprintln!("{} {:?}", pth.to_string_lossy(), pth.file_stem().unwrap())
    }
    write!(test_code, "}}").unwrap();
    if test_cnt == 0 {
        panic!("did not find any examples to use for parsing tests");
    }
    let mut out_file = PathBuf::from(env::var("OUT_DIR").unwrap());
    out_file.push("parse_tests.rs");
    fs::write(out_file, test_code).expect("failed to write");
}
