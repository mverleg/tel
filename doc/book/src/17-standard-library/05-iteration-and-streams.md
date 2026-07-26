# Iteration and Streams

<!-- TODO: review -->

## What

`std` provides lazy iteration — pipelines of `map`, `keep` (filter), `fold`,
and friends over collections and generators — plus *streams*, restricted
computation graphs that connect a producer to a consumer. The library aims
to make pipelines that read like prose, fuse for performance where the
implementation can see end-to-end, and stay safe over fallible operations.

## External vs internal iteration

Iteration comes in two flavours, and `std` exposes both:

- **External iteration** — the user pulls values (`for x in xs`, an
  explicit `iter.next()`). Strengths: precise control over control flow
  (handle the first element specially, break early, parallelise across
  consumers), the compiler can prove the last `next()` is *really* last and
  consume the iterator's state by-move, and external iterators compose
  ergonomically.
- **Internal iteration** — the iterator pushes values into a callback
  (`xs.for_each(|x| ...)`). Strengths: much easier to optimise — for a
  simple array these match the external form, but for tree-shaped sources
  and combinators like `chain` the internal form skips bookkeeping that
  external iteration cannot always elide. The cost is weaker control flow
  (early-exit must be encoded in a return value the iterator may or may
  not respect).

The library defaults to external iteration — it composes better with the
rest of the language — and exposes internal iteration through a clearly
named family (`for_each`, `inspect`, `drain_into`). `TODO(open): whether
the language exposes one *protocol* both styles desugar to, or two; whether
streams (below) are simply a renaming of internal iteration.`

## Naming: `keep`, not `filter`

`filter` is a name that invites the wrong guess — does
`filter(predicate)` keep the matches or remove them? Tel leans toward `keep`
(and a matching `drop`/`discard`) so the name states the direction. The
language can still surface `filter` as a *suggested rename* through the editor
hint mechanism — see
[`../18-tooling/09-editor-integration.md`](../18-tooling/09-editor-integration.md).
`TODO(open): confirm the final iterator method names.`

## The adapter set

Beyond `map` / `keep` / `fold`, `std` offers an opinionated but
generous catalogue of adapters. The lean is to include all of these; final
names are not yet settled.

- **`inspect(|x| ...)`** — call a void function for each element, pass the
  value through unchanged. The standard place to insert logging, counters,
  timing, or in-place tweaks without breaking the pipeline. Also a foothold
  for the editor's "show me what flows here" tooling.
- **`chunks(n)` / `windows(n)`** — non-overlapping vs sliding views of
  fixed size; both flavours are needed often enough.
- **`pairs()`** — convenience for `windows(2)` returning tuples; the
  borrowing variant is restricted (see below).
- **`separate_by(predicate)`** (working name; `split` for slices is the
  Rust precedent) — group elements between delimiters, e.g. blocks of lines
  separated by blanks. `TODO(open): pick the final name; `chunk_by` is also
  in the running.`
- **`select(Variant)`** — for an iterator of a union/enum type, keep only
  values of one variant *and* unwrap them. Saves the `match`-and-keep
  dance. `TODO(open): name clash with `async select` — find a distinct
  name.`
- **`append(...) / prepend(...)`** — extend an iterator at either end
  without materialising it.
- **`chain(other)`** — concatenate two iterators.
- **`first` / `body` / `last` markers** — an adapter that wraps each
  element with whether it is the first, middle, or last item, so a
  pipeline can render commas, indentation, or punctuation correctly.
