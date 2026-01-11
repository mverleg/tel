use std::path::PathBuf;

use rand::rngs::StdRng;
use rand::Rng;
use rand::SeedableRng;

use telc::tel_build_str;
use tel_ast::TelFile;
// use tel_ast_to_code::ast_to_code;  //TODO @mark: ast_to_code crate doesn't exist yet

fn main() {
    let _rng: StdRng = SeedableRng::seed_from_u64(123_456_789);
    let _file = gen_random_file(&_rng);
    // let code = ast_to_code(file);  //TODO @mark: implement ast_to_code
    // let res = tel_build_str(PathBuf::from("test-input"), code.clone(), false);
    // if res.is_err() {
    //     println!("{}", &code);
    //     panic!("failed to parse generated code")
    // }
    println!("Testing binary not yet implemented - ast_to_code crate missing");
}

fn gen_random_file(_rng: &impl Rng) -> TelFile {
    TelFile {}
}
