use sandbox::qcompiler2::Context;

fn main() {
    let my_ctx = Context::root();

    my_ctx.in_read("test.telsb", |ctx| {
        ctx.in_parse("test.telsb", |ctx| {
            ctx.in_resolve("add", |ctx| {
                ctx.in_exec("add", |_ctx| {
                    println!("Simulating execution");
                })
            });
            ctx.in_resolve("multiply", |ctx| {
                ctx.in_exec("multiply", |_ctx| {
                    println!("Simulating execution");
                })
            })
        })
    });

    println!("\n=== Dependency Graph with Leaf Nodes and Path ===\n");
    println!("{}", my_ctx.to_json());
}
