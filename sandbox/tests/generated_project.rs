//! Regression guard for generated-project compilation.
//!
//! The criterion benchmark compiles large generated projects, but benches are
//! not part of `cargo test` — when the "Function arity" rule landed, the
//! generator started producing rejected programs (`ArityGap`: a body using
//! `(arg 2)` but never `(arg 1)`) and every bench run has failed since,
//! unnoticed by the green test suite. This test compiles the same generator's
//! output (shared module, shrunk config) so that class of breakage fails
//! `cargo test`.
//!
//! The config keeps `num_l0 >= 401` deliberately: base-function bodies depend
//! only on their index (fresh seeded RNG, generated first), so `l0_400` — the
//! exact function the unfixed generator emitted with a 2-without-1 arg gap —
//! is byte-identical here to the one that broke the real benchmark.

#[path = "../benches/project_gen/mod.rs"]
mod project_gen;

use project_gen::{ProjectConfig, ProjectGenerator};
use sandbox::{Compiler, NoopPrinter};
use std::sync::Arc;

#[tokio::test(flavor = "multi_thread")]
async fn generated_project_compiles_and_recompiles_from_cache() {
    let mut generator = ProjectGenerator::new(ProjectConfig {
        num_l0: 450,
        num_l1: 300,
        num_l2: 200,
        num_l3: 150,
        num_l4: 80,
        num_l5: 40,
    })
    .unwrap();
    let main_path = generator.generate_project().unwrap();
    let path = main_path.to_str().unwrap();

    let mut compiler = Compiler::new(Arc::new(NoopPrinter));
    compiler.run(path, false).await.unwrap();
    assert!(
        compiler.computed_parse_count() > 100,
        "the demanded cone should cover a substantial part of the project, \
         or this guard is not exercising anything (got {} parses)",
        compiler.computed_parse_count()
    );

    // At scale too: an unchanged recompile must be pure cache hits.
    let (parses, resolves, monos) = (
        compiler.computed_parse_count(),
        compiler.computed_resolve_count(),
        compiler.computed_mono_count(),
    );
    compiler.run(path, false).await.unwrap();
    assert_eq!(compiler.computed_parse_count(), parses, "no re-parse on unchanged recompile");
    assert_eq!(compiler.computed_resolve_count(), resolves, "no re-resolve on unchanged recompile");
    assert_eq!(compiler.computed_mono_count(), monos, "no re-mono on unchanged recompile");

    drop(generator);
}
