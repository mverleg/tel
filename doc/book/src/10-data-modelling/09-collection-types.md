# Collection Types

The standard, ready-made containers for everyday data: variable-length
sequences, key-value lookups, sets, and a few specialised siblings. The
*language* features that make these types possible — generics, traits,
invariance — are in [`../05-types/07-generics.md`](../05-types/07-generics.md)
and [`03-traits-or-interfaces.md`](03-traits-or-interfaces.md); this page is
about the collections themselves and how Tel exposes them.

## The core collections

The shapes a Tel script reaches for first:

- **`List[T]`** — an ordered, variable-length sequence of `T`. The default
  "many things in order". Use it where you would use Python's `list`, Java's
  `ArrayList`, or Rust's `Vec`.
- **`Map[K, V]`** — a key-value lookup, keys are unique. The default "by-name"
  container. Requires `K: Eq + Hash` (see
  [`07-equality-and-hashing.md`](07-equality-and-hashing.md)).
- **`Set[T]`** — a collection of unique elements. Requires `T: Eq + Hash`.
- **`Array[T, n]`** — a fixed-length sequence of `T`. Length is part of the
  type — see [`../05-types/04-tuples-and-arrays.md`](../05-types/04-tuples-and-arrays.md).
- **Ordered counterparts** — `OrderedMap[K, V]` / `OrderedSet[T]` that
  iterate in insertion (or sorted) order. These are *free of the `random`
  effect* at iteration time (see
  [`07-equality-and-hashing.md`](07-equality-and-hashing.md)).
- **`SortedMap[K, V]`** / **`SortedSet[T]`** — kept in key order; require
  `K: Ord`.

This list is deliberately short. Tel's stance, per
[the maxims](../02-philosophy/02-maxims.md), is *non-overlapping, composable,
consistent* — a single obvious container per shape, rather than a long menu
of nearly-identical variants.

TODO(open): exact names — `List` vs `Vec` vs `Seq`, `Map` vs `Dict`, `Set` vs
`HashSet`. Lean: `List`, `Map`, `Set` (Kotlin/Python flavour), with
`OrderedMap`/`SortedMap` for the ordered cases.

## Values, not references

Collections are **values** like everything else in Tel: immutable by default,
compared by content (see [`07-equality-and-hashing.md`](07-equality-and-hashing.md)),
hashable when their elements are. Two `List[Int64]` with the same elements are
equal; one is not "the original" and the other "the copy".

