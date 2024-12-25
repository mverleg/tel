
# Compiler design

This is a new query-based compiler architecture (for the same language), intended to eventually replace the other one.

Similar projects in Rust: [salsa](https://salsa-rs.netlify.app/overview), [rock](https://github.com/ollef/rock).

## Properties

* This is for the 'front-end'; backend codegen is not considered _yet_.
* Query-based, e.g. 'parsed F', 'type of X', 'monomorph F for types (U, T)'
* These form a tree structure, starting with root (end result)
* To enforce tree structure, query 'kinds' are ordered and can only call 'down' (and same level, so it is not sufficient)
* Query results are cached based on query id
  * Store multiple answers per query? e.g. when switching branch, keep old cache? how to detect which is correct quickly?
  * Per run only resolve each query once, to avoid having to traverse tree to leafs each time
  * Cache is smart - even if source leaf changed, if that doesn't change the answer of e.g. `type of X`, then dependencies of that aren't executed
* Leafs are (only?) source files, which can have different impls (disk vs web ide)
* Two versions of many queries: fast compile mode (no meta) and ide mode (full metadata)
  * The latter is also used for generating errors, so compiler will try fast mode first, and if any error, re-try in meta mode to get good messages

### Questions:

* How does swappable codegen fit into this? Is it just separate?
* How to impl memory vs disk caching? it should keep 'most popular' in memory but put everything on disk
* How to deal with compiler vs IDE? they need different levels of detail, are they separate queries?
* There are several things that introduce 'flavors', how to impl? generics, references, 
  - which source filesystem?
  - which cache?
  - compiler flags like debug/opt mode
  - compiler mode or ide mode (latter has source locations etc)
* How to do cycle detection? It could happen for e.g. circular imports, right? Keep a hashset, maybe only when dependency step is 'higher' or equal?

