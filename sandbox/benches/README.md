# Large Project Benchmark

This benchmark tests the Tel compiler's performance with large projects containing many modules and imports.

## What it does

The benchmark:
- Generates Tel projects deterministically using a fixed random seed
- Creates modules with functions that import from other modules
- Tests various project sizes (10 to 500 modules)
- Measures compilation and execution time
- Uses the async compilation pipeline with dependency tracking

## Running the benchmark

```bash
cd sandbox
cargo bench --bench large_project
```

## Viewing results

After running, open the HTML report at:
```
target/criterion/compile_project/report/index.html
```

## Project configurations

The benchmark tests realistic large-scale projects with shared base dependencies:
- **75k functions**: 5,000 base, 20,000 mid-level, 50,000 leaf functions
- **107k functions**: 7,000 base, 30,000 mid-level, 70,000 leaf functions
- **150k functions**: 10,000 base, 40,000 mid-level, 100,000 leaf functions

Each configuration creates a realistic DAG structure:
- Base functions have no dependencies (like stdlib)
- Mid functions depend on 1-3 base functions
- Leaf functions depend on 1-2 base and 1-2 mid functions

**Note**: The Tel compiler is extremely fast (~1.5ms for 75k functions). The bottleneck in this benchmark is temporary file generation, not compilation. You can adjust the sizes in `large_project.rs` to tune for your target runtime.

## Tuning for ~5 seconds

Your compiler is very efficient! Based on testing:
- 7,500 functions ≈ 1ms compilation
- 75,000 functions ≈ 1.5ms compilation

To reach longer benchmark times, you'll need much larger projects, but be aware that file generation becomes the bottleneck around 100k+ functions.

## Generated code structure

Each generated project contains:
- `base_N.telsb` - Base function files (no dependencies)
- `mid_N.telsb` - Mid-level function files (import from base)
- `leaf_N.telsb` - Leaf function files (import from base and mid)
- `main.telsb` - Entry point that imports and calls functions

Example base function (`base_5.telsb`):
```lisp
(+ (arg 1) (arg 2))
```

Example mid function (`mid_10.telsb`):
```lisp
(import base_3)
(import base_7)

(call base_3 (arg 1) (arg 2))
```

Example leaf function (`leaf_42.telsb`):
```lisp
(import base_2)
(import mid_10)
(import mid_5)

(call mid_10 (arg 1) (arg 2))
```

## Determinism

The benchmark uses a fixed seed (`SEED = 42`) to ensure:
- Reproducible results across runs
- Consistent import graphs
- Same function definitions

This makes it suitable for regression testing and performance comparison.
