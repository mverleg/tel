
# Compiler design

This is a new query-based compiler architecture (for the same Tel language), intended to eventually replace the other one.

Similar projects in Rust: [salsa](https://salsa-rs.netlify.app/overview), [rock](https://github.com/ollef/rock).
Salsa looks good, but it doesn't include IO, and queries are mutable. How does it deal with concurrency, duplicate results, and pending results? Can it store to disk?

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

### Questions

* In incremental mode (like editing one file in IDE), it's probably faster to invalidate from source file up, instead of check everything from edit down
  * Is it really a problem though? If circular imports aren't allowed, then if you only change one file and only care about problems in that file, it means you don't need to walk the whole tree
* How does swappable codegen fit into this? Is it just separate?
* How to impl memory vs disk caching? it should keep 'most popular' in memory but put everything on disk
* How to deal with compiler vs IDE? they need different levels of detail, are they separate queries?
* There are several things that introduce 'flavors', how to impl? generics, references, 
  - which source filesystem?
  - which cache?
  - compiler flags like debug/opt mode
  - compiler mode or ide mode (latter has source locations etc)
* How to do cycle detection? It could happen for e.g. circular imports, right? Keep a hashset, maybe only when dependency step is 'higher' or equal?
* How to deal with results that are being computed?
  * Could it possibly be faster to ignore races and just compute twice when it happens?
* I don't think we can handle panics right? Have to trash the whole cache in that case
* Should a parallel async runtime be used? It seems all popular Rust async runtimes use some locking anyway

### Notes

* Insert some boxes around specific awaits to move the futures onto to heap and prevent stack overflow
* Perhaps store the hashcodes in the query/answer objects 

