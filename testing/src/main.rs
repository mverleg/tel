use ::std::path::PathBuf;

use ::rand::Rng;
use ::rand::rngs::StdRng;
use ::rand::SeedableRng;

use ::tel::tel_build_str;
use ::tel_api::telir::TelFile;
use ::tel_ast_to_code::ast_to_code;

fn main() {
    let rng: StdRng = SeedableRng::seed_from_u64(123_456_789);
    let file = gen_random_file(&rng);
    let code = ast_to_code(file);
    tel_build_str(PathBuf::from("script-input"), code, false).unwrap();
    unimplemented!();
}

fn gen_random_file(rng: &impl Rng) -> TelFile {
    TelFile {}
}
