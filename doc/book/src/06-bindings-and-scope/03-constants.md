# Constants

<!-- TODO: review -->

A *constant* is a binding whose value is fixed. In Tel constants are not a
special, rarely-used construct: **everything declared at the top level of a
file is a constant**.

## What

Tel has two flavours of constant binding:

- **Local constants** — an ordinary `let` binding inside a function or block.
  It is immutable (see [Let Bindings](01-let-bindings.md)) but its value is
  computed when control reaches it.
- **Top-level constants** — every binding at the top level of a file (module).
  These are always constant; there is no top-level `uniq`.

```tel
# top level of a file — all of these are constants
let MAX_RETRIES = 5
let GREETING    = "hello"
let double      = fn(x: Int64) -> Int64 { 2 * x }
```

A function defined at the top level is just a constant whose value happens to
be a function. The same is true of types and traits. There is no separate
"declaration" mechanism: a top-level name is a constant binding, and *that* is
what makes it safely importable — the binding can never be reassigned, so an
importer sees a stable value.

## Compile-time evaluation

Top-level constants are evaluated before the program's `main` runs. A constant
initialiser must therefore be computable without ambient I/O — it cannot read
the clock, the filesystem, or randomness, because those arrive only as
capabilities passed into running code (see
[antifeatures](../02-philosophy/04-antifeatures.md)).

Constant initialisers may *call functions*, including user-defined ones, as
long as the call is itself constant — see
[`const` Functions and Compile-Time Evaluation](../15-metaprogramming/04-compile-time-evaluation.md).

```tel
let TABLE = build_lookup_table()   # OK if build_lookup_table is const
```

TODO(open): whether top-level constant initialisers run in a strict *const
context* (rejecting any non-const operation) or are simply evaluated eagerly
and allowed to fail at runtime is unresolved. The lean is toward a const
context — "run funcs in const context, and just fail if they do anything
non-const". This ties into [`const` functions](../15-metaprogramming/04-compile-time-evaluation.md).

## Importing files with top-level logic

Because top-level bindings are constants, importing a file is well-behaved:
the importer pulls in named, immutable values. A file is *not* a script that
runs arbitrary statements on import.

TODO(open): what happens when a file mixes importable
constants with top-level *logic* (statements that are not bindings) — fail,
run, ignore, or wrap into `main`. It also raises whether scripts and importable
modules should be distinguished (e.g. by file extension). Unresolved; this is
partly a [modules](../11-modules-and-packages/) question. The current lean,
consistent with "everything top-level is const", is that a top-level file is a
set of constant bindings plus an optional entry point, not a sequence of
side-effecting statements.

## Why

- **Safe imports for free.** If every top-level name is an immutable constant,
  importing a module cannot smuggle in mutable shared state. This is the
  binding-level half of the [no-global-mutable-state](07-no-global-mutable-state.md)
  rule.
- **One concept, not two.** Functions, types, and value constants are all "a
  name bound to a fixed thing." Fewer rules to learn.
- **Stability.** A constant a host or another module depends on cannot shift
  under it at runtime.

## See also

- [Let Bindings](01-let-bindings.md) — local immutable bindings.
- [No Global Mutable State](07-no-global-mutable-state.md) — the `Context`
  alternative to mutable globals.
- [`const` Functions](../15-metaprogramming/04-compile-time-evaluation.md) — functions usable
  in constant initialisers.
