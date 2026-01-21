use sandbox::{qcompiler2, Error};
use sandbox::types::{PreExpr, Expr, SymbolTable};

fn main() {
    let result = qcompiler2::with_root_context(|ctx| {
        ctx.read("test.telsb", |ctx, _source| {
            ctx.parse("test.telsb", "", |ctx, _pre_ast| {
                ctx.resolve("add", ".", PreExpr::Number(0), |ctx, _ast, _symbols| {
                    ctx.exec("add", Expr::Number(0), &SymbolTable::new(), |ctx| {
                        println!("Simulating execution");
                        ctx.resolve("multiply", ".", PreExpr::Number(0), |ctx, _ast, _symbols| {
                            ctx.exec("multiply", Expr::Number(0), &SymbolTable::new(), |ctx| {
                                println!("Simulating execution");
                                println!("\n=== Dependency Graph with Leaf Nodes and Path ===\n");
                                println!("{}", ctx.to_json());
                                Ok::<_, Error>(())
                            })
                        })
                    })
                })
            })
        })
    });

    if let Err(e) = result {
        eprintln!("Error: {}", e);
    }
}
