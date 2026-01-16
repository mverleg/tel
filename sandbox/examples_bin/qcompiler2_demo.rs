use sandbox::qcompiler2::CompilationLog;

fn main() {
    let mut my_log = CompilationLog::new();

    my_log.in_resolve("main", |log| {
        log.in_read("examples/factorial/main.telsb", |log| {
            log.in_parse("examples/factorial/main.telsb", |_log| {
            })
        });

        log.in_resolve("fact_helper", |log| {
            log.in_read("examples/factorial/fact_helper.telsb", |log| {
                log.in_parse("examples/factorial/fact_helper.telsb", |_log| {
                })
            })
        });

        log.in_exec("main", |_log| {
        })
    });

    println!("\n=== Dependency Graph (JSON) ===");
    println!("{}", my_log.to_json());
}
