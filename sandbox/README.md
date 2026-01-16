# Tel Sandbox

A simple Lisp-like language with a 4-phase compiler implementation.

## Language Overview

Tel is a minimal functional language with S-expression syntax that compiles through four distinct phases: parsing, name resolution, type checking (future), and execution.

## Syntax

All expressions use S-expression syntax with parentheses:

```lisp
(operator arg1 arg2 ...)
```

### Comments

Comments start with `#` and continue to the end of the line:

```lisp
# This is a comment
(let x 42)  # Inline comment
```

### Variables

**Declare** a new variable with `let`:
```lisp
(let x 10)
```

**Reassign** an existing variable with `set`:
```lisp
(set x 20)
```

**Important scoping rules:**
- `let` creates a NEW variable and forbids shadowing (error if name exists in any scope)
- `set` updates an EXISTING variable (searches parent scopes)
- Variables declared in inner scopes (like inside `if`) are not accessible outside
- Both `let` and `set` value expressions can reference outer scope variables

### Arithmetic and Logic

Binary operators:
```lisp
(+ 1 2)     # Addition: 3
(- 5 3)     # Subtraction: 2
(* 4 5)     # Multiplication: 20
(/ 10 2)    # Division: 5
(> 5 3)     # Greater than: 1 (true)
(< 5 3)     # Less than: 0 (false)
(== 5 5)    # Equality: 1 (true)
(&& 1 1)    # Logical AND
(|| 0 1)    # Logical OR
```

### Control Flow

Conditional expressions:
```lisp
(if condition then-expr else-expr)
```

Example:
```lisp
(if (> x 0)
    (print x)
    (print (- 0 x)))
```

### Functions

**Import** a function from another file:
```lisp
(import module_name)  # Automatically appends .telsb extension
```

**Call** a function:
```lisp
(call func_name arg1 arg2)
```

**Access** function arguments (inside function files):
```lisp
(arg 1)  # First argument
(arg 2)  # Second argument
```

**Return** early from a function:
```lisp
(return value)
```

All functions take exactly 2 arguments. The function name is derived from the imported filename (without extension).

### I/O

Print a value:
```lisp
(print expression)
```

### Sequences

Multiple expressions are evaluated in order:
```lisp
(let x 5)
(let y 10)
(print (+ x y))
```

## Example Program

```lisp
# factorial/fact_helper.telsb
(import mul)
(import dec)

(if (<= (arg 1) 0)
    (return (arg 2))
    (call fact_helper (call dec (arg 1) 0) (call mul (arg 2) (arg 1))))
```

```lisp
# factorial/main.telsb
(import fact_helper)

(let n 5)
(let result (call fact_helper n 1))
(print result)  # Outputs: 120
```

## Compiler Phases

The compiler processes programs in four phases:

1. **Parse** - Tokenize source and build PreExpr AST with string names
2. **Resolve** - Convert names to unique VarId/FuncId, handle imports, check scoping rules
3. **Type Check** - (Future phase for static type checking)
4. **Execute** - Interpret the resolved AST

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

See the `examples/` directory for complete working programs demonstrating:
- Recursive functions
- Multi-file imports
- Nested function calls
- Conditional logic
