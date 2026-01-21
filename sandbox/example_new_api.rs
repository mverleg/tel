// Example of the new forced-nesting API

use sandbox::qcompiler2;

fn main() {
    // The ONLY way to get a context is through with_root_context
    let result = qcompiler2::with_root_context(|ctx| {
        // Now we MUST nest operations using closures
        ctx.read("examples/factorial/main.telsb", |ctx, source| {
            // ctx here is inside the Read step
            ctx.parse("examples/factorial/main.telsb", &source, |ctx, pre_ast| {
                // ctx here is inside the Parse step (which is inside Read)
                ctx.resolve("main", "examples/factorial", pre_ast, |ctx, ast, symbols| {
                    // ctx here is inside the Resolve step
                    ctx.exec("main", ast, &symbols, |_ctx| {
                        // ctx here is inside the Exec step
                        Ok::<_, sandbox::Error>(())
                    })
                })
            })
        })
    });

    match result {
        Ok(_) => println!("Success!"),
        Err(e) => eprintln!("Error: {}", e),
    }
}

// The following patterns are NOW IMPOSSIBLE:

// ❌ Can't create a context directly
// let mut ctx = qcompiler2::Context::root();  // Error: root() is private

// ❌ Can't call operations sequentially at the same level
// ctx.read(path, |ctx, source| Ok(source))?;
// ctx.parse(path, &source, |ctx, ast| Ok(ast))?;  // Error: ctx is borrowed!

// ❌ Can't pass context across closure levels
// ctx.read(path, |ctx1| {
//     ctx1.parse(path, source, |ctx2| {
//         // Can use ctx2 ✓
//         // CANNOT use ctx1 ✗ - it's mutably borrowed by ctx2!
//     })
// })
