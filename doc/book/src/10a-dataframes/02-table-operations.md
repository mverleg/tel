# Table Operations

This chapter is the user-facing method surface of a [dataframe](01-overview.md),
built on the [record-shape calculus](../05-types/15-record-shape-calculus.md). Operations
divide cleanly: those that keep the row type (an ordinary generic library) and
those that change it (the calculus). Three of the schema-changers — `extend`,
`agg`, `pivot` — share one **column-expression** sublanguage; a fourth, `summary`,
is the field-generic `mapfields` reduce.

`<!-- TODO: review -->`

## Row-preserving operations (the ordinary library)

These return `Table[R]` unchanged in type — only the row *count* moves — so they
need no compiler support:

```tel
sales.filter(qty > 100)        # Table[Sale]
     .sort(by = price)         # Table[Sale]
     .distinct()               # Table[Sale]
     .head(10)                 # Table[Sale]
```

Also here: `tail`/`sample`, `drop_nulls`, `fill_null`, and rolling/window. One
edge case from [core-collections](../17-standard-library/04-core-collections.md):
filtering on a column known **unique** yields `Option[R]` rather than a table.

## The column-expression sublanguage

`extend`, `agg`, and `pivot` all take **column-expressions**: the bound names are
the **columns of `R`, each at its own type**, and an expression is built per
column and is **elementwise by default** (e.g. `price * qty` lifts `*` over two
columns), with reducers (`.sum`, `.mean`, `count()`) collapsing a column to a
scalar. Because these operations are compiler-recognised, the column names are
simply in scope and fully typed against `R` — no stringly column handle, no
`row => row.price` boilerplate.

The three differ in **one law** — the length each expression's result must have:

| context | each binding's result must be | so it contributes |
| --- | --- | --- |
| `extend` | a **column** (length `N`) | a new/replaced column |
| `agg` (per group) | a **scalar** (length 1) — a reducer must appear | one aggregate field |
| `pivot` value | a **scalar** per `(index, on)` cell | one cell of a target column |

`price * qty` is legal in `extend`; the *same* expression in `agg` is a type error
until a reducer collapses it (`(price * qty).sum`).

`TODO(open): the spelling of a column reference when a bare name is ambiguous (a
column named like a local, or a computed name) — reserve a fallback handle
(`col.price` / `$price`) while keeping bare names the default.`

## Add columns — `extend`

```tel
sales.extend(
    total = price * qty,     # new column, type EurAmt
    net   = total * 0.9,     # may read `total` (see threading)
    qty   = qty as Float64,  # name collision ⇒ replace-in-place, type updated
)
# : Table[{ item, price, total: EurAmt, net: EurAmt, qty: Float64 }]
```

