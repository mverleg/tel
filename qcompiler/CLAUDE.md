# Qcompiler Architecture Overview

This is a query-based incremental compiler for the Tel language, intended to replace the existing compiler. Similar to Rust projects like Salsa and Rock, but with custom requirements around IO and immutability.

## Core Architecture

### Query System

Queries represent compilation tasks such as:
- `parsed F` - parse file F
- `type of X` - determine type of X
- `monomorph F for types (U, T)` - monomorphize function F for specific types

These queries form a **tree structure** starting from the root (end result).

**Ordering constraints:**
- Query 'kinds' are ordered and can only call 'down' the tree (and same level)
- This enforces the tree structure

**Caching:**
- Results are cached based on query ID
- Each query is resolved once per run to avoid traversing to leafs each time
- Smart caching: if a source leaf changes but doesn't affect a query result (e.g. `type of X`), dependents of that query aren't re-executed

**Leaf nodes:**
- Only source files are leafs
- Different implementations possible: disk vs web IDE

## Compilation Modes

Two versions of many queries exist:

1. **Fast mode** - no metadata, used for initial compilation
2. **IDE mode** - full metadata including source locations

The compiler tries fast mode first. If any errors occur, it retries in IDE mode to generate good error messages. IDE mode is also used for IDE features.

## Async/Await Design

The goal is to replace callback-style code with flat async/await code.

**Instead of callbacks:**
```rust
fn resolve(query: Query, engine: Engine, finish: impl FnOnce<Result<AST>>) {
    engine.parse(&query.file, |pre_ast| {
        // nested callbacks...
    });
}
```

**Use async/await:**
- Inject 'localized' engine with queryId into each generator step
- Tasks ask the engine for dependencies, which tracks what depends on what
- Custom futures are returned that can be composed and awaited

**Engine state per query:**
- `Ready(dbResultId)` - result computed and stored
- `Pending(AsyncWaker)` - computation in progress
- Unknown - query not yet started

**Dependency tracking:**
- Whenever action B is requested by A, the A→B relationship is recorded
- When an engine task completes, look up all its dependents and call their Wakers
- Results can be borrowed from the database

**Threading:**
- Likely standard Tokio runtime
- Requires Send constraints and thread-safe shared data (query→answer store)

## Dependency Graph

The dependency graph must be **bidirectional**:

**Forward direction (root → leafs):**
- Query A needs to know what it depends on
- Naturally encoded in query execution code

**Reverse direction (leafs → root):**
- When a leaf changes (e.g. source file edited), need to know "what depends on this?"
- For each query B, track all queries A where A→B
- Enables invalidation: walk reverse edges from changed leaf to mark affected queries as dirty

This supports both:
- **Top-down execution**: Start from root, traverse forward to compute results
- **Bottom-up invalidation**: When file changes, invalidate dependents by walking reverse edges
