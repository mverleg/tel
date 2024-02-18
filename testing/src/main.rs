use ::rand::Rng;
use ::rand::rngs::StdRng;
use ::rand::SeedableRng;

use tel_api::telir::TelFile;

fn main() {
    let rng: StdRng = SeedableRng::seed_from_u64(123_456_789);
    let file = gen_random_file(&rng);
    unimplemented!();
}

fn gen_random_file(rng: &impl Rng) -> TelFile {
    TelFile {}
}
