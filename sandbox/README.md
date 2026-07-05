# Tel Sandbox

A simple Lisp-like language with a 4-phase compiler implementation.

## Language Reference

For syntax and language details, see [language.md](language.md).

## Compiler Phases

The compiler processes programs in four phases:

1. **Parse** - Tokenize source and build PreExpr AST with string names
2. **Resolve** - Convert names to unique VarId/FuncId, handle imports, check scoping rules
3. **Type Check + Monomorphise** - Infer numeric types (i32/i64, `Number` trait bound) and specialise each function per type it is called at
4. **Execute** - Interpret the monomorphised AST

## Query engine features

Status list below; for the *ordered* execution plan (with rationale, the
caching design, and gaps vs `qcompiler`), see [plans/](plans/) —
start with [plans/roadmap.md](plans/roadmap.md).

- [x] Build dependency graph
- [x] Force always going through context
- [x] Process imports in parallel
- [x] What if same task twice in parallel? and recursion?
- [x] Inverse dependency graph
- [x] Concurrency-safe
- [ ] Prevent ctx leak outside scope (just pure fn pointers?)
- [ ] Lock-free (during compile)
- [x] Write using async
- [x] Cache computation steps (parse/resolve/mono answers are content-cached, keys chained through dep answer fingerprints; exec deliberately stays uncached — its value is its side effects)
- [x] Cache IO steps (read stays fused with parse: the cached IO answer is the parse-level result; the read itself re-runs each compile to derive the lookup digest)
- [x] Early cutoff via result fingerprints (a formatting-only edit re-parses one file and recomputes nothing above it — Scenario B; errors are fingerprinted answers too, so dependents of stably-failing steps are cached as well)
- [ ] Store cache in LMDB (with postcard)
- [x] Include schema hash in file cache
- [x] Incremental compile starting from main (a `Compiler` is a persistent, droppable process — no more `Box::leak`; each run is a demand-driven pull from the entry point, and re-resolves replace their graph edges so restructured imports leave no zombies)
- [x] Incremental compile starting from leafs (explicit `invalidate(path)` + `run_watch`: reverse-edge cone marking, dirty cleared only on successful whole-record commit, `catch_unwind` keeps panicking nodes dirty; OS file watcher itself still pending — new dependency)
- [x] Selective caching (e.g not file read) (policy: file reads and exec side effects are never cached; deterministic errors are)
- [x] Cycle detection

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

## Profiling

Benchmarks show excellent performance: 107k functions compile in 1.82s.

### Quick Profiling with samply (recommended)

```bash
cargo install samply
samply record cargo run --example profile_run --release
```

This opens an interactive flamegraph in your browser.

### Alternative: cargo-flamegraph

```bash
cargo install flamegraph
cargo flamegraph --example profile_run -o flamegraph.svg
firefox flamegraph.svg
```

### Benchmark Reports

Detailed criterion benchmark HTML reports are at `target/criterion/compile_project/report/index.html`.
