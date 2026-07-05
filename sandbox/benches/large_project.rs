use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use pprof::criterion::{Output, PProfProfiler};

mod project_gen;
use project_gen::{ProjectConfig, ProjectGenerator};

fn bench_compile_project(c: &mut Criterion) {
    let mut group = c.benchmark_group("compile_project");

    // Configurations with 6-level onion-shaped DAG structure:
    // - Narrow at bottom (L0: base/stdlib)
    // - Widens toward middle (L1, L2)
    // - Widest at middle (L3)
    // - Narrows toward top (L4, L5: application code)
    // This mimics real software: shared base libraries, expanding middleware, narrow apps
    let configs = vec![
        ProjectConfig {
            num_l0: 1000,
            num_l1: 3000,
            num_l2: 8000,
            num_l3: 12000,
            num_l4: 6000,
            num_l5: 2000,
        },
        ProjectConfig {
            num_l0: 2000,
            num_l1: 6000,
            num_l2: 16000,
            num_l3: 24000,
            num_l4: 12000,
            num_l5: 4000,
        },
        ProjectConfig {
            num_l0: 3000,
            num_l1: 9000,
            num_l2: 24000,
            num_l3: 36000,
            num_l4: 18000,
            num_l5: 6000,
        },
    ];

    for config in configs {
        let config_str = format!(
            "{}funcs_{}_{}_{}_{}_{}_{}",
            config.total_funcs(), config.num_l0, config.num_l1, config.num_l2,
            config.num_l3, config.num_l4, config.num_l5
        );

        group.bench_with_input(
            BenchmarkId::from_parameter(&config_str),
            &config,
            |b, config| {
                b.to_async(tokio::runtime::Runtime::new().unwrap())
                    .iter(|| async {
                        let mut generator = ProjectGenerator::new(config.clone()).unwrap();
                        let main_path = generator.generate_project().unwrap();

                        sandbox::run_file(main_path.to_str().unwrap(), false)
                            .await
                            .unwrap();

                        // Keep temp_dir alive until benchmark iteration is done
                        drop(generator);
                    });
            },
        );
    }

    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default().with_profiler(PProfProfiler::new(100, Output::Flamegraph(None)));
    targets = bench_compile_project
}
criterion_main!(benches);
