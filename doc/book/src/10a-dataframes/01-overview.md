# Dataframes — Overview

A **dataframe** (`Table[R]`) is Tel's blessed type for tabular, column-typed
data: many rows, each row a record of the **same** named, typed columns. It is
the type you reach for to load a CSV, join two datasets, group-and-aggregate, or
pivot — the pandas / Polars / dplyr surface, made statically typed.

```tel
type Sale = { item: Text, price: EurAmt, qty: UInt32 }

let sales : Table[Sale] = load_csv("sales.csv")   # N rows, 3 typed columns
let big   = sales.filter(qty > 100)               # still Table[Sale]
let withTotal = sales.extend(total = price * qty) # Table[{ item, price, qty, total: EurAmt }]
```

`<!-- TODO: review -->`

## What — a table is a record carried over many rows

The natural unit of a dataframe is a **row**: a value whose fields are the
columns, at the columns' types. That row type `R` is an ordinary **structural
record** — the anonymous, compared-by-shape form (see
[tuples](../05-types/04-tuples-and-arrays.md) and
[records](../10-data-modelling/01-records.md)), *not* a nominal `struct`. A
`Table[R]` is that one row shape stored over many rows.

Because the row type is real, swapping two columns, reading a column that does
not exist, or feeding a `Text` column to a numeric aggregate are **compile-time
type errors**, not runtime surprises. Column names are
[statically-checked labels](../05-types/12-refined-types.md), so `sales.price` is
a typed field access, not a stringly-typed lookup.

## Why it is not a matrix

A [matrix](../17-standard-library/07-numerics-and-math.md) and a dataframe look
alike (both rectangular) but differ in kind:

| | matrix | dataframe (`Table[R]`) |
| --- | --- | --- |
| element types | one, homogeneous | **one type per column**, heterogeneous |
| named axes | labels for indices | columns *are* a derived record type |
| operations | linear algebra | relational: `filter`, `join`, `group_by`, `pivot` |

A matrix stores a single element type in a fixed shape; "named axes" there are
just labels for integer indices. A dataframe has `price : EurAmt`, `name : Text`,
`count : UInt32` side by side — there is no single element type, so it is a
different feature with a different home. (The numerics chapter's matrix type
therefore has **no** heterogeneous "named axes" mode.)

## Why it needs the compiler

Most table operations are an ordinary generic library over `Table[R]` — they do
not change the row type, only the row *count*:

- **No schema change** — `filter`, `sort`, `distinct`, `head`/`tail`/`sample`,
  `drop_nulls`, `fill_null`, rolling/window. Row type in, same row type out.

A minority compute a **new** row type from `R`, and that is the part only the
compiler can do, because the result type depends on the *values* of the call
(which column names, which keys), not just its argument types:

- **Schema change** — `select`, `drop`, `rename`, `extend`/`with_columns`,
  `cast`, the `join` family, `group_by` + `agg`, `value_counts`/`crosstab`,
  `summary`/`describe`, `melt`/`unpivot`, `pivot`.

Tel handles the second group with a small, **compiler-recognised calculus over
record shapes** — five primitives that every schema-changing operation is built
from. That calculus is a type-system feature in its own right, defined in
[The Record-Shape Calculus](../05-types/15-record-shape-calculus.md) (under
Types); the dataframe operations built on it are in
[Table Operations](02-table-operations.md).

## The shape of the feature

- A row is forced to be a **scalar or a vector of the common length**, so the
  table is rectangular along the row axis even though columns differ in type.
  That uniform length is what lets the compiler treat the column set as one
  derived row type.
- The whole schema-changing surface is **value-returning** (functional, like
  Polars/dplyr, not pandas' in-place mutation); buffers are reused when the input
  is unique. In-place *row* mutation within a fixed schema uses a `!Table`
  builder. See [Storage, mutability and evaluation](03-storage-mutability-evaluation.md).
- Evaluation is **eager**; Tel does not ship a lazy query optimiser (same chapter).

## Scope for 1.0

- The dataframe is **in the 1.0 standard library**, built on the
  [core-collections](../17-standard-library/04-core-collections.md) columnar
  substrate.
- The calculus is granted in **monomorphic** position only: user code composes
  the operations at concrete schemas, but the language has **no row
  polymorphism** (no abstractly-typed combinators over an unknown schema). This
  is a deliberate, permanent scope line — see the
  [calculus chapter](../05-types/15-record-shape-calculus.md#reuse-is-monomorphic).

This chapter set supersedes the design sketch in
[TIP-0008](../tips/0008-named-axis-dataframes.md).
