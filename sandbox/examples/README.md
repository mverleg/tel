# Sandbox Examples

This directory contains example programs demonstrating the 4-phase compiler with multi-file support.

## Running Examples

From the sandbox directory, you can run the examples:

```bash
cargo run --example run_factorial
cargo run --example run_fibonacci
cargo run --example run_math
```

Or using the library directly from Rust code:

```rust
use sandbox::run_file;
run_file("examples/factorial/main.lisp").unwrap();
```

## Examples

### 1. Factorial (`examples/factorial/`)

Computes factorial of 5 using recursive helper function.

Files:
- `mul.lisp` - Multiplication function
- `dec.lisp` - Decrement function
- `fact_helper.lisp` - Recursive factorial helper (imports mul and dec)
- `main.lisp` - Entry point (imports fact_helper)

Expected output: `120`

### 2. Fibonacci (`examples/fibonacci/`)

Computes first 8 Fibonacci numbers recursively.

Files:
- `add.lisp` - Addition function
- `sub.lisp` - Subtraction function
- `fib.lisp` - Recursive Fibonacci (imports add and sub)
- `main.lisp` - Entry point (imports fib, prints sequence)

Expected output:
```
0
1
1
2
3
5
8
13
```

### 3. Math Operations (`examples/math/`)

Demonstrates various math utility functions.

Files:
- `max.lisp` - Returns maximum of two numbers
- `min.lisp` - Returns minimum of two numbers
- `abs.lisp` - Returns absolute value
- `main.lisp` - Entry point (imports all, tests various cases)

Expected output:
```
20
15
42
42
-5
```

## Language Features Demonstrated

All examples demonstrate:
- **Multi-file imports**: Files can import other files using `(import filename.lisp)`
- **Function calls**: `(call funcname arg1 arg2)` calls imported functions
- **Argument access**: `(arg 1)` and `(arg 2)` access function arguments
- **Early return**: `(return value)` exits function early
- **Nested imports**: Imported files can import other files
