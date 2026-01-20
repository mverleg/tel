# Tel Sandbox

A simple Lisp-like language with a 4-phase compiler implementation.

## Language Reference

For syntax and language details, see [language.md](language.md).

## Compiler Phases

The compiler processes programs in four phases:

1. **Parse** - Tokenize source and build PreExpr AST with string names
2. **Resolve** - Convert names to unique VarId/FuncId, handle imports, check scoping rules
3. **Type Check** - (Future phase for static type checking)
4. **Execute** - Interpret the resolved AST

## Query engine features

- [x] Build depenency graph
- [ ] Inverse dependency graph
- [ ] Concurrency-safe
- [ ] Prevent ctx leak outside scope (just pure fn pointers?)
- [ ] Lock-free (during compile)
- [ ] Write using async
- [ ] Cache computation steps
- [ ] Cache IO steps
- [ ] Selective caching (e.g not file read)
- [ ] Cycle detection

## Running Programs

```rust
use sandbox::run_file;
run_file("path/to/main.telsb").unwrap();
```

Or via the examples:
```bash
cargo run --example run_factorial
cargo run --example run_fibonacci
cargo run --example run_math
```

## Examples

See the `examples/` directory for complete working programs.
