
# Compiler design

This is a new query-based compiler architecture (for the same language), intended to eventually replace the other one.

## Properties

* This is for the 'front-end'; backend codegen is not considered _yet_.
* Query-based, e.g. 'parsed F', 'type of X', 'monomorph F for types (U, T)'
* These form a tree structure, starting with root (end result)
* To enforce tree structure, query 'kinds' are ordered and can only call 'down'
* Query results can be cached (but in some cases won't be) (even multiple versions per 'thing')
* Leafs are (only?) source files, which can have different impls (disk vs web ide)

* Questions:

* Is cache invalidation proactive (if source changes, purge anything that depends on it),
  or start from root and check all the way to leaf if anything changed (one per node per run)
  the latter only needs single-link, but keeps a 'seen' map per run; it works better with branch switching back and forst
* How does swappable codegen fit into this? Is it just separate?
* How to impl memory vs disk caching? it should keep 'most popular' in memory but put everything on disk
* How to deal with compiler vs IDE? they need different levels of detail, are they separate queries?