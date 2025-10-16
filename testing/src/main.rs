use ::std::path::PathBuf;

use ::rand::Rng;
use ::rand::rngs::StdRng;
use ::rand::SeedableRng;

use ::tel::tel_build_str;
use ::tel_ast::TelFile;
use ::tel_ast_to_code::ast_to_code;

fn main() {
    let rng: StdRng = SeedableRng::seed_from_u64(123_456_789);
    let file = gen_random_file(&rng);
    let code = ast_to_code(file);
    let res = tel_build_str(PathBuf::from("test-input"), code.clone(), false);
    if res.is_err() {
        println!("{}", &code);
        panic!("failed to parse generated code")
    }
}

fn gen_random_file(_rng: &impl Rng) -> TelFile {
    TelFile {}
}
