
# Tel

Tel (Typed Embedded Language) is a statically-typed language meant to be
embedded in host applications. This repository holds both the language
documentation and the implementation work.

## Repository layout

| path | what |
| --- | --- |
| `doc/` | the **language documentation** — the mdBook design docs (`doc/book/src`), raw notes (`doc/inputs`), examples, and the site deployment. See `doc/book/CLAUDE.md`. |
| `sandbox/`, `sandbox-daemon/` | the query-compiler sandbox: a small Lisp-like demo language used to develop the caching/dependency architecture (see below). |
| `common/`, `ast/`, `hir/`, `parser/`, `compiler/`, `cli/` | the real Tel implementation, still early. |
| `telc-cache/`, `async-lazy/`, `testing/` | supporting crates. |
| `scripts/` | repo tooling, notably `spec_links.py` (see below). |

The query engine's **design** is documented in the book, chapter
`19a-compiler-internals` — keys and fingerprints, invalidation, hashing,
determinism, concurrency and recovery, cycle detection, the numbered invariants,
and the content-addressed-vs-verifying-trace rationale. It used to live in a
top-level `docs/` directory; that directory is gone and `sandbox/` doc comments
cite the book chapter by path. The book's invariant numbering is the one those
comments cite ("invariant 6"), so renumbering it is a breaking change.

Sandbox **status** — what is built, what is next — lives in `sandbox/plans/`,
not in the book. The book documents the model and stays valid once the model is
implemented.

## Spec anchors: linking documentation to code

The documentation states the rules of the language; the code enforces them.
A **spec anchor** ties one rule to the code implementing it, so neither side
can be changed without noticing the other.

An anchor is a `SCREAMING_SNAKE_CASE` id, unique across the book, that names a
single rule — `SAME_SCOPE_REDECLARATION`, `IDENTIFIER_SHAPE`.

**In the documentation** (`doc/book/src/**.md`) the id is *declared* exactly
once, on its own line, right under the prose that states the rule:

```markdown
Re-declaring a name **in the same scope** is an error.

{{#spec SAME_SCOPE_REDECLARATION}}
```

**In the code** the id is *claimed* by every site that implements the rule —
any number of sites, since a rule is usually split over parser, resolver and
error reporting:

```rust
use tel_common::spec;

spec!(SAME_SCOPE_REDECLARATION);
spec!(IDENTIFIER_SHAPE, "ascii only; the case-twin rule is not implemented yet");
```

The macro (`common/src/spec_anchors.rs`) expands to nothing — it is a marker
for the checker, not code. Use it in item or statement position; it is not an
expression. Where a Rust macro cannot go — `.lalrpop` grammars, `.tel` sources,
shell, TOML — use the comment form, which the checker treats identically:

```text
// spec: SAME_SCOPE_REDECLARATION — rejected here, reported in resolve
```

The optional note says *what part* of the rule this site covers, or which part
is still missing. Prefer an honest note over silence: a claim that overstates
what the code does is worse than no claim.

### The check

```bash
scripts/spec_links.py                  # check both sides agree
scripts/spec_links.py --unimplemented  # + list documented rules with no code
scripts/spec_links.py --write-links    # + refresh doc/book/spec-links.json
```

It fails (exit 1) when:

- code claims an id the docs never declare — a typo, or a rule that was
  renamed or deleted in the docs;
- the same id is declared twice in the book;
- an id on either side is malformed.

A documented rule that *no* code claims is **not** an error — most of the book
describes things that are not built yet. It is reported by `--unimplemented`,
and `--strict` turns it into an error (do not use that until the compiler
catches up with the book).

Markers inside code blocks are ignored on both sides, so a page can show the
syntax literally.

### The link back

`--write-links` writes `doc/book/spec-links.json`, which the book's
`spec-anchors` preprocessor turns into "implemented in `path:line`" links on
each anchor. That file is generated, gitignored, and refreshed automatically by
`doc/k8s-deploy.sh` before every image build — so the deployed book always
links live code, and a deploy fails if docs and code have drifted apart.
Building the book without it just renders the bare anchor.

### When to add one

Add an anchor when the code makes a decision the documentation argues for —
a rejected program, a defaulting rule, a resolution order, a precedence
choice. Do not anchor plumbing that no reader would look up. When a rule and
its implementation are both touched in one change, both sides move in the same
commit; that is the whole reason the docs live in this repo.

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
3. **Type Check + Monomorphise** - Infer numeric types (i32/i64, `Number` trait bound) and specialise each function per type it is called at
4. **Execute** - Interpret the monomorphised AST

### Dependency Graph

The `Graph` structure tracks dependencies between compilation steps using a concurrent HashMap (DashMap):
- Each step (Parse, Resolve, Mono, Exec) has a unique StepId
- When one step depends on another, the dependency is registered (with reverse edges for leaf-driven invalidation)
- The graph lives in the `Arc`-shared `Global` core and is thread-safe for concurrent access

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
* Commit each change when it is complete — one commit per logical change,
  written directly on `main`. Don't leave finished work sitting uncommitted.
  Don't push unless requested.
* Don't run formatters.