The mutable counterpart is the `!T` form of the same type — see
[Mutable builders](#mutable-builders) below. This split is the same one
applied to text (`Text` vs `!Text`, see
[`../05-types/03-strings-and-text.md`](../05-types/03-strings-and-text.md)):
mutability is a property of the *type*, spelled with the `!` sigil, plus a
`uniq` binding to hold it (see
[mutability](../06-bindings-and-scope/02-mutability.md)).

## Constructing collections

Two of the core collections — `List` and `Map` — have **literal syntax**;
everything else is built with ordinary functions. The guiding rule is *one way
to do things*: no per-type magic constructor that the compiler has to know
about.

### List and map literals — `[ … ]`

Both share the bracket literal, distinguished by their contents (the lexical
details and the lookahead argument are in
[`../03-lexical-structure/05-literals.md`](../03-lexical-structure/05-literals.md#collection-literals----for-lists-and-maps)):

```tel
let xs = [1, 2, 3]                          # List[Int64]
let tr = ["en" => "hi", "fr" => "salut"]    # Map[Text, Text]
let by_id = [u.id => u, v.id => v]          # keys are expressions, not just literals
```

- A bare sequence of expressions is a **list**; `key => value` entries make it a
  **map**.
- Both are **immutable** — consistent with [values, not
  references](#values-not-references). Build mutably with `!List` /
  `!Map` (see [Mutable builders](#mutable-builders)).
- Keys and values are arbitrary expressions; keys need not be literals.

### Empty collections

`[]` is the **empty list**, unconditionally — Tel does not use the expected type
to reinterpret it as an empty map. The empty map is `map_of()`, which falls out
of the pair-based form below rather than needing a separate `empty_map()`.

### Other collections — plain functions

Sets and any further containers are built from functions, not new bracket
shapes:

```tel
let s  = set_of(1, 2, 3)               # Set[Int64] from listed elements
let e  = set_of()                      # empty Set
let s2 = set([1, 2, 3])                # Set[Int64] from an existing list (dedupes)

let m  = map_of([k1, v1], [k2, v2])    # Map from [key, value] pairs
let e2 = map_of()                      # empty Map
```

`map_of` takes **`[key, value]` pairs**, not a flat alternating argument list.
A flat `map_of(k1, v1, k2, v2)` cannot be typed — `k` and `v` generally have
different types, so a single vararg would have to collapse them to a common
type. Grouping each key with its value keeps every pair well-typed *and* makes
`map_of()` the natural empty map, which is why the pair form is preferred over a
dedicated empty-map constructor.

TODO(open): whether the pair is a 2-list `[k, v]` or a tuple `(k, v)`. A tuple
types more tightly (`(K, V)` instead of `List[(K | V)]`); the `[k, v]` spelling is
what the literal syntax makes cheapest. Lean toward whichever the tuple-vs-list
ergonomics settle on elsewhere.

## Invariance — collection element type does not vary

`List[Cat]` is *not* a `List[Animal]` even though `Cat` is usable where
`Animal` is. This is invariance, the default for every generic parameter (see
[`../05-types/09-subtyping-and-variance.md`](../05-types/09-subtyping-and-variance.md)).
The practical consequence for collections: a function meant to accept either
takes a *union*-typed list explicitly, or a generic function with the
appropriate bound.

```tel
fn count_animals(items: List[Cat | Dog]) -> Int64 { items.len() }
fn count_anything[T](items: List[T]) -> Int64 { items.len() }
```

## A shared shape — the iteration trait

A `List`, an `Array`, a `Map`'s values, a `Set` — all of these can be
*iterated*. Tel exposes the shared shape as a trait (working name
**`Iterable`**, see [`10-iterators-and-sequences.md`](10-iterators-and-sequences.md))
so generic code can take any of them, and so user-defined containers and
lazy generators interoperate with the same map/filter/reduce machinery.

This is a long-standing pain point in some languages:
mixing finite lists with infinite generators. Tel's working answer is that
`List`, `Array`, and a hypothetical generator all implement the same
iteration trait, so `take(3, ...)` works on any of them — see
[`10-iterators-and-sequences.md`](10-iterators-and-sequences.md).

## Mutable builders

Some operations are quadratic on immutable values (building a `List` element
by element via copy-update is `O(n²)`). Tel's answer is the *owned form* of
the type: `!List` is the builder for `List`, an *owned* (affine, mutable in
place) type whose scope is small and local, which produces a shareable
collection when done.

```tel
let uniq builder = !List[Int64]()
for x in inputs {
    if x > 0 { builder.push(x) }
}
let result: List[Int64] = builder.finish()
```

The owned forms are `!List`, `!Map`, `!Set`, each yielding the shareable
counterpart `List` / `Map` / `Set` on `finish()`. This is the general `!T`
**ownership** sigil ([TIP-0001](../tips/0001-mutability-and-borrowing.md)), the
same split text uses (`!Text` / `Text`) — `!T` is affine (owned, mutable in
place), bare `T` is shareable; ownership is a property of the type, not a
modifier on a binding. (The earlier `ListBuilder`/`MapBuilder` naming convention
is retired in favour of `!List`/`!Map`.)

Two important consequences:

- **Immutable collections are freely shareable.** Two scopes holding a
  `List[Int64]` cannot interfere with each other; this is a hard guarantee, not
  a convention.
- **A shareable collection only holds values of shareable (`Alias`) types.** An
  affine `!T` element would make the whole container affine, so it cannot sit in
  a shared one — the data-race-safety intent: a shareable container statically
  guarantees no element is reachable affinely through it. (Elements need not be
  *immutable* — a synchronised `Sync` type such as `Mutex` is shareable too; see
  [substructural types](../12-memory-and-runtime/08-substructural-types.md).)

The broader mutability model is settled
([TIP-0001](../tips/0001-mutability-and-borrowing.md)): Tel has both the
type-level `!T` form (the builder here) *and* explicit `uniq` bindings, and they
compose — `let uniq builder = !List[Int64]()`. See
[mutability](../06-bindings-and-scope/02-mutability.md).

TODO(open): a third option is *snapshotting* (an O(1) immutable snapshot
of a mutable collection, like a `ctrie`), sitting between
builder-and-finish and copy-update. Useful but specialised; lean: not in the
core language, available as a library trait if a collection wants to opt in.

## Beyond the core — specialised containers

A range of specialised containers come up. Tel's instinct,
per [`one good way`](../02-philosophy/02-maxims.md), is to keep the core
small and let the standard library cover specialised cases without making
the language bigger.

### Unrolled lists

A list stored as a chain of contiguous blocks (between a linked list and an
array list). Better cache behaviour than a linked list, cheap inserts in the
middle, no value-moving on grow. A worthwhile optimisation, but a
library-level container, not a language-level one.

### Tables (relation-like collections)

A *table* is essentially "a Pandas DataFrame without an index" — a row type
plus efficient column-wise storage. This is a compelling direction, with
a long list of capabilities: projections, joins, row-vs-column access,
filtered views without copy, unique-column types that return `Option` instead
of a collection on lookup, indices on combinations of columns.

Tel's stance: tables are the kind of thing the language enables but does *not*
build in. They want generics, refined types, and ideally const generics — all
features already on the roadmap. The big question — "build
tables into the language (easy) or add the compiler magic so libraries can
make tables (hard, cool)?" — the embedding priority answers: *neither, build
them as a library on top of language features that are already general*. A
specialised "Tables" construct does not pay its weight when a host already
provides its own data-table story.

TODO(open): adopt or reject `Table[Row]` as a *standard-library* type. Lean:
yes, library-level; the compiler does not need a special case if generics
and refined types are strong enough.

### Transposed / struct-of-arrays storage

Should `List[(Real64, Real64)]` sometimes be stored as
`(List[Real64], List[Real64])` for SIMD / locality? This is an
*implementation-detail* concern, like string representation (see
[`../05-types/03-strings-and-text.md`](../05-types/03-strings-and-text.md)):
do not let it leak into the type surface, record in `impl-notes/` only if
worth pursuing.

### Linked, deque, ring-buffer, etc.

Each has its place; each lives in the standard library, not the language.
The traits in [`10-iterators-and-sequences.md`](10-iterators-and-sequences.md)
are what makes them interoperable.

## Collections and refined types — properties the type system tracks

Several useful properties of a collection are naturally expressed as refined
types over the standard ones (see
[`../05-types/12-refined-types.md`](../05-types/12-refined-types.md)):

- **`NonEmpty[List[T]]`** — a list with at least one element. `first()`
  returns `T`, not `Option[T]`; `head` / `tail` always succeed. This is a
  sharp use case for refined types.
- **`Sorted[List[T]]`** — a list known to be in order. `binary_search` runs
  on it without re-sorting. See
  [`08-ordering.md`](08-ordering.md).
- **`Unique[List[T]]`** — a list known to have no duplicates. Useful as the
  key column of a table-like join.
- **Indexed access bounds** — a `BoundedIndex[n]` into an array of length `n`
  elides the bounds check. The deeper version of this is *branded* types —
  see [`../05-types/12-refined-types.md`](../05-types/12-refined-types.md).

TODO(open): which of these refined-collection types ship with the standard
library at 1.0. Lean: `NonEmpty` for sure (used by most container APIs as a
return type); `Sorted` and `Unique` if the constraint-propagation story makes
them low-friction.

## Collections of equal shape — co-indexed columns

One pattern *is* worth surfacing in the type system: two
maps over the same key set, or two lists of the same length co-indexed by
position. The straightforward Tel encoding is "use a single map / list of
tuples" — `Map[K, (A, B)]` instead of `(Map[K, A], Map[K, B])` — which keeps
the relationship in the type. A more expressive encoding (a
type-level "these two maps have the same keys") is tempting, but the simple
encoding covers the common case at zero language cost.

TODO(open): whether there is a better encoding than
`Map[K, (A, B)]` for "two maps with the same keys" or "this map's keys are a
subset of that map's". This points toward dependent-types territory; lean
"no, stick with the simple tuple encoding", flag if a real use case demands
more.

## Collection methods, shared and per-type

A `List` exposes the standard `map` / `filter` / `reduce` / `fold` family.
The same names — on the iteration trait — work on anything iterable, so
`Set.map` and `Map.values().map` are the same call shape. This is the
*consistent surface* commitment from
[the maxims](../02-philosophy/02-maxims.md).

Tel adopts the convention that generic methods
that *might* fail (`first` on a list) return `Option`, with a separate
*loud-failing* accessor for the "I know it is there" case. There is no
silent nullable.

```tel
items.first()           # Option[T]
items.first_or_abort("expected at least one item")   # T, aborts on empty
```

TODO(open): final naming convention for the loud-failing accessors. The
[`11-conversions-and-coercions.md`](../05-types/11-conversions-and-coercions.md)
page has the same open question; resolve consistently across types.

## See also

- [Iterators and Sequences](10-iterators-and-sequences.md) — the shared
  iteration trait and lazy generators.
- [Tuples and Arrays](../05-types/04-tuples-and-arrays.md) — fixed-length
  cousins.
- [Equality and Hashing](07-equality-and-hashing.md) — the `K: Eq + Hash`
  bound, the `random` effect.
- [Ordering](08-ordering.md) — `SortedMap`, comparators.
- [Refined Types](../05-types/12-refined-types.md) — `NonEmpty`, `Sorted`,
  `Unique`, branded indices.

TODO: review
