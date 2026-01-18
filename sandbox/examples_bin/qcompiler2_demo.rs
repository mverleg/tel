use sandbox::qcompiler2::Context;

fn main() {
    let mut my_ctx = Context::new();

    my_ctx.in_resolve("main", |ctx| {
        ctx.in_read("examples/factorial/main.telsb", |ctx| {
            ctx.in_parse("examples/factorial/main.telsb", |_ctx| {
            })
        });

        ctx.in_resolve("fact_helper", |ctx| {
            ctx.in_read("examples/factorial/fact_helper.telsb", |ctx| {
                ctx.in_parse("examples/factorial/fact_helper.telsb", |_ctx| {
                })
            })
        });

        ctx.in_exec("main", |_ctx| {
        })
    });

    println!("\n=== Dependency Graph (JSON) ===");
    println!("{}", my_ctx.to_json());
}
