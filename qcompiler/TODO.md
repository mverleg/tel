
## Would this work for the query graph?

The goal is essentially the equivalent of:

    fn parse(query: Query, engine: Engine, finish: impl FnOnce<Result<PreAST>>) {
        engine.read_file(&query.file, |source| {
            // do parsing
        });
    }
    fn resolve(query: Query, engine: Engine, finish: impl FnOnce<Result<AST>>) {
        engine.parse(&query.file, |pre_ast| {
            // imports:
            let parse_queries = pre_ast.imports.iter()
                .map(|import| determine_path(import.identifier))
                .map(|path| ImportQuery(path))
                .to_vec();
            // (how to handle circular dependencies? not relevant to example...)
            engine.list_all(parse_queries, |asts| {
                // handle the rest of the file, not shown here...            
            });
        });
    }

But with the main goal of using async magic to write flat code without callbacks.
Secondary goal is to make it much easier to await multiple heterogeneous engine tasks.

* Inside a generator step (like 'read source'), inject a 'localized' engine that contains a queryId
* When a task needs something else, it asks the engine, which uses the queryId to know what depends on what
* Tasks such as the ones in the engine return custom futures, which the function can compose and await:
  - The engine keeps state for each query: Ready(dbResultId), Pending(AsyncWaker), unknown (=not an enum variant, but possible that a query isn't known)
  - In addition, whenever an action B is requested by A (independent of state), the A->B is recorded
  - When an engine task completes, look up all its dependents, and call their Wakers (the result can be borrowed from the db)

Problems:

* How to detect cycles? Could there be a max depth, and then find the actual cycle for reporting as the stack unwinds?
* How to make the custom future drop-safe? A task would have no way to detect that all dependents are stopped, right?

Normal futures or tokio::spawn?

* Will computation be parallel without spawn? E.g. if just awaiting multiple futures? Probably not
* I think tokio::spawn tasks can't easily cancel the whole task graph
* When awaiting one dependency that doesn't need IO, this probably runs to completion immediately?
* There's only any parallelism if queries await multiple dependencies at once, right?

Not sure what thread approach is best, probably just tokio:

* Standard Tokio requires all tasks to be Send, and all shared data to be thread-safe, including the store of query->answers.
* Making the core graph walking single-threaded avoids this, but needs an exotic async runtime, cpu work must be manually spawned on thread pool, and shared data is still hard.

