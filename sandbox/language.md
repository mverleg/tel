# Tel Language Reference

Tel is a minimal functional language with S-expression syntax that compiles through four distinct phases: parsing, name resolution, type checking + monomorphisation, and execution.

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

### Types

There are two numeric types, `i64` and `i32`. Literals can carry a type
suffix; unsuffixed literals are polymorphic and get their type from inference:

```lisp
(let a 42)      # inferred; defaults to i64
(let b 42i64)   # explicitly i64
(let c 42i32)   # explicitly i32
```

**No implicit conversions.** Mixing types in an operation is a compile-time
error:

```lisp
(+ 1i32 2i64)   # error: type mismatch
```

**The Number trait.** Both types implement the built-in `Number` trait, which
is what binary operators and function type parameters require. Every function
is implicitly generic over a single type parameter `T: Number`: all of its
arguments *and* its return value have type `T`. There is no way for one
function to take or return a mix of numeric types.

**Monomorphisation.** Because of the single type parameter, each function is
compiled into at most two specialised instances — one per numeric type — and
only the instances actually reachable from `main` are created:

```lisp
(function double (* (arg 1) 2))   # the literal 2 adopts the instance's type

(print (call double 21))          # instantiates double @ i64
(print (call double 21i32))       # instantiates double @ i32
```

The instantiation type is determined by the call's arguments; a call with no
arguments (or only unconstrained literals) defaults to the enclosing
instance's type — in `main`, that is `i64`. Recursion works in both
instances, including calls that recurse at a different type.

Unsuffixed literals that end up inferred as `i32` must fit in 32 bits, and a
suffixed literal that does not fit its suffix is rejected at parse time.

Comparison and logic operators return `1` or `0` in the same type as their
operands.

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

**Import** a file-function from another file:
```lisp
(import module_name)  # Automatically appends .telsb extension
```

**Define** a local function (within a file):
```lisp
(function name body)
```

Local functions:
- Must be declared after imports and before other code
- Cannot capture variables (not closures)
- Can only be called within the same file (not exported)

**Call** a function:
```lisp
(call func_name arg1 arg2 ...)
```

**Access** function arguments (inside functions):
```lisp
(arg 1)  # First argument
(arg 2)  # Second argument
(arg 3)  # Third argument (if function uses it)
# ... and so on
```

**Return** early from a function:
```lisp
(return value)
```

**Panic** to abort the program with an error message:
```lisp
(panic)
```
This will immediately abort execution and print an error message showing the source location.

**Mark unreachable code** (compile-time check):
```lisp
(unreachable)
```
This will cause a compilation error if reached during resolution. Use this to mark code paths that should never be executed.

**Function Arity:**
- Functions can take any number of arguments (0, 1, 2, 3, etc.)
- Arity is determined by the highest `(arg N)` used in the function body
- All arguments from 1 to N must be used (no gaps allowed)
- File-function names are derived from the imported filename (without extension)

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
