use sandbox::qcompiler2::CompilationLog;

fn main() {
    let mut my_log = CompilationLog::new();

    my_log.log_read("examples/factorial/main.telsb");
    my_log.log_parse("examples/factorial/main.telsb");

    my_log.log_read("examples/factorial/fact_helper.telsb");
    my_log.log_parse("examples/factorial/fact_helper.telsb");

    my_log.log_resolve("fact_helper");
    my_log.log_resolve("main");

    my_log.log_exec("main");

    println!("\n=== Complete compilation log (JSON) ===");
    println!("{}", my_log.to_json());
}
