extern crate lalrpop;

use std::env;
use std::fmt::Write;
use std::fs;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=./build.rs");
    generate_example_parse_tests();
}

fn generate_example_parse_tests() {
    let examples = PathBuf::from("./examples");
    println!("cargo:rerun-if-changed={}", examples.to_str().unwrap());
    let mut test_code = "// Generated
    use std::path::PathBuf;
    use std::fs::read_to_string;\n\n"
        .to_owned();
    let mut test_cnt = 0;
    for pth in fs::read_dir(&examples).unwrap() {
        let pth = pth.unwrap().path();
        let pth_str = pth.to_str().unwrap();
        if !pth.is_file() || pth.extension() != Some("tel".as_ref()) {
            println!("skipping test generation for '{}' in examples dir", pth_str);
            continue;
        }
        test_cnt += 1;
        let name = pth.file_stem().unwrap().to_str().unwrap().replace('-', "_");
        write!(
            test_code,
            "#[test]
fn parse_{name}() {{
    // generated!
    let pth = PathBuf::from(\"{pth_str}\");
    let code = read_to_string(&pth).unwrap();
    // str_to_ast, ast_to_api, get_test_modes should be available in the context where this is included
    let mode = get_test_modes(&code);
    let parse_res = str_to_ast(pth, code);
    assert!(!mode.should_fail);  // TODO @mark
    if let Err(TelErr::ParseErr {{ msg, .. }}) = &parse_res {{
        eprintln!(\"Failed to parse example file {pth_str}:\\n{{}}\", msg);
    }}
    if mode.parse_only {{
        println!(\"parsing only\");
        return;
    }}
    let api_res = ast_to_api(parse_res.unwrap());
    if let Err(TelErr::ParseErr {{ msg, .. }}) = &api_res {{
        eprintln!(\"Failed to resolve scopes for example file {pth_str}:\\n{{}}\", msg);
    }}
}}\n\n"
        )
        .unwrap();
        //eprintln!("{} {:?}", pth.to_string_lossy(), pth.file_stem().unwrap())
    }
    if test_cnt == 0 {
        panic!("did not find any examples to use for parsing tests");
    }
    let mut out_file = PathBuf::from(env::var("OUT_DIR").unwrap());
    out_file.push("parse_tests.rs");
    fs::write(out_file, test_code).expect("failed to write");
}