Each `name = expr` is one [`extend`](../05-types/15-record-shape-calculus.md#the-five-primitives)
of the row type — a new field if fresh, a replace (with the new type) if it
collides; on a replace the right-hand side reads the *old* column. Multi-binding
`extend` is exactly repeated `extend`.

**Threading is sequential** (dplyr `mutate` semantics): bindings evaluate
left-to-right and each sees the columns added before it (`net` sees `total`). This
reads top-to-bottom and is the fold "extend each binding in turn".

`TODO(open): confirm sequential over Polars-style parallel (every RHS sees only
the original `R`); parallel is friendlier to the vectorised engine, so an explicit
parallel variant may still be wanted.`

`select` / `drop` / `rename` round out the schema-shuffling family
(`project`-based); `cast` is `extend` replacing in place.

## summary — `describe`

`summary` is `mapfields` at a `Summarisable` trait whose result is an
**associated type**, so each column lands at its *own* summary type:

```tel
trait Summarisable {
    type Summary                              # associated — each impl picks it
    fn summarise(col: Column[Self]) -> Self::Summary
}

sales.summary
# : { item: Text::Summary, price: EurAmt::Summary, qty: UInt32::Summary }
sales.price.summary          # one column
# : EurAmt::Summary
```

The result type is the projection `T::Summary`, **not** a uniform `Summary[T]` —
which matters, because `Bool::Summary` (`count`, `n_true`, `n_false`) is genuinely
a different type from `Real64::Summary` (`mean`, `std`, `min`, `max`, quantiles).
An associated type lets each `impl` choose its own record, so the result is a
**record** (the table collapses) of honestly heterogeneous summaries — *not*
pandas' all-`Float` `describe()` rectangle. Every column type must implement
`Summarisable`; the stdlib impls fan stats out by capability (`count`/`null_count`
for all; `n_unique`/`mode` where `Eq`; `min`/`max`/`median` where `Ord`;
`mean`/`std`/quantiles for numerics).

`TODO(open): a secondary `summary_table` keeping only the common stats (count,
null_count, numeric-common) can be a real `Table[{ stat: Text, … }]` for
pretty-printing, at the cost of a fixed common stat set and float coercion — a
render helper, not the typed result.`

## Aggregate — `group_by` + `agg`

```tel
sales.group_by(by = [region, item])      # : Grouped[{ region, item }, Sale]
    .agg(
        revenue = (price * qty).sum,     # reducer ⇒ scalar : EurAmt
        n       = count(),               #                  : UInt64
        avg_p   = price.mean,            #                  : EurAmt
    )
# : Table[{ region, item, revenue: EurAmt, n: UInt64, avg_p: EurAmt }]
```

`group_by` is the user name for the [`partition`](../05-types/15-record-shape-calculus.md#the-five-primitives)
primitive; `by` projects the key fields (a single key may drop the brackets). The
result row type is **key fields ++ aggregate fields**, flat — each aggregate's
name is its binding name and its type is the reducer's result type. `agg` is the
column-expression scope under the length-1 contract: every binding must reduce to
a scalar, and `count()` is the one reducer with no column argument.

A **keyless aggregate** `sales.agg(total = revenue.sum)` (no `group_by`) is a
whole-table reduce; it returns a **record** (one group, collapse), consistent with
`summary`.

`TODO(open): confirm keyless `agg` returns a record vs a one-row Table.`

`TODO(open): how much of `Grouped[K, R]` is user-visible beyond `.agg`.`

## pivot

`pivot` requires its target column **names as arguments**, which keeps it fully
static (see the [signature](../05-types/15-record-shape-calculus.md#how-a-result-type-is-computed)):

```tel
sales.pivot(
    index     = [date],          # surviving key columns (project)
    on        = region,          # column whose VALUES select a target
    using     = revenue,         # the value column fed to `agg`
    agg       = sum,             # pivot implies aggregation over each cell
    into      = [north, south, east, west],   # static target names ⇒ new fields
    unmatched = error,           # policy: error | drop | other
)
# : Table[{ date, north: EurAmt, south: EurAmt, east: EurAmt, west: EurAmt }]
```

The result row type is `index fields ++ one field per name in into`, each typed as
`agg`'s result over `using`'s type. The `on` column is matched at runtime against
the static `into` names, so `into`'s entries are *values* of `on`'s element type.

**If `on` is a closed [union](../10-data-modelling/02-union-types.md) whose
variants are exactly `into`, "unmatched" is statically impossible** — the compiler
checks exhaustiveness and `unmatched` may be omitted. It is required only when `on`
is open (e.g. `Text`). `agg` is mandatory because a cell can hold many rows
(unless `(index, on)` is statically unique).

`TODO(open): `unmatched`'s default (lean `error` — no silent loss) and whether the
catch-all `other` is a scalar aggregate or a nested `Table`.`

## Joins — `merge`

A join is the [`merge`](../05-types/15-record-shape-calculus.md#the-five-primitives) primitive:
the result row is the field union, the key counted once. The carrier supplies the
semantics — a table matches rows by key, and `left`/`right`/`outer` make the other
side's columns optional:

```tel
type Cost = { item: Text, unit_cost: EurAmt }

sales.merge(costs, on = item)                # inner : Table[{ item, price, qty, unit_cost }]
sales.merge(costs, on = item, how = left)    # left  : Table[{ item, price, qty, unit_cost? }]
```

Uniqueness on the join key propagates: joining on a key unique in the right table
introduces no duplicates, and the type system knows it.

## melt / unpivot

`melt` is `mapfields` plus a [union](../10-data-modelling/02-union-types.md) of the
melted columns' types — the inverse of `pivot`. It is an ordinary library
composition over the primitives and needs no special call shape beyond naming the
id columns and the columns to melt.

## Summary of the schema-changers

| op | primitive(s) | scope | returns |
| --- | --- | --- | --- |
| `extend` (add column) | `extend` (folded) | column-expr, length `N` | `Table[R']` |
| `summary` | `mapfields` reduce | field-generic | a **record** of `T::Summary` |
| `group_by` + `agg` | `partition` + `mapfields` | column-expr, length 1 | `Table[keys ++ aggs]` |
| `pivot` | `project` + repeated `extend` | column-expr, length 1 | `Table[index ++ into]` |
| `merge` (join) | `merge` | — | `Table[ρ₁ ⊕ (ρ₂ ∖ K)]` |
