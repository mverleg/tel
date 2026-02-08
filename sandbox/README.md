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

- [x] Build dependency graph
- [x] Force always going through context
- [x] Process imports in parallel
- [ ] What if same task twice in parallel? and recursion?
- [ ] Inverse dependency graph
- [x] Concurrency-safe
- [ ] Prevent ctx leak outside scope (just pure fn pointers?)
- [ ] Lock-free (during compile)
- [ ] Write using async
- [ ] Cache computation steps
- [ ] Cache IO steps
- [ ] Store cache in LMDB (with postcard)
- [ ] Include schema hash in file cache
- [ ] Incremental compile starting from main
- [ ] Incremental compile starting from leafs
- [ ] Selective caching (e.g not file read)
- [ ] Cycle detection

## Running Programs

```rust
use sandbox::run_file;
run_file("path/to/main.telsb", false).unwrap();
// Or with dependency graph:
run_file("path/to/main.telsb", true).unwrap();
```

Or via the examples:
```bash
cargo run --example run_factorial
cargo run --example run_fibonacci
cargo run --example run_math
```

## Examples

See the `examples/` directory for complete working programs.
