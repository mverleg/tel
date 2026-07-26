# TIP-0008: Named-axis dataframes (pandas-style), not a matrix feature

**Status:** Accepted and **migrated** into the chapter docs (2026-06-19) — see the
[Dataframes](../10a-dataframes/01-overview.md) chapter
([calculus](../05-types/15-record-shape-calculus.md),
[operations](../10a-dataframes/02-table-operations.md),
[storage](../10a-dataframes/03-storage-mutability-evaluation.md)). Kept as the
historical record. Design is **monomorphic only**; row polymorphism is out of
scope (not a future TIP).
**Created:** 2026-06-14
**Touches:** `17-standard-library/07-numerics-and-math.md` (removes the
*named axes* matrix bullet), `17-standard-library/04-core-collections.md`
(the vectorized/transposed substrate a dataframe would build on),
`10-data-modelling/01-records.md` (the auto-derived per-row type),
`05-types/12-refined-types.md` (statically-checked column names).

## Summary

A "named axes — pandas-style named rows and columns" feature was sketched as a
**matrix** capability in
[`07-numerics-and-math.md`](../17-standard-library/07-numerics-and-math.md).
That is the wrong home: a matrix is homogeneous (one element type, a fixed
shape), whereas a pandas-style frame has **a different type per column** and
wants relational operations (`filter`, `map`, `groupby`, `pivot`). It is a
*dataframe*, not a matrix, and it needs substantial compiler support to be
type-safe. This TIP records the design — the blessed type, its primitives, and
its storage — and moves the idea out of the matrix chapter into its own home.

## Why it is not a matrix

A matrix stores one element type in a rectangular shape; every cell is the same
type, and the value of named axes there is just *labels for indices*. A
dataframe is different in kind:

- **Heterogeneous columns.** Column `price` is `EurAmt`, column `name` is
  `Text`, column `count` is `UInt32`. There is no single element type.
- **A row is a derived record.** The natural unit is a *row* whose fields are
  the columns, with the columns' types — an **automatically derived record
  type** the user never writes by hand. That row type is what makes "I swapped
  two columns" a type error instead of a runtime surprise.
- **Relational operations.** `filter`, `map`, `groupby`, `pivot`, joins — the
  pandas/Polars surface — not linear algebra.

## The shape this would take

- Each "row" is forced to be either a **scalar** or a **vector of the same
  length as the others**, so the frame is rectangular along the row axis even
  though columns differ in type. That uniform-length assumption is what lets the
  compiler treat the column set as a single derived **row type**.
- Column names are **statically known** where possible, so `frame.price` is a
  typed column access checked at compile time (a
  [refined-type](../05-types/12-refined-types.md)-flavoured guarantee), not a
  stringly-typed lookup.
- The operation set — `filter`, `map`, `groupby`, `pivot`, and friends — is
  expressed over that derived row type.

## Operations that need compiler support

Grouping the common dataframe operations (from the pandas / Polars / dplyr
survey) by what they do to the **row type** `R` — the auto-derived record —
shows that only a minority need the compiler. The rest are an ordinary generic
`Table[R]` library.

- **No type change — ordinary library.** `filter`, `sort`, `distinct`,
  `head`/`tail`/`sample`, `drop_nulls`, `fill_null`, rolling/window. Row type
  in, same row type out; only the row *count* changes. (Edge case already noted
  in [`core-collections`](../17-standard-library/04-core-collections.md):
  filtering a *unique* column yields `Option[R]`.)
- **Schema transform — needs compiler support.** `select`, `drop`, `rename`,
  `mutate`/`with_columns`, `cast`, the join family, `group_by`+`agg`,
  `value_counts`/`crosstab`, `describe`, `melt`/`unpivot`, `pivot`. Each
  computes a *new* row type from `R` — the magic a library cannot express,
  because the result type depends on the *values* (column names, key sets) of
  the call, not just its argument types.

