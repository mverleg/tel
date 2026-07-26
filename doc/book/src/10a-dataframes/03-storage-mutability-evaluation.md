# Storage, Mutability, and Evaluation

How a [dataframe](01-overview.md) is laid out in memory, how it is mutated, and
when it computes. None of this changes the row type story of the
[calculus](../05-types/15-record-shape-calculus.md) — it is the runtime substrate underneath.

`<!-- TODO: review -->`

## Storage: columnar

A `Table` stores data **column-major — a struct of equal-length arrays (a list of
columns)**, not row-major. Reasons:

- It is the layout the schema-transforming primitives want: `project`, `extend`,
  `cast`, and `mapfields` add, drop, or rewrite *whole columns*, which is cheap
  column-major and expensive row-major.
- It enables SIMD / vectorised per-column kernels (the
  [vectorised/transposed substrate](../17-standard-library/04-core-collections.md#vectorized--transposed-collections)),
  matching how analytical workloads process data.
- It makes a columnar-engine / SQL pushdown natural (see
  [the query carrier](#the-query-carrier-direction)).

The **row view** — iterating the table as a sequence of `R` records
(array-of-structs) — is offered as a *view* over the columnar storage. It needs
**no compiler support**: a plain `List[R]` of an ordinary record already works.
So the magic is spent specifically on the columnar `Table`; the row-sequence form
is the trivial case.

## Mutability — one type, two axes

There are **not** two dataframe types. Mutability falls out of Tel's ordinary
[ownership and mutability](../06-bindings-and-scope/02-mutability.md) axes, and the
schema-transform story decides most of it:

- **Shape transforms are value-returning, never in-place.** `project` / `extend` /
  `merge` / `mapfields` / `partition` change the *static* row type
  (`Table[{a,b}]` → `Table[{a,b,c}]`), and a value cannot mutate into a different
  type. So the whole schema-changing surface is functional by construction — the
  same reason Polars and dplyr are immutable, without pandas' in-place mess.
- **Value-returning does not mean copying.** When the input is `uniq` / affine the
  columnar buffers are **moved / reused (copy-on-write)**, so `t2 = t.extend(...)`
  is cheap. Returning a new value and reusing storage are independent.
- **Mutation applies only within a fixed schema** — append/drop rows, update
  cells, fill nulls. That is the normal mutable axis, via a **`!Table` unique
  builder**, exactly the [`!List` builder](../17-standard-library/04-core-collections.md)
  pattern:

```tel
t2 = t.extend(total = price * qty)   # new typed value; buffers reused if t is uniq

uniq b : !Table[Sale] = Table.builder()
b.push({ item = "pen", price = eur(2), qty = 5 })
let table = b.finish()               # : Table[Sale]
```

So: one `Table`, transforms are functional (with COW), and the `!` / `uniq` axis
covers fixed-schema mutation — no frame-specific mutable/immutable split.

## Evaluation is eager

Polars' other headline feature is **lazy evaluation** — recording an operation
graph and optimising it (predicate/projection pushdown, reordering) before
running. Tel's `Table` **evaluates eagerly** and does **not** ship a lazy query
optimiser. Laziness is an *optimisation*, orthogonal to the static typing this
feature is about (the row type and primitives are identical either way), and a
query planner is a large subsystem that sits awkwardly with Tel's embedding goal —
the host, or a real database behind a capability, is the better place for heavy
planning.

Forgoing laziness costs some peak performance on large pipelines but keeps the
feature focused on the part only the compiler can provide — the static schema.

## The I/O boundary

The calculus has one **escape hatch**: a **dynamic-schema `Table`** whose schema is
a runtime value. It is needed *only* where a CSV/Parquet schema arrives at runtime
and must be reconciled with a static row type. No in-language operation produces a
dynamic-schema table — it is confined to the edge where data enters the program,
and is bridged to a static `Table[R]` by a checked conversion.

`TODO(open): reconcile with the serialisation data model
([TIP-0007](../tips/0007-serialisation-data-model-and-formats.md)) — how the
runtime schema of a freshly-read frame is matched against the static row type, and
what the conversion's failure mode is.`

## The query carrier (direction)

The same calculus has a natural third carrier (beyond *record* and *column-table*):
a **query carrier** that lowers `project`/`extend`/`merge`/`partition` to
relational algebra / SQL, with predicate and projection pushdown to a
capability-backed table or host database. This is the payoff of the
record-shape-calculus framing — the operations are already the relational core.

`TODO(open): whether the query carrier is a 1.0 carrier or later, and how far the
carrier abstraction is exposed (are `record`, `Table`, and `query` instances of a
named, compiler-known calculus the user can see, or is the sharing an
implementation/spec convenience with concrete blessed APIs?). Keep carriers
sealed either way.`

## What it costs

The dataframe needs **substantial compiler support**: deriving a fresh row type
from a column declaration, propagating it through the schema-changing primitives,
and keeping column access statically checked. That is more than a library on the
collection substrate — it is the compiler-recognised exception described in the
[calculus chapter](../05-types/15-record-shape-calculus.md#a-closed-calculus-not-open-type-level-programming).
The scope is bounded by the closed primitive set, by monomorphic-only reuse, and
by forgoing laziness — which is what keeps it tractable for 1.0.
