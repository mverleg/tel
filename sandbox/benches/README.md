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

The benchmark tests realistic large-scale projects with an onion-shaped dependency structure:
- **32k functions**: 1k → 3k → 8k → 12k → 6k → 2k (L0-L5)
- **64k functions**: 2k → 6k → 16k → 24k → 12k → 4k (L0-L5)
- **96k functions**: 3k → 9k → 24k → 36k → 18k → 6k (L0-L5)

Each configuration creates a realistic 6-level onion-shaped DAG:
- **L0** (base): No dependencies, simple stdlib-like functions
- **L1**: Depends on 2-4 L0 functions
- **L2**: Depends on 1-2 L0 + 2-3 L1 functions (widening)
- **L3** (widest): Depends on 1-2 L0 + 1-2 L1 + 2-3 L2 functions
- **L4**: Depends on 1-2 L1 + 1-2 L2 + 2-3 L3 functions (narrowing)
- **L5** (leaf): Depends on 1-2 L2 + 1-2 L3 + 2-3 L4 functions

This mimics real software architecture: shared base libraries, expanding middleware layers, and narrow application code.

**Note**: The Tel compiler is extremely fast. With more complex function bodies (nested expressions, conditionals, multiple let bindings), you'll get more realistic compilation workload compared to trivial single-operation functions.

## Complexity levels

Generated functions include 4 complexity levels:
1. **Simple**: Direct operations with nested expressions
2. **Medium**: One or more let bindings with local computations
3. **Complex**: Multiple let bindings and chained operations
4. **Very complex**: Conditional logic with nested function calls

This creates a more realistic benchmark that tests parsing, name resolution, and execution of non-trivial code patterns.

## Generated code structure

Each generated project contains:
- `l0_N.telsb` - Level 0 base functions (no dependencies)
- `l1_N.telsb` through `l5_N.telsb` - Higher level functions (import from previous levels)
- `main.telsb` - Entry point that imports and calls functions from all levels

Functions are generated with varying complexity including:
- Nested arithmetic expressions
- Multiple let bindings
- Conditional logic (if statements)
- Chained and parallel function calls
- Realistic computation patterns

Example base function (`l0_5.telsb`) - complex variant:
```lisp
(let check (+ (* (arg 1) 42) (arg 2)))
(if (< check 50)
  (+ (arg 1) (* (arg 2) 3))
  (* check (arg 1)))
```

Example mid-level function (`l2_10.telsb`):
```lisp
(import l0_3)
(import l1_7)
(import l1_12)

(let r1 (call l0_3 (arg 1) (arg 2)))
(let r2 (call l1_7 (arg 2) (arg 1)))
(* r1 r2)
```

Example leaf function (`l5_42.telsb`):
```lisp
(import l2_15)
(import l3_8)
(import l4_23)
(import l4_45)

(let val (call l3_8 (arg 1) (arg 2)))
(if (< val 30)
  (call l4_23 val (arg 1))
  (call l4_45 (arg 2) val))
```

## Determinism

The benchmark uses a fixed seed (`SEED = 42`) to ensure:
- Reproducible results across runs
- Consistent import graphs
- Same function definitions

This makes it suitable for regression testing and performance comparison.
