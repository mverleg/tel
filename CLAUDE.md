
# Tel Sandbox Project

## Overview

Tel sandbox is a demo language designed to test and develop a query compiler architecture with dependency tracking and caching capabilities. The sandbox project implements a minimal Lisp-like language with a 4-phase compiler that demonstrates these concepts.

## Project Goals

The primary goal is to build a query compiler system where:
- All compilation steps go through a central `Context` to register dependencies
- Dependencies are tracked in a dependency graph for later analysis
- The architecture supports future caching of computation and I/O steps
- The system is concurrency-safe and designed to support lock-free operation

## Architecture

### Context-Driven Execution

All compiler operations (parse, resolve, execute) must go through the `Context` object:
- `ctx.parse(id)` - Parse a file into PreExpr AST
- `ctx.resolve(id)` - Resolve names to unique IDs and check scoping
- `ctx.execute(id)` - Execute the resolved AST

The Context automatically registers dependencies between compilation steps, building a dependency graph that can be used for:
- Analyzing compilation dependencies
- Future caching of expensive operations
- Detecting cycles
- Enabling concurrent execution

### Compiler Phases

1. **Parse** - Tokenize source and build PreExpr AST with string names
2. **Resolve** - Convert names to unique VarId/FuncId, handle imports, check scoping rules
3. **Type Check** - (Future phase for static type checking)
4. **Execute** - Interpret the resolved AST

### Dependency Graph

The `Graph` structure tracks dependencies between compilation steps using a concurrent HashMap (DashMap):
- Each step (Parse, Resolve, Exec) has a unique StepId
- When one step depends on another, the dependency is registered
- The graph is shared via Rc and thread-safe for concurrent access

## Language Features

Tel is a minimal functional language with:
- S-expression syntax
- Variables with explicit scoping rules (no shadowing)
- Functions with explicit imports and local definitions
- Basic arithmetic and control flow
- Simple I/O (print)

See `sandbox/language.md` for full language reference.

## Project Rules

* Always ask before adding new dependencies, internal and external.
* Don't create commits unless requested.
* Don't run formatters.

