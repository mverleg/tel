
Summaries of todos, and my notes on their (partial) resolution. Please include in other design docs and remove the TODOs that are done.

Push back on any of these that seem questionable - leave TODOs in this file with reasons for and against

Higher-level design questions still to resolve. Inline `TODO(open):` markers in
the chapters carry the detail; this file is the tracker.

## Mutability / memory

- **Building immutable graphs with cycles.** A cycle needs a node reachable by
  two paths (aliasing), but a `uniq` (affine) value is uniquely owned, so a
  `uniq`-build-then-`finish()` is always a tree and can never close a loop — the
  difficulty is at *construction*, not at the freeze. Options: a `rec`/letrec
  knot-tying form, or construction through shared-mutable `Sync` cells. If Tel
  offers neither, immutable cycles cannot be built and every cycle in a program
  originates in a stdlib `Sync` type — which simplifies cycle reclamation.
  Portable workaround needing no cycles: ID-indirection (nodes hold `Id`s, a
  `Map[Id, Node]` resolves them). Decide whether `rec` is in scope.
  See `book/06-bindings-and-scope/02-mutability.md` and
  `book/14-concurrency-and-parallelism/07-memory-model-for-concurrency.md`.

## Top-level / script-mode code

- **Cross-file access to top-level `let`/`var`.** Open. When a script's
  top-level bindings (the body of the entry file) are visible from *other* files
  in the same module, a hazard appears: top-level bindings initialise *eagerly*
  in source order (like locals), but cross-file visibility makes them act like
  globals, so another file can observe one *before* it is initialised. In Swift
  this combination became an actual memory-safety hole (SR-3316) and the
  language now regrets it: Jordan Rose, "Swift Regret: Top-Level Decls in Script
  Mode", https://belkadan.com/blog/2021/10/Swift-Regret-Top-Level-Decls-in-Script-Mode/ .
  Tel is undecided. Options:
  (a) top-level bindings are **file-private** — never reachable from another
  file (lean; consistent with no-global-mutable-state);
  (b) only **immutable, statically-initialised** top-level constants are
  cross-file visible;
  (c) allow it but require **lazy/once** initialisation so init order cannot be
  observed.
  Decide alongside the global-mutable-state question. See
  `book/06-bindings-and-scope/07-no-global-mutable-state.md`.
