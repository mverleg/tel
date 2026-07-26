# Composing Tasks

<!-- TODO: review -->

A single `spawn` / `join` pair (see [tasks](02-tasks.md)) is rarely the whole
story. Real concurrent code fans work out, waits for several results, races
alternatives, or builds a small tree of dependent steps. Tel's concurrency
abstraction is designed to **compose**: the building blocks combine cleanly
into larger structures.

This topic catalogues the composition operators. They all respect
[structured concurrency](04-structured-concurrency.md) — every combinator is a
parent whose tasks are its children, errors propagate up by default, and a
failure cancels the siblings that are no longer needed.

## The four shapes

Four composition shapes the abstraction must support
cleanly. Two are over *homogeneous* tasks (same result type, variable count);
two are over *heterogeneous* tasks (different result types, fixed count).

| Shape | Input | Result |
|---|---|---|
| **Combine** | N homogeneous tasks | all N results (a collection) |
| **Race / pick-first-N** | M homogeneous tasks | the first N to finish |
| **Await-all** | fixed set of heterogeneous tasks | a tuple of their results |
| **Await-first** | fixed set of heterogeneous tasks | one result, as a union |

### Combine — N homogeneous tasks

Run the same kind of work over many inputs and collect every result. This is
the fan-out / `parallel_map` case. The combinator owns the
[spawn strategy](02-tasks.md#the-spawn-strategy) — it decides per item whether
to spawn a real task or run inline, so the script never hand-tunes that.

```tel
# Score every order; results come back in input order.
let scores: List[Score] = combine(orders, |o| score(o))
```

If any task fails, `combine` cancels the rest and propagates the failure
(unless the caller handles it at this boundary).

### Race — pick first N of M homogeneous tasks

Run several equivalent tasks and take whichever finish first; cancel the
losers.

```tel
# Query three mirrors, take the fastest answer.
let answer: Reply = race_first(mirrors, |m| query(m))

# Pick the first two of five to respond.
let quorum: List[Reply] = race_first_n(replicas, 2, |r| query(r))
```

The unfinished tasks are cancelled as soon as enough have completed — see
[cancellation](08-cancellation-and-timeouts.md).

### Await-all — heterogeneous tasks into a tuple

Wait for a fixed, statically known set of differently-typed tasks and get a
tuple back. The shape is fixed at compile time, so the result type is a tuple,
not a collection.

```tel
let prices  = tasks.spawn("prices",  || fetch_prices(mkt))
let weather = tasks.spawn("weather", || fetch_weather(loc))
let news    = tasks.spawn("news",    || fetch_news(topic))

# (PriceTable, Forecast, Headlines)
let (p, w, n) = await_all(prices, weather, news)
```

A failure in any one cancels the others and propagates.

### Await-first — heterogeneous tasks into a union

Wait for a fixed set of differently-typed tasks and take whichever finishes
first. Because the winner could be any of them, the result is an
[untagged union](../02-philosophy/03-features.md) of their types, dispatched
with `match`.

```tel
let cached = tasks.spawn("cache", || read_cache(key))   # -> CacheHit
let fresh  = tasks.spawn("fetch", || fetch_origin(key)) # -> OriginDoc

# (CacheHit | OriginDoc)
match await_first(cached, fresh) {
    hit:  CacheHit  => use_cached(hit),
    doc:  OriginDoc => use_fresh(doc),
}
```

## Building a task tree

The four shapes nest. A combinator's body may itself spawn and combine tasks,
so a script builds an arbitrary **tree** of concurrent work, not just a flat
list. Because composition is structured, the tree's lifetime and error
behaviour are exactly the [structured-concurrency](04-structured-concurrency.md)
rules applied recursively: cancelling or failing any node tears down its
subtree.

```tel
# A two-level tree: search fans out, each branch collates internally.
fn render(req: Request, tasks: Tasks) -> Page {
    let ads  = tasks.spawn("ads", || build_ads(req))
    let hits = tasks.spawn("search", || {
        # nested combine — children of the "search" task
        combine(terms(req), |t| collate(query(t)))
    })
    let (ad_links, results) = await_all(ads, hits)
    Page.of(ad_links, results)
}
```

## Dependency graphs

The hardest fan-out case is the **dependency graph** of mixed
I/O and CPU work — startup that needs to read config *and* connect to a
database *and* warm a cache, where some steps depend on the outputs of others
and others can proceed in parallel. The naive serial form is slow; the naive
"spawn everything" form deadlocks or wastes work.

In Tel, the four shapes plus structured nesting already cover this; there is
no separate "build a DAG" API.

```tel
fn warm(env: Env, tasks: Tasks) -> AppState {
    let cfg_t = tasks.spawn("config", || load_config(env))
    let cfg   = cfg_t.join()                        # everything below needs config

    # Independent given cfg — run together.
    let db_t    = tasks.spawn("db",      || connect_db(cfg.db))
    let secrets = tasks.spawn("secrets", || fetch_secrets(cfg.kms))
    let (db, sec) = await_all(db_t, secrets)

    # The cache needs both.
    let cache = build_cache(db, sec)
    AppState.of(cfg, db, cache)
}
```

The shape of the graph is expressed in the *order* of `await`s and `join`s;
no separate scheduler annotates the DAG. Two guidance points to
underline:

- **Parallelise at the highest level the dependencies allow.** Spawning
  inside a tight inner loop wastes scheduler overhead; one `combine` over the
  whole batch is usually better than ten nested ones.
- **Don't conflate "may run in parallel" with "will run in parallel".** A
  Tel script declares independence; the runtime decides whether to exploit
  it. The same code orchestrates a four-core worker pool and a
  no-concurrency host.

A separate "build a graph object, then schedule it" API — analogous to a
TensorFlow graph or a build-system DAG — is **not** part of the language. A
library can offer one for use cases (large pipelines, build orchestration)
where the dependencies are dynamic enough that direct `spawn`/`join` becomes
unreadable.

TODO(open): Whether the standard library should ship a small dependency-graph
helper (`graph.add(node, deps = [..])`, `graph.run()`) or leave it to
ecosystem libraries. This question is open. Lean:
leave it out of the core stdlib unless a clear small use-case appears.

## Implicit tree building

An ergonomic idea: instead of spelling out `spawn` and the
combinator, **mark a value as "evaluate as a task, await at first use"**. The
task starts when the binding is defined and the runtime joins it implicitly the
first time the value is read — so independent bindings naturally run
concurrently and the dependency tree is inferred from data flow. Swift's
`async let` is the reference point.

This would make the common case ("these three things are independent, run them
concurrently") require no combinator at all, while still letting an explicit
`spawn` build a lazy handle when you want to defer. One sketched
syntax sits somewhere between an assignment and an arrow — a starting marker on
the binding, plus an opt-in `go` marker on the expression that *starts*
work — so a pipeline could read as something like:

```tel
# Illustrative — syntax NOT pinned down. The `<-` marks the binding as a
# concurrent task; `go` marks the expression as a started-now sub-task.
let user_ids <- find_users(filter)
let user_pensions = user_ids.iter()
    .map(|uid| load_user_details(uid))
    .map(|user| (user, go calculate_pension(user)))
    .collect()
```

Two ergonomic wins this would buy:

- A "shallow" container of started tasks (a list of `(user, pension-task)`
  pairs above) does not eagerly resolve. The list itself is just data; the
  pensions are joined when the caller reaches for `.1`.
- The dependency tree emerges from ordinary control flow, no combinator
  scaffolding needed for the common 80%.

It is **not committed**. It is attractive but interacts badly with several
existing decisions, and the open ends are real:

- *When is an unused implicit task cancelled?* If a caller stores the value
  in a list and never reads it, does the work run anyway (wasteful) or get
  cancelled at scope end (silent dropped error)?
- *How is "shallow use" detected?* Storing the binding in a struct field or
  passing it to a generic function should not force eager evaluation, but
  the compiler must know the difference.
- *Does it interact with* [function colouring](03-async-and-function-colouring.md)?
  Implicit await at first use is functionally equivalent to inserting an
  `await` on every read of an inferred-`Future` value — see the
  rejected-automatic-`await` discussion in that topic.
- *What about error propagation?* If the implicit join surfaces an `Err`,
  does it propagate at the point of first read (surprising) or at the point
  the binding was created (impossible — the task had not run yet)?

TODO(open): Decide whether implicit "await at first use" task bindings ship.
If yes: specify when the task starts, when it is joined, what happens to one
that is never read, and how it composes with explicit combinators. If no:
keep the four explicit combinators as the whole story. Lean: keep it
explicit; the ergonomics gain is real but it muddies several other clean
stories at once.

## Open questions

- TODO(open): Names and exact signatures of `combine`, `race_first`,
  `race_first_n`, `await_all`, `await_first` are illustrative.
- TODO(open): `combine` result ordering — input order (shown here) vs
  completion order. Input order is the readable default; confirm.
- TODO(open): How `await_first`'s union result behaves when two tasks of the
  *same* type are raced — untagged unions deduplicate, so `(T | T)` is `T` and
  the winner's identity is lost. For same-type racing, `race_first` (the
  homogeneous shape) is the intended tool; document the boundary.
- TODO(open): Whether a combinator over pure (side-effect-free) tasks may
  reorder or coalesce them — "auto-reorg if the futures are
  pure." Depends on whether Tel can statically know a task body is pure —
  defer to the effects/purity discussion.