- **`map_accum(state, |s, x| ...)`** — like `map` but threads an
  accumulator (Ramda's `mapAccum`), useful for stateful transforms that
  shouldn't reach for shared mutable state.
- **`map_values(|v| ...) / map_keys(|k| ...)`** — for map-typed sources,
  transform one side while preserving the other.
- **Opposites of `flatten`** — `unfold`, `group_by`, `partition`, so the
  pipeline can grow structure as well as flatten it.
- **`partition_by` / list-partition** — split a list into the elements
  that match and the elements that do not, in one pass.
- **Quantifiers** — `all`, `any`, `none`, plus a `count_where(pred)` for
  the count form. Defined on iterators of any element type with a
  predicate, not specialised to booleans.
- **`coalesce()`** — first non-`None` (likely also as a prelude free
  function, see [`03-prelude.md`](03-prelude.md)).

`TODO(open): the catalogue is large; vet each entry against "one good way
over many clever ones" once the iterator protocol is firm.`

## Fallible iteration

A pipeline whose source or middle step can fail must not silently swallow
errors. `std` follows the *fallible iterator* pattern: an iterator of
`Result[T, E]` keeps a single short-circuit story across all adapters, so
the first `Err` ends the pipeline and propagates. The library also offers
explicit `try_*` variants where the user wants to collect errors instead of
short-circuit. The motivating prior art is the Rust *fallible iterator*
discussion. `TODO(open): the exact propagation contract — short-circuit by
default, opt-in collect — needs to be pinned down alongside the error-
handling chapter and the prelude combinators on `Result`.`

## Streams as restricted computation graphs

A *stream* connects a producer and a consumer as a pipeline. Because a stream
describes a closed computation — no arbitrary host calls injected mid-pipeline
— it is a natural unit for the implementation to fuse and, where the data and
operations allow, vectorize. This is the same motivation as the transposed
collections in [`04-core-collections.md`](04-core-collections.md): give the
compiler a chunk of work it can see end-to-end.

Streams should be reachable from a collection without ceremony (`xs.stream()`
or implicit) and should expose roughly the same adapter set as iterators,
plus stream-only operations:

- **Chain streams** — concatenate, like iterators.
- **Per-value side effects** — the `inspect` pattern, on streams.
- **`gather`-style flexible reshaping** — Java's JEP 461
  *Stream::gather* is the right shape: a user-defined adapter that emits
  zero, one, or many values per input. `TODO(open): pick a better name than
  Java's `gather`; lean: `reshape` or `unfold_each`.`
- **Stream operations that resemble `jq`** — for stream-of-JSON-shaped
  data, the adapter set should feel as expressive as `jq`. This
  is more a *target* than a feature; ensure the data-format chapter exposes
  enough to make it true (see
  [`13-data-formats.md`](13-data-formats.md)).

`TODO(open): the relationship between streams, lazy iterators, and the
vectorized collections is not worked out — three overlapping ways to express
"a pipeline of work" would violate "one good way". Unify or delineate them.
One concrete option: make a stream the *only* iterator type
and drop the separation (mirroring Rust's "iterator is the async version of
itself" suggestion).`

## Borrowed-element iterators

An iterator may yield owned values or borrowed views into the source. A
borrowed iterator cannot expose two live elements at once — so `pairs()`
and `windows(n)` only work when elements are owned (or cheaply copied), or
when the borrowing rules permit overlap. The library exposes both forms;
the type makes the choice explicit. `TODO(open): names and ergonomics of
the owned / borrowed split depend on the unresolved mutability/ownership
model — see
[`../02-philosophy/04-antifeatures.md`](../02-philosophy/04-antifeatures.md).`

## Generators

Tel iteration interoperates with generators (`yield`-based producers). Their
full design — eager vs lazy start, whether they may `return` a value, whether
inputs can be sent in, whether linear-typed generators must be depleted —
belongs to the language chapter on generators, not to `std`. `std` only
provides the iteration adapters that consume them, and the small
`SimpleGenerator` (yields until depleted) / `Generator[Input]` (takes inputs,
returns a final value) split.

```tel
# Sketch — syntax not pinned down.
gen fn fibs() -> Iter[Int64] {
    let (uniq a, uniq b) = (0, 1)
    loop {
        yield a
        (a, b) = (b, a + b)
    }
}

let first_ten = fibs().take(10).to_list()
```

`TODO(open): generator design is open. Decide eager-vs-lazy start, the
re-entry contract after a final `return`, and whether `SimpleGenerator` and
the input-taking `Generator` are two types or one.`

## Parallel streams

A stream that holds only immutable values and a pure adapter chain can run
in parallel without user-visible change. The library exposes a
`.parallel()` adapter (final name TBD) that opts in: the host's task
runtime decides how the work is sharded.

Parallel iteration is **opt-in**, never on by default — quiet parallelism
would surface in observable side-effect ordering. The same restriction
applies as everywhere else: a stream that captures the host clock, RNG, or
a logger keeps reproducible results only because those are themselves
host-granted capabilities. `TODO(open): how the parallel scheduler
interacts with the task model — see
[`12-concurrency-utilities.md`](12-concurrency-utilities.md). TODO(open):
re-justify against embedding — heavy data-parallel work may belong in the
host behind a capability.`

## Lazy wrappers: prefer `Lazy[Map]` over `LazyMap`

A specific catalogue hazard worth stating explicitly: nesting
collection-wrapping lazy adapters can produce a value whose *type* implements
the collection's interface but whose internals are an unbounded chain of
delegating wrappers. The catalogue records repeated cases:

- A "lazy view of a map" was wrapped in another lazy view, then another, on
  every iteration; a stack overflow eventually surfaced from a deeply
  nested method call chain.
- A `flatMap` over already-lazy iterables stacked wrappers per call until
  the operations slowed proportional to the depth.

Tel's stance: a `Map`-typed value should always *be* a real map; laziness
belongs on the *outside* as `Lazy[Map[K, V]]`, not as a hidden trait
implementation. The stdlib lazy adapters return ordinary iterators that
materialise once consumed; collection-typed adapters that look like maps or
lists are always concrete.

TODO(open): confirm there is no `LazyMap[K, V]: Map[K, V]` in the standard
collection surface — laziness composes through `Lazy[T]` only, never through
a hidden delegation chain.

## Bugs the iteration design prevents

A handful of concrete catalogue cases:

- **"`filter` was used when `keep` was meant" (or vice versa).** The
  name `filter` reads ambiguously; the `keep` / `discard` rename above is
  the structural response.
- **"`add_all(some)` where `some` might be `None`."** A guard around an
  iterable that may be absent is awkward. The "any operation that
  consumes an iterable also accepts `Option[Iter[T]]`" rule (see
  [`04-core-collections.md`](04-core-collections.md)) is the response.
- **"Lazy stack overflow that took weeks to diagnose."** Already covered
  above — `Lazy[Map]` over `LazyMap`.
- **"Iterators that fail mid-stream and lose the rest of the failure."**
  The fallible-iterator pattern (above) shapes this: a `Result`-yielding
  iterator either short-circuits or collects, with the choice explicit.

## See also

- [Core Collections](04-core-collections.md)
- [Concurrency Utilities](12-concurrency-utilities.md)
- [Observability and Logging](14-observability-and-logging.md) — `inspect`
  is the standard hook for log-in-pipeline