[`core-collections`](../17-standard-library/04-core-collections.md) already
sketches this substrate (two equivalent layouts, joins as "functions on
types"); this TIP pins down the full primitive set those operations need.

## The minimal blessed primitives

Rather than bless each operation, bless a **small closed set of type-level
primitives** and define every schema-changing operation as a composition of
them. The key insight: these primitives are not really about *dataframes* — they
are about **record shapes**. A `Table[R]`'s schema *is* the record type `R`, so
the same five operations describe transforming a single record and transforming
a table's columns. They are a **record-shape calculus** parameterised over a
**carrier**:

- **record carrier** — one value of type `R` (an ordinary struct).
- **column-table carrier** — `Table[R]`, the columnar dataframe.
- **query carrier** — a target that lowers to relational algebra / SQL
  (`TODO(open):` direction, see below).

The compiler recognises the five on each carrier and computes the result type;
everything else in the operations list is library code written over them.

1. **`project`** — keep a subset of fields.
   Type: `{a,b,c} -> {a,c}`.
2. **`extend`** — add *or replace* one field; its type is the defining
   expression's type. Type: `{a,b}, d := expr -> {a,b,d}`.
3. **`merge`** — combine two record shapes on a key (field union, key shared
   once). Type: `{id,x}, {id,y} -> {id,x,y}`.
4. **`mapfields`** — apply one field-generic op across *every* field, building a
   new shape field-by-field. The only primitive generic over an unknown field
   set.
5. **`partition`** — the one *collection-only* primitive: split a carrier of
   many rows into a `Grouped[K, R]` keyed by some fields. Meaningless on a single
   record (nothing to group), so it exists only on the table/query carriers. An
   aggregation is `mapfields` applied per group, collapsing back to a `Table`.

The first four are pure shape transforms and so apply to **any** carrier; only
`partition` needs a collection. The next section works each one on both a record
and a table to show they are literally the same operation.

Plus one escape hatch:

6. **dynamic-schema `Table`** — a table whose schema is a runtime value, needed
   *only* at the I/O boundary where a CSV/Parquet schema arrives at runtime. No
   in-language operation produces one (see `pivot` below), so this hatch stays
   confined to the edge where data enters the program.

Every operation that needs the compiler composes from these — none below is its
own primitive:

| Operation | Built from |
| --- | --- |
| `select` | `project` |
| `drop` | `project` (the complement) |
| `rename` | `project` + `extend` |
| `mutate` / `with_columns` | `extend` |
| `cast` | `extend` (replace in place) |
| inner / left / right / outer `join` | `merge` |
| `group_by` + `agg` | `partition` + `mapfields` |
| `value_counts` / `crosstab` | `partition` + count (`mapfields`) |
| `describe` | `mapfields` (aggregate each column) |
| `melt` / `unpivot` | `mapfields` + a union of the melted columns' types (uses [TIP-0002](0002-untagged-unions-and-sealed-traits.md) unions) |
| `pivot` | static target-column-name args + repeated `extend`; unmatched values handled by an explicit policy |

So the irreducible magic is **`project`, `extend`, `merge`, `mapfields`,
`partition`**. The **dynamic-schema fallback** is then needed *only* at the I/O
boundary (a CSV/Parquet schema arriving at runtime), not for any in-language
operation.

`pivot` is the operation that forced this question, because in pandas/Polars its
new column *names* are discovered from the *data*. Tel makes it static instead
by **requiring the target column names as arguments** rather than inferring them
— `pivot` then becomes ordinary repeated `extend` onto a known schema. Values
that do not map to one of the named columns are handled by an **explicit
policy** (error, drop, or collect into a catch-all `other` column), never by
silently growing the schema. This keeps `pivot` fully static and removes its
need for the dynamic-schema fallback.

Row `concat`/`union` is deliberately *not* in this list: it is not a transform
but a *constraint* that two row types match — an ordinary type equality the
library checks, needing no new primitive.

### Five operations, but only three type-level operators

The "five primitives" are a *value/runtime* taxonomy. At the **type** level a
`Table[ρ]` is just its row `ρ` — row counts never appear in a type — so a type
transform only describes how `ρ` changes, and the five collapse to **three**
operators over the [row calculus](#a-schema-is-a-row-the-primitives-are-functions-on-rows),
plus a constructor for the base case:

- **insert labels** — add-or-extend the row with new labels. The right operand
  may be a **literal** `{ℓ:τ}` (that is `extend`) *or* **another row** `ρ₂` (the
  union behind `merge`).
- **delete labels** — drop labels. The operand may be a **literal** label set
  (that is `project`'s complement / `drop`) *or* **another row's** labels (the
  asymmetric difference `ρ₂ ∖ K` behind `merge`).
- **field-map** — the `mapfields` per-field rewrite `{ℓ : (ρ.ℓ)::Out}`: the one
  operator generic over an *unknown* field set.

**`⊕` is not a primitive — it is delete-then-insert.** The earlier draft listed an
"override-insert `⊕`"; replace-in-place (`qty = qty as Float64`) is just *delete
the colliding label, then insert the new one*. `merge on K` is likewise
`ρ₁ ∪ (ρ₂ ∖ K)` — difference the right by the keys, then union — so `merge` and
`partition` add **no** new type operator (`partition` is `delete` to the key
labels plus an ordinary `Grouped[…]` constructor). The literal-operand and
row-operand forms are the *same* two edits, so the honest split is **insert /
delete** over the label domain.

**field-map is *not* delete + insert.** Tempting, but the part that matters is the
type: `mapfields` inserts each label back at `(ρ.ℓ)::Out` — a type-level *function
of the label's old type*, applied uniformly across a possibly-unknown field set.
insert/delete only move labels with **literally-given** types; they never compute
a new type from an old one. The two axes are orthogonal: **insert/delete change
the label *domain* (with literal types); field-map fixes the domain and rewrites
each *type* by a function.** `cast`/replace looks like a type change but is
delete+insert with a *literal* target (`as Float64`), so it stays in the first
axis. field-map is the only operator in the second.

**The base case: a constructor.** All three transforms have shape `ρ → ρ'` — they
presuppose an input row. The introduction form is the **empty row `{}`**, with any
literal `{a:A, b:B}` being `{}` then two inserts. This costs nothing new: it is
just Tel's ordinary **record-type literal**, reused. Its genuinely-magic runtime
sibling is the **dynamic-schema `Table`** (escape hatch #6), where labels arrive
from a CSV/Parquet header at runtime — the one constructor confined to the I/O
boundary.

Because the constructor and transforms only ever build **structural** rows
(records compared by shape — named fields, but shape identity), a derived schema
is structural by construction. A **nominal** frame (distinct identity, own
methods) is not produced by the calculus — nominal identity is a freshly *declared*
identity, not derivable from shape — so it is capped explicitly with `newtype` at
the boundary (see [*Naming and reuse of derived schemas*](#naming-and-reuse-of-derived-schemas)).

So the many-to-many row blow-up that makes `merge` *value*-irreducible (a general
inner join can grow the row count beyond either input — see
[the row-count argument](#how-the-result-type-is-computed-the-row-calculus)) is
**invisible at the type level**, because the type does not count rows. The five
operations still exist to name distinct *runtime* behaviors (row-count change,
which kernel — a join is not an `extend` at runtime even though its type is); the
type system deliberately cannot see those. So the compiler magic an implementer
must build is **three row-type operators (insert, delete, field-map) over a
structural-record constructor — not five primitives**.

## Records and tables: one calculus, two carriers

The same five operations, worked on a single record and on a table, to show the
*type-level* transform is identical and only the carrier differs. Syntax is
loose — Tel's is not pinned down.

A row shape is an ordinary record type; a table is that shape carried over many
rows:

```tel
type Sale = { item: Text, price: EurAmt, qty: UInt32 }

one  : Sale         = { item = "pen", price = eur(2), qty = 5 }
many : Table[Sale]  = load_csv("sales.csv")   # columnar: 3 columns, N rows
```

**`project`** — same type function `{item, price, qty} -> {item, price}`:

```tel
one.project[item, price]    # : { item: Text, price: EurAmt }      narrows one struct
many.project[item, price]   # : Table[{ item: Text, price: EurAmt }]   drops a column
```

**`extend`** — add or replace a field; result type adds `total`. On a record the
expression runs once; on a table it runs per row (vectorised over the column):

```tel
one.extend(total = price * qty)    # : { item, price, qty, total: EurAmt }
many.extend(total = price * qty)   # : Table[{ item, price, qty, total: EurAmt }]

one.extend(qty = qty as Float)     # replace-in-place is a cast: qty now Float
```

**`merge`** — the field union is the same both ways; the *carrier* supplies the
semantics. A record just combines; a table matches rows by key, and `left`/etc.
make the other side `?`:

```tel
type Cost = { item: Text, unit_cost: EurAmt }

one.merge(costs_one, on = item)            # : { item, price, qty, unit_cost }   structural combine
many.merge(costs_tab, on = item)           # : Table[{ item, price, qty, unit_cost }]   inner join
many.merge(costs_tab, on = item, how = left)
                                           # : Table[{ item, price, qty, unit_cost? }]   left join
```

**`mapfields`** — the one field-generic primitive. If the per-field op is a
*transform* the shape is preserved (a cast-all); if it *reduces* a column to a
scalar, a table collapses to a single record (that is `describe`):

```tel
one.mapfields(f => f.to_text)
#   : { item: Text, price: Text, qty: Text }
many.mapfields(c => c.to_text)
#   : Table[{ item: Text, price: Text, qty: Text }]       transform → same carrier

many.mapfields(c => c.summary)
#   : { item: Text::Summary, price: EurAmt::Summary, qty: UInt32::Summary }   reduce → one record
```

**`partition`** — collection-only; there is nothing to group in one record:

```tel
many.partition(by = item)
#   : Grouped[{ item }, Sale]
many.partition(by = item).agg(revenue = (price * qty).sum)
#   : Table[{ item, revenue: EurAmt }]      key fields + agg fields
```

Summarised — the same op across carriers:

| op | record | `Table` | SQL (query carrier) |
| --- | --- | --- | --- |
| `project` | narrow a struct | drop columns | `SELECT cols` |
| `extend` | add/replace a field | add/replace a column | `SELECT expr AS c` |
| `merge` | combine two structs | join | `JOIN` |
| `mapfields` | retype each field | cast-all / `describe` | apply over columns |
| `partition` | — (needs rows) | group | `GROUP BY` |

This is the relationship: the dataframe is not a bespoke feature but the
**columnar carrier of a record-shape calculus** whose four shape transforms also
work on plain structs, and whose collection op (`partition`) appears once there
are many rows. The query carrier (lowering the same calculus to SQL) is the
natural third instance.

## How the result type is computed: the row calculus

The signatures below say "the compiler computes a new row type"; this section
says *how*, because that mechanism is the whole feature. None of it is user
type-level programming — it is a **fixed, compiler-internal calculus** over one
data structure (a row), invoked only by the five blessed primitives. This is the
sealed exception to Tel's "no row polymorphism / no value-as-type machinery"
stance (see [tuples](../05-types/04-tuples-and-arrays.md) and
[type-system overview](../05-types/01-type-system-overview.md)).

### Labels: what a column name *is*

A column name is a **label** — a compile-time identifier that is **neither a term
nor a type**, but a third sort the compiler tracks (Tel already states field
names are "static type information only", with no runtime surface). A label has
identity at compile time (`price` is the same label everywhere) and is the
**key** in a row.

The consequence for the signatures: every place a column name appears as an
*argument* — `project[item, price]`, `by = [region, item]`,
`into = [north, south, east, west]`, the `total =` in `extend(total = …)` — is a
**label in type-argument position**, even though it shares surface syntax with
values. `into = [north, south]` is **not** a `List[Text]`; it is a *literal list
of labels* the compiler reads at compile time and lifts into the result row. A
runtime `List[Text]` could never appear there, because it has no compile-time
identity to become field names — which is exactly why `pivot`'s target names
must be static.

### A schema is a row; the primitives are functions on rows

Write a **row** as a finite, ordered map from labels to types, e.g.
`ρ = {a: A, b: B}`. A record type is `{ρ}`; a table is `Table[ρ]`. Three
operations on rows are all the calculus needs:

- `ρ.ℓ` — the type stored at label `ℓ` (lookup).
- `ρ ⊕ {ℓ: τ}` — **override-insert**: add `ℓ:τ` if `ℓ ∉ dom ρ`, else replace its
  type. (Used by `extend`.)
- `ρ ↾ L` — **restrict** to a set of labels `L ⊆ dom ρ` (used by `project`);
  `ρ ∖ L` is its complement.

Each blessed primitive is then a **type-level function** with these signatures
(premises above the line, result row below):

```text
project   L ⊆ dom ρ
          ─────────────────────────────────
          project[L] : Table[ρ] -> Table[ρ ↾ L]

extend    ℓ : Label    e : ColExpr over ρ, of type τ
          ─────────────────────────────────────────
          extend(ℓ = e) : Table[ρ] -> Table[ρ ⊕ {ℓ: τ}]

merge     K ⊆ dom ρ₁ ∩ dom ρ₂   ρ₁.k = ρ₂.k for k ∈ K
          ─────────────────────────────────────────────
          merge(on=K) : Table[ρ₁], Table[ρ₂] -> Table[ρ₁ ⊕ (ρ₂ ∖ K)]

partition K ⊆ dom ρ
          ───────────────────────────────────────
          partition[K] : Table[ρ] -> Grouped[ρ ↾ K, ρ]
```

`mapfields` is the one that quantifies over an *unknown* label set. It is
parameterised by an **ordinary per-type trait** `Tr` carrying **two halves** — a
**static / type-level** transform (the associated type `Tr::Out`, i.e.
`T → T::Out`) and a **dynamic / value-level** transform (a method on the column).
It requires **every** column type to implement `Tr`, and lifts the *pair* across
the row: the type half rebuilds the row *type* label-by-label, the value half
rebuilds the row *value* label-by-label.

```text
mapfields  ρ.ℓ : Tr   for every ℓ ∈ dom ρ
           ──────────────────────────────────────────────
           mapfields[Tr] : Table[ρ] -> C[ {ℓ : (ρ.ℓ)::Tr::Out | ℓ ∈ dom ρ} ]
```

The per-field result type is the **associated-type projection** `(ρ.ℓ)::Tr::Out`,
so each field can land at a *different* concrete type — this is what makes
`summary` honest (next section). When `Tr::Out` is a *transform* (e.g.
`ToText::Out = Text`) the carrier `C` is preserved (`Table`); when it *reduces* a
column to a scalar the table collapses to a record. So `mapfields` is "lift a
trait method across every field and project its (possibly associated) result type
into a new row" — the magic stays in the row rebuild; the trait `Tr` is just
ordinary code.

**The `ρ.ℓ : Tr for every ℓ` premise is not a new user-facing bound.** Because
schemas are always concrete in user code (Tel is **monomorphic** here — no row
polymorphism), the premise **discharges to `N` ordinary `FieldType : Tr` checks**
over the known field set, exactly like any method-call resolution — no quantifier,
no new syntax. The quantification lives *inside* the `mapfields` primitive, not in
user code; it would only need *written* syntax under row polymorphism (a function
abstracting over an unknown row `R`, stating "every field of `R` implements `Tr`"),
which this TIP excludes (see
[*Naming and reuse of derived schemas*](#naming-and-reuse-of-derived-schemas)).

Because `Tr` is ordinary code, it **may be any user trait**: a user who writes a
`Tr` with an associated `Out` gets a new field-wise schema transform *without* a
new primitive. That does **not** crack the seal — the sealed thing is the closed
set of *operations* (`mapfields` and friends) and *carriers*, not the traits they
take. Composing them lets users build their own dataframe library (monomorphically
— see *Naming and reuse of derived schemas*); what they still cannot do is add a
sixth operation or a new carrier.

### Worked answers to the three questions

**`extend` is override-insert, folded.** A multi-binding `extend` is just the
`extend` rule applied left-to-right:

```text
extend(c = …, d = …) on ρ = {a, b}
  = ( {a,b} ⊕ {c: C} ) ⊕ {d: D}
  = {a, b, c, d}          # C, D are the inferred ColExpr types
```

So `{a,b}` extended by `{c,d}` is `{a,b,c,d}` because `⊕` is map union with the
new labels appended. A name already in `ρ` would replace (same `⊕`), not
duplicate.

**`agg` is project-then-concatenate — flat by construction.** `group_by` =
`partition`, giving `Grouped[ρ ↾ K, ρ]`; `agg` adds the aggregate fields onto the
*key row*, never nesting:

```text
group_by[K].agg(m₁ = r₁, …, mⱼ = rⱼ) on ρ
  : Table[ (ρ ↾ K) ⊕ {m₁: σ₁, …, mⱼ: σⱼ} ]      where σᵢ = type of reducer rᵢ
```

For `ρ = {a, b, c}` grouped by `K = {a}` with one aggregate, the result is
`{a, m: σ}` — the key labels and the aggregate labels in one **flat** row. (If
you grouped by all of `{a,b,c}` and added no aggregates you would get `{a,b,c}`
back, which is the degenerate "flat" case.) It is flat because `⊕` concatenates
rows; there is no `{keys: …, aggs: …}` nesting.

**`pivot` is project-then-extend over a *static label list*.** This is the one
the usage example hid. Its full type-level signature:

```text
pivot   I ⊆ dom ρ        # index labels, kept
        p ∈ dom ρ        # the `on` column; its VALUES are matched, type unused in result
        v ∈ dom ρ        # the `using` (value) column
        into = [n₁ … nₖ] # a literal label list, distinct, disjoint from I
        f : Reducer[ρ.v, σ]     # the aggregation: value-column type ρ.v reduced to σ
        ────────────────────────────────────────────────────────────────────────
        pivot(index=I, on=p, using=v, agg=f, into) :
            Table[ρ] -> Table[ (ρ ↾ I) ⊕ { n: σ | n ∈ into } ]
```

The crucial simplification: **every target column has the *same* type `σ`** (one
aggregation of one value column), so the result row is the index row extended by
one `σ`-typed field per label in `into`. No value-dependent typing is needed —
the names come from the static `into` list, not from the data. The runtime job is
only to *route* each row's `p`-value to the matching target name; the `unmatched`
policy covers `p`-values that hit none. And when `ρ.p` is a closed union whose
variants are exactly `into`, the routing is statically exhaustive and `unmatched`
is dead (see the pivot signature below).

This is why `pivot` needs no dynamic-schema fallback: it is `project[I]` followed
by `k` applications of the `extend` rule, all with compile-time-known labels.

## Naming and reuse of derived schemas

Every operation above mints a *fresh* row type, so a natural worry is that the
schemas are all anonymous and unnameable. They are anonymous — but because Tel
records are **structural** (compared by exact shape; see
[tuples](../05-types/04-tuples-and-arrays.md) and the *structural alias
identity* of [TIP-0002](0002-untagged-unions-and-sealed-traits.md)), anonymity is
not the obstacle it would be in a nominal language.

### Concrete schemas: a plain alias names them, for free

A `type` alias is a name for a *shape*; structural identity makes it unify with
the derived row automatically — no special feature:

```tel
type Sale         = { item: Text, price: EurAmt, qty: UInt32 }
type EnrichedSale = { item: Text, price: EurAmt, qty: UInt32, total: EurAmt }

let t2 = sales.extend(total = price * qty)   # : Table[{ item, price, qty, total }]
let t3 : Table[EnrichedSale] = t2            # ✅ same shape ⇒ same type
```

`EnrichedSale` is not "the type `extend` returns" nominally; it is *a* name for
that shape, and the two are interchangeable. By design you name only the
**endpoints** you care about (the input schema, a published output) — pipeline
**intermediates stay anonymous**, which is the whole point of the *auto-derived
row* (the user never writes the mid-chain `{ item, price, qty, total }` by hand).

### A `schema_of` trick to name a transform's result without spelling it

Writing the full shape of a derived schema is tedious and drifts when a column
changes. A `schema_of` (a `typeof` in type position) lets a derived type be named
by **pointing at an expression** instead of transcribing its shape:

```tel
type Enriched = schema_of(sales.extend(total = price * qty))
# ≡ { item: Text, price: EurAmt, qty: UInt32, total: EurAmt }
```

This is decidable and stays structural — it *types* the expression, it does not
evaluate it (no value-dependence, no dependent types). It is a quality-of-life
alias, not new expressive power. `TODO(open):` exact spelling (`schema_of` /
`typeof` / `T.schema`) and whether it may reference a table value or only a
table *type*.

### Nominal frames: an explicit `newtype` cap at the boundary

A `type` alias is transparent, so it cannot stop a same-shaped `{...}` from being
accepted, nor carry its own methods. When a frame schema should be a *distinct*
domain type, wrap it with `newtype` — but note the transforms **only ever emit
structural rows**, so nominal identity is applied **explicitly at the ends**, not
threaded through the calculus:

```tel
newtype Report = { region: Text, total: EurAmt }
let r = Report.wrap(sales.group_by(by = region).agg(total = margin.sum))
```

### Reuse is monomorphic; row polymorphism is out of scope

Naming a *transform* rather than a *result* — e.g.
`type Enriched[R] = extend[R, total, EurAmt]`, a type-level **function** over an
unknown row `R` — would require the closed operators to appear in user signatures
with row variables and row constraints ("`R` lacks `total`", "every field of `R`
is `Summarisable`"), i.e. **row polymorphism**. Tel **does not provide this**, and
it is *not* a deferred future TIP — it is excluded. The cost (row unification in
inference, a row-constraint surface syntax, error messages showing operator
expressions) buys no new expressive power — only the ability to put a combinator
behind an *abstract, published* signature — and it conflicts with the frozen,
conservative, fast-to-compile priorities. So **all reuse is monomorphic**:

- **Factor pipelines into helpers re-checked at each concrete call site**
  (composes with the inline-function story,
  [TIP-0009](0009-inline-lambdas-and-non-local-control-flow.md)). The result row
  is recomputed per call; no row variable ever appears in a signature, so **zero
  new type surface**. This already lets users build their own dataframe library by
  composition — what it cannot do is publish a combinator with an abstract row in
  its signature.
- **A column identifier may be a comptime/`inline` parameter** — specialised at
  each call site like an array length in a const generic — giving reusable
  column-generic helpers *without* an abstract label variable:

  ```tel
  inline fn znorm[comptime c: Label](t) = t.extend({c} = ({c} - {c}.mean) / {c}.std)
  znorm[price](sales)        # `c` specialised to `price`; row computed concretely
  ```

  Because `c` is specialised before the body is typed, `c ∈ dom R` is an ordinary
  concrete check — no `Has`/`Lacks` constraint, no row variable. An *abstract*
  label variable (one that survives into a signature) is row polymorphism by
  another name, and is out of scope for the same reason.

## Method signatures

The primitives above fix the *type-level* transforms; this section pins the
*call shape* of the four user-facing operations the design must get right —
**add column(s)**, **summary**, **aggregate**, **pivot**. They are not four
unrelated APIs: three of them (`extend`, `agg`, `pivot`) share a single
**column-expression** sublanguage, and the fourth (`summary`) is the
field-generic `mapfields` reduce. Getting the signatures right is mostly getting
*those two scopes* right.

### The governing principle: labels are the only magic; reads are ordinary code

The whole operation surface reduces to one rule:

> **Labels are the only magic; everything you *read* is ordinary code.**

A column name is *magic* (a compile-time label in type-argument position, see
[*Labels*](#labels-what-a-column-name-is) above) **only** where it sits in
**schema position** — where it decides the result *type*. That is both the names
an operation *introduces* (`extend`'s `total =`, `agg`'s output names, `pivot`'s
`into`) and the names it *references as structure* (`project`/`select`/`drop`
lists, `group_by`'s `by`, `merge`'s `on`, `pivot`'s `index`/`on`/`using`). None
of those are values; they are type-level identifiers.

Everywhere a column name appears in **value position** — the body you *read* to
compute something (`extend`'s RHS, `filter`'s predicate, `agg`'s reducer,
`pivot`'s cell aggregation) — it is **ordinary code**, not a special form. There
is no bespoke "column-expression sublanguage" the compiler has to recognise; the
read is a plain expression whose bare column names resolve through the ordinary
**receiver mechanism** ([TIP-0010](0010-lambda-receivers-and-builder-dsls.md)).

### One lambda flavor: a receiver closure over whatever the op iterates

Earlier drafts described two *syntactic* flavors — a "blessed column-expression
special form" for `extend`/`agg`/`pivot` versus an "ordinary lambda" elsewhere.
**That split is dropped.** There is **one** flavor: every read is the **more
general lambda** — a [TIP-0010](0010-lambda-receivers-and-builder-dsls.md)
**receiver closure** whose `this` is **whatever the operation iterates**:

- **per-row ops** (`filter`, `extend`, `map`) — `this` is a **row** of `R`;
  bare `price` is `this.price`, a **scalar** (`EurAmt`). `price * qty` is an
  ordinary per-row scalar expression.
- **per-group ops** (`agg`, `pivot` cells) — `this` is a **group**; bare `price`
  is the group's **column** (`Column[EurAmt]`), so `(price * qty).sum` is
  ordinary column code (`.sum` collapses the column to the aggregate cell).

So "always a lambda" does not mean "always a *row* lambda": it means **always a
receiver closure over the iterated unit** — a row for the per-row ops, a group
for the aggregations. Bare names work the same way in both (implicit `this`,
ordinary member resolution); only what `this` *is* changes. The readability win
(no `pl.col("price")` handle, no explicit `row => row.price`) comes from the
receiver, not from a special form. The single remaining special case is
`mapfields` (below): there the closure is applied to *every* field, so its
parameter is a column of **unknown** type and only field-generic ops
(`c.to_text`, `c.summary`) are legal on it — the one place an unknown-field-set
closure appears.

**No allocation guarantee — the inliner earns the speed.** Because the read is a
genuine per-row (or per-group) closure, the model does **not** promise rows are
never materialised. Instead the **row inliner must vectorise the common
elementwise case**: an `extend` body of column arithmetic/comparisons/casts
inlines to per-column kernels with no row struct ever built; a body that calls an
opaque scalar function may fall back to materialising a row and running per-row.
This is the deliberate trade — one uniform, fully general surface in exchange for
a *best-effort* (not structural) no-row-allocation property. See
[*Storage, mutability and evaluation*](../10a-dataframes/03-storage-mutability-evaluation.md)
for what the inliner is expected to handle.

`TODO(open):` **tentative `must_inline` / `@hot` annotation.** A binding marker
that turns "this body did not vectorise / left the inlinable subset" into a
**compile error** instead of a silent per-row cliff. Tentative — only added if
the silent-cliff risk proves real in practice; not committed.

`TODO(open):` spelling of a column reference when a bare name is ambiguous (a
column named like a local, or a computed name). Reserve a fallback handle
(`col.price` / `$price`) for those cases while keeping bare names the default.

### The one law that separates extend / agg / pivot: result length

All three take the same receiver-closure reads; they differ **only** in the
*length contract* the context imposes on what each binding's closure produces
**aggregated over the iteration**:

| context | each binding's result must be | so it contributes |
| --- | --- | --- |
| `extend` | one value **per row** ⇒ a **column** (length `N`) | a new/replaced column |
| `agg` (per group) | one value **per group** ⇒ a **scalar** (length 1) — a reducer must appear | one aggregate field |
| `pivot` value | one value **per `(index, on)` cell** ⇒ a **scalar** | one cell of a target column |

That is the whole distinction. In `extend` (`this` = row) `price * qty` is a
per-row scalar, yielding a length-`N` column; the *same* surface text in `agg`
(`this` = group) reads `price`/`qty` as columns and is a type error until a
reducer collapses it (`(price * qty).sum`). Stating the law as a length contract
is what lets one closure flavor serve all three and keeps the error messages
crisp ("expected a scalar aggregate, got a column of length N").

### add column(s) — `extend` (the `extend` primitive)

```tel
many.extend(
    total = price * qty,     # ColExpr[EurAmt] — length N, a new column
    net   = total * 0.9,     # may read `total` (see threading, below)
    qty   = qty as Float64,  # name collision ⇒ replace-in-place, type updated
)
# : Table[{ item, price, total: EurAmt, net: EurAmt, qty: Float64 }]
```

- **Variadic named bindings.** Each `name = expr` is one `extend` of the row
  type — a new field if `name` is fresh, a replace (with the new type) if it
  collides. The multi-binding form is exactly *repeated* `extend`, which is why
  it needs no new primitive.
- **Threading (sequential, not parallel).** Recommend dplyr `mutate` semantics:
  bindings evaluate **left-to-right**, and each sees the columns added by the
  ones before it (`net` sees `total`). This reads top-to-bottom and is literally
  the fold "`extend` each binding into `R` in turn". The alternative — Polars
  `with_columns` parallel semantics, where every RHS sees only the *original*
  `R` — is rejected as the less readable default. `TODO(open):` confirm; parallel
  is friendlier to the vectorised engine (no intra-call dependency), so an
  explicit parallel variant may still be wanted.
- **On a replace, the RHS reads the *old* column** (`qty = qty as Float64` casts
  the existing `qty`).
- Value-returning: `fn[R] Table[R].extend(**cols) -> Table[R']`. Buffers reused
  when `self` is `uniq` (the COW story under *Mutability*).

### summary — `describe` (the `mapfields` reduce)

`summary` is `mapfields` instantiated at a `Summarisable` trait whose result is
an **associated type**, so each column lands at its *own* summary type:

```tel
trait Summarisable {
    type Summary                              # associated — each impl picks it
    fn summarise(col: Column[Self]) -> Self::Summary
}

many.summary
# : { item: Text::Summary, price: EurAmt::Summary, qty: UInt32::Summary }
many.price.summary          # one column
# : EurAmt::Summary
```

- **The result type is the projection `T::Summary`, not a generic `Summary[T]`.**
  This is the distinction that matters: a uniform constructor `Summary[T]` would
  force one shape for all `T`, but `Bool::Summary` (`count`, `n_true`, `n_false`)
  is genuinely a *different type* from `Real64::Summary` (`mean`, `std`, `min`,
  `max`, quantiles). An associated type lets each `impl Summarisable for τ` choose
  its own concrete record, so the result struct is honestly heterogeneous:
  `Table[{a: Int32, b: Real64, c: Bool}].summary :
  {a: Int32::Summary, b: Real64::Summary, c: Bool::Summary}` — three distinct
  types. Per the row calculus, that field is exactly the
  `(ρ.ℓ)::Summarisable::Summary` projection `mapfields` builds.
- **Returns a record, not a table** — the table collapses (the `mapfields`-reduce
  case). This is deliberately *not* pandas' `describe()` frame: pandas returns a
  rectangle only by coercing every stat to `Float`. Tel's columns are
  heterogeneous, so the type-honest result is a record of per-type summaries.
- **Every column type must implement `Summarisable`** — that is the
  `ρ.ℓ : Summarisable for every ℓ` premise of the `mapfields` rule. The stdlib
  impls fan the stats out by capability (`count`/`null_count` for all; `n_unique`
  /`mode` where `Eq`; `min`/`max`/`median` where `Ord`; `mean`/`std`/quantiles
  for numerics), but that is library policy, not a type-system rule.
- `TODO(open):` a secondary `summary_table` that keeps only the **common** stats
  (count, null_count, plus the numeric-common ones) *can* be a real
  `Table[{ stat: Text, … }]` for pretty-printing — but it needs a fixed common
  stat set and float coercion. Offer it as a render helper, not the typed result.

### aggregate — `group_by` + `agg` (`partition` + `mapfields`)

```tel
many.group_by(by = [region, item])      # : Grouped[{ region, item }, Sale]
    .agg(
        revenue = (price * qty).sum,     # reducer ⇒ scalar  : EurAmt
        n       = count(),               # group cardinality : UInt64
        avg_p   = price.mean,            #                   : EurAmt
    )
# : Table[{ region, item, revenue: EurAmt, n: UInt64, avg_p: EurAmt }]
```

- `group_by` is the user-facing name for the `partition` primitive; `by` projects
  the key fields out of `R` (a single field may drop the brackets:
  `by = region`).
- **Result row type = key fields `++` agg fields.** The key fields come straight
  from the `partition` key record; each agg field's name is its binding name and
  its type is the **reducer's result type** over the (elementwise) column
  expression.
- **`agg` is a per-group receiver closure under the length-1 contract** (above):
  `this` is the group, bare names are its columns, and every binding must reduce
  to a scalar. `count()` is the one reducer with no column argument and is
  provided as a free name in the scope.
- **Keyless aggregate.** `many.agg(total = revenue.sum)` (no `group_by`) is a
  whole-table reduce. Recommend it returns a **record** (one group, collapse) —
  consistent with `summary` — rather than a one-row `Table`. `TODO(open):` confirm
  record vs 1-row table; dplyr/Polars return a 1-row frame, but the record is the
  honest `mapfields`-reduce result and composes with `summary`.
- `TODO(open):` how much of `Grouped[K, R]` is user-visible beyond `.agg`
  (carried over from the open questions below).

### pivot — static target columns (`extend` onto a known schema)

```tel
sales.pivot(
    index     = [date],          # surviving key columns (project)
    on        = region,          # column whose VALUES select a target column
    using     = revenue,         # the value column fed to `agg`
    agg       = sum,             # pivot implies aggregation over each cell
    into      = [north, south, east, west],   # STATIC target names ⇒ new fields
    unmatched = error,           # policy: error | drop | other
)
# : Table[{ date, north: EurAmt, south: EurAmt, east: EurAmt, west: EurAmt }]
```

- **`into` makes the schema static.** The new column *names* are arguments, not
  inferred from the data, so the result row type is `index fields ++ one field
  per name in into`, each typed as `agg`'s result over `using`'s type. This is
  the "repeated `extend` onto a known schema" of the primitives table — no
  dynamic-schema fallback.
- **`on` is matched at runtime against the static `into` names**, so `into`'s
  entries must be *values* of `on`'s element type. The payoff: **if `on` is a
  closed enum/union ([TIP-0002](0002-untagged-unions-and-sealed-traits.md)) and
  `into` lists every variant, "unmatched" is statically impossible** — the
  compiler can check exhaustiveness and `unmatched` may be omitted entirely.
  `unmatched` is *required* only when `on` is open (e.g. `Text`). This ties
  pivot's safety directly to the union machinery.
- **`agg` is mandatory** because a `(index, on)` cell can hold many rows — unless
  the pair is statically `uniq`, in which case `agg = first` (or its omission) is
  allowed. The reducer is the same per-group receiver closure, length-1 contract.
- `unmatched = other` adds a single catch-all `other` column. `TODO(open):`
  whether `other` is a scalar aggregate or a nested `Table` of the dropped rows,
  and which of `error` / `drop` / `other` is the default (lean `error` — no
  silent loss).
- The inverse, `melt`/`unpivot`, is `mapfields` + a union of the melted columns'
  types; not detailed here.

### Summary of the four

| op | primitive(s) | scope | returns |
| --- | --- | --- | --- |
| add column(s) `extend` | `extend` (folded) | column-expr, length `N` | `Table[R']` |
| `summary` | `mapfields` reduce | field-generic | a **record** of `T::Summary` |
| aggregate `group_by`+`agg` | `partition` + `mapfields` | column-expr, length 1 | `Table[keys ++ aggs]` |
| `pivot` | `project` + repeated `extend` | column-expr, length 1 | `Table[index ++ into]` |

`TODO: review` — signatures above pin call shape and result types; the open
markers feed the questions list below.

## Mutability

There are **not** two dataframe types. Mutability falls out of Tel's ordinary
ownership/mutability axes (see the ownership table in the records/memory
chapters), and the schema-transform story decides most of it for us:

- **Shape transforms are value-returning, never in-place.** `project`/`extend`/
  `merge`/`mapfields`/`partition` change the *static* row type
  (`Table[{a,b}]` → `Table[{a,b,c}]`), and a value cannot mutate into a
  different type. So the whole "magic" surface is functional by construction —
  the same reason Polars and dplyr are immutable, and the mess pandas' in-place
  mutation creates.
- **Value-returning does not mean copying.** When the input is `uniq`/affine the
  columnar buffers are **moved / reused (copy-on-write)**, so
  `t2 = t.extend(...)` is cheap. Returning-a-new-value and reusing-storage are
  independent.
- **Mutation only applies within a fixed schema** — append/drop rows, update
  cells, fill nulls. That is the normal mutable axis and uses a **`!Table`
  unique builder**, exactly the `!List` builder pattern:

```tel
t2 = t.extend(total = price * qty)   # new typed value; buffers reused if t is uniq

uniq b : !Table[Sale] = Table.builder()
b.push({ item = "pen", price = eur(2), qty = 5 })
let table = b.finish()               # : Table[Sale]
```

So: one `Table`, transforms are functional (with COW), and the `!`/`uniq` axis
covers fixed-schema mutation — no frame-specific mutable/immutable split.

## A closed set of operations, not open type-level programming

The seal is on the **operations and carriers**, not on what users may build with
them. The calculus is **closed** in exactly two places: a fixed set of **five
operations** (`project`, `extend`, `merge`, `mapfields`, `partition`) over a fixed
set of **carriers** (record, column-table, and later query). Users cannot define a
sixth operation or add a carrier; that is the seal, and it is what keeps the
feature compatible with Tel's frozen-language goal and its stance against
open-ended type-level programming (see
[`antifeatures`](../02-philosophy/04-antifeatures.md)).

What users *can* do — and this is intended, not a leak — is **compose** the five
freely and supply **ordinary traits** to `mapfields` (a user `Tr` with an
associated `Out` is a new field-wise transform without a new primitive). Composed
far enough, that is a user-written dataframe library. The earlier framing of this
as "doubly closed — users cannot build their own schema-transforming library" was
too strong: they can, by composition; what stays sealed is the *primitive set*,
not its use. The one capability deliberately **excluded** is doing this behind an
*abstract, row-polymorphic signature* (see
[*Naming and reuse of derived schemas*](#naming-and-reuse-of-derived-schemas)) —
the calculus is granted in **monomorphic** position only, and row polymorphism is
not planned (not a future TIP).

So it remains a bounded, first-order exception — a **small closed type-level
sublanguage**, not a new tier of general type-level programming and far from
dependent types — but it is honestly more than "one blessed type": it is a closed
*calculus* users program in by composition.

`TODO(open): sealing the operation set caps expressiveness — a user who wants a
sixth schema transform must get it into std. Confirm the five cover the real
workloads, or define how a new primitive is proposed.`

## Storage: columnar (list of columns)

The blessed `Table` stores data **column-major — a struct of equal-length
arrays (a list of columns)**, not row-major. Reasons:

- It is the layout the schema-transforming primitives want: `project`,
  `extend`, `cast`, and `mapfields` add, drop, or rewrite *whole columns*,
  which is cheap column-major and expensive row-major.
- It enables SIMD / vectorised per-column kernels (the
  [vectorised/transposed substrate](../17-standard-library/04-core-collections.md))
  and matches how analytical workloads actually process this data.
- It makes the columnar-engine / SQL pushdown natural.

The **row view** (array-of-structs — iterating the table as a sequence of `R`
records) is offered as a *view* over the columnar storage, but it needs **no
compiler support**: a plain `List[R]` of an ordinary record is already
expressible today. So the magic is spent specifically on the columnar `Table`;
the row-sequence form is the trivial case that already works without help.

## Laziness: out of scope

Polars' other headline feature is **lazy evaluation** — recording an operation
graph and optimising it (predicate/projection pushdown, reordering) before
running. That is an *optimisation*, orthogonal to the static typing this TIP is
about: the row type and the blessed primitives are identical whether evaluation
is eager or lazy. It is also genuinely hard — a query planner/optimiser is a
large subsystem, and it sits awkwardly with Tel's embedding goal (the host, or a
real database behind a capability, is the better place for heavy planning).

So **Tel's `Table` evaluates eagerly** and does not ship a lazy query optimiser.
If predicate pushdown ever matters, the natural path is the capability-backed
table pushing operations down to a host engine / SQL (see the relational-algebra
closure), not a general in-language lazy graph. Forgoing laziness costs some
peak performance on large pipelines but keeps the feature tractable and focused
on the part only the compiler can provide — the static schema.

## What it costs

This needs **substantial compiler magic**: deriving a fresh record type from a
column declaration, propagating it through the blessed primitives (which *change*
the schema), and keeping column access statically checked. That is more than a
library on the existing collection substrate — it is the compiler-recognised
exception described above. The scope is deliberately bounded by the closed
primitive set and by forgoing laziness, which is what keeps it tractable for
**1.0**.

- **In the 1.0 standard library** (current plan).
- The matrix chapter's *named axes* bullet is **removed** — a matrix stays the
  homogeneous, fixed-shape numeric type, with axis labels (if any) being just
  index labels, not a heterogeneous schema.

## Open questions

- `TODO(open):` the **query carrier** — does the same calculus lower to
  relational algebra / SQL, and is that a 1.0 carrier or a later one? **Framing
  correction:** SQL is *not* a capability that restricts which transform
  arguments are allowed. `filter` takes an ordinary `Row.fn() : Bool` receiver
  lambda and `extend` an ordinary per-row expression against *any* backing; if
  the expression lowers to the engine it is pushed down (`WHERE` / `SELECT expr
  AS`), and if it cannot, the engine **materialises and runs it in Tel** — slower
  but identical type and result. So pushdown is an *opportunistic optimisation*
  with an in-memory fallback, invisible to the type system; the query carrier
  carries **no operation restriction**. (Two edge cases, both opt-in/boundary:
  an optional "guarantee pushdown — fail rather than silently materialise"
  assertion for huge remote tables, and SQL-representability of `R` checked only
  where a `Coll[R]` is *bound* to a real SQL table, like deserialisation.) The
  concrete customer driving the 1.0 decision is the typed, injection-safe query
  DSL in [`inputs/tip9-tip10-dsl-examples.md`](../inputs/tip9-tip10-dsl-examples.md)
  (Example 2).
- `TODO(open):` **clause order vs schema** (from that query DSL example).
  `project` must be written *last* so `filter` / `sort_by` can still see columns
  it would drop. SQL's `WHERE` / `ORDER BY` may reference unselected columns; the
  typed calculus cannot once `project` has removed them. Decide whether the query
  carrier remembers pre-projection columns for predicate/ordering, or requires
  `project` last.
- `TODO(open):` how far the carrier abstraction is exposed — and **whether there
  are three carriers at all at the type level.** Leaning *no*: the only
  type-level distinctions are the **row schema `R`** and **one row vs many**
  (`R` vs `Coll[R]`, the ordinary scalar/collection split). **Columnar** is a
  *storage layout* of `Coll[R]`, not a type; the **query/SQL** target is a *lazy
  backing* of `Coll[R]` (opportunistic pushdown, per above), not a type. So
  "record / column-table / query" may collapse to `R` and `Coll[R]` with layout
  and backing as orthogonal, mostly-non-type annotations — preserving the real
  insight (one record-shape calculus over a single row and over many) while
  dropping the claim that "columnar many-rows" and "SQL many-rows" are distinct
  *types*. Reconcile with the closed-carrier seal (the seal then covers the five
  operations, not a carrier list). (Keep the calculus sealed either way.)
- `TODO(open):` `mapfields` shape rule — the result carrier depends on whether
  the per-field op transforms (shape preserved) or reduces (table → record). Pin
  down how the compiler distinguishes the two and what each carrier yields.
- `TODO(open):` how `partition` exposes the `Grouped[K, R]` intermediate — is it
  a user-visible type, or only reachable via `group_by(...).agg(...)`?
- `TODO(open):` `pivot`'s unmatched-value policy — which of error / drop /
  catch-all `other` is the default, and is the catch-all a single column or a
  nested table?
- `TODO(open):` reconcile with the `Tables` section of
  [`core-collections`](../17-standard-library/04-core-collections.md): is the
  blessed `Table` here the *same* type that chapter sketches (it already frames
  joins as type-functions), and does the chapter become the user-facing home
  once this TIP is accepted?
- `TODO(open):` relationship to the serialisation data model
  ([TIP-0007](0007-serialisation-data-model-and-formats.md)) — a frame read from
  CSV/Parquet arrives with a runtime schema that must be reconciled with the
  static row type.
- `TODO(open):` **column-expression threading** in `extend` — sequential
  (dplyr `mutate`, later bindings see earlier ones; recommended) vs parallel
  (Polars `with_columns`, every RHS sees only the original `R`). Confirm
  sequential as the default and whether an explicit parallel variant is offered.
- `TODO(open):` **column reference fallback** — bare column names are the default
  in the receiver-closure read; pin the escape handle (`col.x` / `$x`) for when
  a name collides with a local or is computed.
- `TODO(open):` **keyless aggregate** — does `t.agg(...)` (no `group_by`) return a
  **record** (collapse, consistent with `summary`) or a one-row `Table`?
- `TODO(open):` **`summary` result** — confirm the typed result is a *record* of
  the associated-type projection `T::Summary` (via a `Summarisable` trait), with a
  separate `summary_table` render helper for the common-stat rectangle; and pin
  the `Summarisable` impls' per-tier stat sets.
- **Resolved — `mapfields` trait openness:** `mapfields` takes *any* user trait
  with an associated `Out`; the seal is on the five operations and the carriers,
  not on the traits. (See *A closed set of operations*.)
- **Resolved — derived schemas are structural** and named by a plain `type` alias
  (unifying by shape), with `schema_of(expr)` as a spell-free alias and `newtype`
  for an explicit nominal cap. (See *Naming and reuse of derived schemas*.)
  `TODO(open):` final spelling of `schema_of`.
- **Resolved — reuse is monomorphic; row polymorphism is excluded** (not a future
  TIP). Helpers are re-checked per call site, and column identifiers may be
  comptime/`inline` parameters; no abstract row or label variable enters a
  signature. (See *Naming and reuse of derived schemas*.)
</content>
