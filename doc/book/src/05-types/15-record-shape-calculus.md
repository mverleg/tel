# The Record-Shape Calculus

A **record shape** — the set of named, typed fields of a
[structural record](04-tuples-and-arrays.md) — can be transformed at the type
level by a **small closed set of primitives**. Add a field, drop a field, combine
two shapes, retype every field, group many values by a key: each computes a *new*
shape from an old one, and the compiler tracks the result type precisely. This is
a type-system feature in its own right; it is **not** specific to dataframes.

What makes it possible to bless is that the primitives are not really about any
one container — they are about **record shapes**, and so apply over a **carrier**:

- a **single record** (an ordinary structural-record value), and
- a **column-table** — the [dataframe](../10a-dataframes/01-overview.md) `Table[R]`,
  whose schema *is* the record type `R`.

The first four primitives are pure shape transforms and work on either carrier;
the fifth needs many values and so appears only on the table. This chapter defines
the calculus and its type rules; the [Dataframes](../10a-dataframes/01-overview.md)
chapters apply it to tables, where it gets its everyday name (`select`, `join`,
`group_by`, `pivot`).

`<!-- TODO: review -->`

## Labels, rows, and what a column name is

A column name is a **label** — a compile-time identifier that is neither a term
nor a type, but a third sort the compiler tracks. Labels have **no runtime
surface** (consistent with Tel's [no-reflection](../02-philosophy/04-antifeatures.md)
stance: field names are static type information only). A label has identity at
compile time and is the **key** of a row.

A **schema** is a **row**: a finite, ordered map from labels to types, written
`ρ = { a: A, b: B }`. A structural record type is `{ρ}`; a dataframe is
`Table[ρ]`. Three operations on rows are all the calculus needs:

- `ρ.ℓ` — the type at label `ℓ` (lookup).
- `ρ ⊕ { ℓ: τ }` — **override-insert**: add `ℓ:τ`, or replace its type if `ℓ`
  already present.
- `ρ ↾ L` — **restrict** to a label set `L ⊆ dom ρ`; `ρ ∖ L` is the complement.

Every place a column name appears as an *argument* — `select[item, price]`,
`by = [region, item]`, the `total =` in `extend(total = …)`, `into = [north,
south]` in a pivot — is a **label in type-argument position**, even though it
shares surface syntax with values. A literal label list is *not* a `List[Text]`;
it is read at compile time and lifted into the result row. That staticness is why
[`pivot`](../10a-dataframes/02-table-operations.md#pivot)'s target names must be literals.

## The five primitives

Each primitive is a **type-level function** on rows. Premises above the line, the
result row below.

```text
project   L ⊆ dom ρ
          ─────────────────────────────────
          project[L] : Table[ρ] -> Table[ρ ↾ L]

extend    ℓ : Label    e : a column-expression over ρ, of type τ
          ─────────────────────────────────────────────────────
          extend(ℓ = e) : Table[ρ] -> Table[ρ ⊕ { ℓ: τ }]

merge     K ⊆ dom ρ₁ ∩ dom ρ₂    ρ₁.k = ρ₂.k for k ∈ K
          ───────────────────────────────────────────────
          merge(on=K) : Table[ρ₁], Table[ρ₂] -> Table[ρ₁ ⊕ (ρ₂ ∖ K)]

partition K ⊆ dom ρ
          ─────────────────────────────────────────
          partition[K] : Table[ρ] -> Grouped[ρ ↾ K, ρ]

mapfields ρ.ℓ : Tr   for every ℓ ∈ dom ρ
          ──────────────────────────────────────────────
          mapfields[Tr] : Table[ρ] -> C[ { ℓ : (ρ.ℓ)::Tr::Out | ℓ ∈ dom ρ } ]
```

The first four are pure shape transforms and apply to **any carrier** (a single
record, or a `Table`); only `partition` needs a collection (there is nothing to
group in one record). `mapfields` is the one primitive generic over an *unknown*
field set — see below.

### `mapfields` and per-type traits

`mapfields` is parameterised by an **ordinary per-type trait** `Tr` carrying two
halves: a **static / type-level** transform (the associated type `Tr::Out`, i.e.
`T → T::Out`) and a **dynamic / value-level** transform (a method on the column).
It requires every column type to implement `Tr`, then lifts the *pair* across the
row — the type half rebuilds the row *type* label-by-label, the value half
rebuilds the row *value*.

Because the per-field result is the **associated-type projection** `(ρ.ℓ)::Tr::Out`,
each field can land at a *different* concrete type. When `Tr::Out` is a transform
(e.g. `ToText::Out = Text`) the carrier is preserved; when it *reduces* a column
to a scalar, a table collapses to a record — that is [`summary`](../10a-dataframes/02-table-operations.md#summary--describe).

The `ρ.ℓ : Tr for every ℓ` premise is **not a new user-facing bound**. Because
schemas are always concrete in user code (Tel is monomorphic here, see below), it
discharges to `N` ordinary `FieldType : Tr` checks over the known field set,
exactly like any method resolution — no quantifier, no new syntax. `Tr` is
ordinary code and **may be any user trait**, so a user can add a field-wise
transform without a new primitive.

## Every schema-changing operation is built from the five

```text
select                 = project
drop                   = project (the complement)
rename                 = project + extend
extend / with_columns  = extend
cast                   = extend (replace in place)
inner/left/right/outer join = merge
group_by + agg         = partition + mapfields (per group)
value_counts/crosstab  = partition + count (mapfields)
summary / describe     = mapfields (reduce each column)
melt / unpivot         = mapfields + a union of the melted columns' types
pivot                  = project + repeated extend onto static target labels
```

So the irreducible magic is exactly **`project`, `extend`, `merge`,
`mapfields`, `partition`**. Row `concat`/`union` is deliberately *not* a
primitive: it is not a transform but a *constraint* that two row types match — an
ordinary type equality the library checks.

The **one escape hatch** is a **dynamic-schema `Table`** whose schema is a runtime
value, needed only at the I/O boundary where a CSV/Parquet schema arrives at
runtime (see [evaluation](../10a-dataframes/03-storage-mutability-evaluation.md#the-io-boundary)).
No in-language operation produces one.

## How a result type is computed

Worked on the three operations users most often ask about:

**`extend` is override-insert, folded.** A multi-binding `extend` is the `extend`
rule applied left-to-right, so adding `{c, d}` to `{a, b}` gives `{a, b, c, d}`;
a colliding name replaces rather than duplicates.

```text
extend(c = …, d = …) on {a, b} = ({a,b} ⊕ {c: C}) ⊕ {d: D} = {a, b, c, d}
```

**`agg` is project-then-concatenate — flat by construction.** `group_by` =
`partition`, then `agg` adds the aggregate fields onto the *key* row, never
nesting:

```text
group_by[K].agg(m₁ = r₁, …) on ρ : Table[ (ρ ↾ K) ⊕ { m₁: σ₁, … } ]
```

For `ρ = {a, b, c}` grouped by `{a}` with one aggregate `m`, the result is
`{a, m: σ}` — key labels and aggregate labels in one **flat** row.

**`pivot` is project-then-extend over a static label list.** Its full signature:

```text
I ⊆ dom ρ        # index labels, kept
p ∈ dom ρ        # the `on` column; its VALUES are matched, its type unused in the result
v ∈ dom ρ        # the `using` value column
into = [n₁ … nₖ] # a literal label list, distinct, disjoint from I
f : Reducer[ρ.v, σ]                  # the aggregation: ρ.v reduced to σ
────────────────────────────────────────────────────────────────────
pivot(index=I, on=p, using=v, agg=f, into) : Table[ρ] -> Table[ (ρ ↾ I) ⊕ { n: σ | n ∈ into } ]
```

Every target column has the **same** type `σ` (one aggregation of one value
column), so the result is the index row extended by one `σ`-typed field per label
in `into`. No value-dependent typing is needed: the names come from the static
`into` list, not the data. This is why `pivot` is `project[I]` followed by `k`
applications of the `extend` rule, all with compile-time-known labels — and needs
no dynamic-schema fallback.

## Naming derived schemas

Every operation mints a fresh row type, so the schemas look anonymous. They are —
but a row type is a **structural** record (compared by shape, see
[tuples](04-tuples-and-arrays.md)), so anonymity is not the obstacle
it would be in a nominal language.

**A plain `type` alias names a concrete derived schema, and unifies with it for
free** (structural identity makes the two interchangeable):

```tel
type Sale         = { item: Text, price: EurAmt, qty: UInt32 }
type EnrichedSale = { item: Text, price: EurAmt, qty: UInt32, total: EurAmt }

let t3 : Table[EnrichedSale] = sales.extend(total = price * qty)   # ✅ same shape
```

By design you name only the **endpoints** (input schema, a published output);
pipeline **intermediates stay anonymous** — that is the point of the auto-derived
row type.

**`schema_of` names a transform's result without spelling its shape** — a
`typeof` in type position. It *types* the expression; it does not evaluate it, so
it stays decidable and far from dependent types:

```tel
type Enriched = schema_of(sales.extend(total = price * qty))
```

`TODO(open): final spelling — `schema_of` / `typeof` / `T.schema` — and whether
it may reference a table value or only a table type.`

**Nominal frames use an explicit `newtype` cap.** A `type` alias is transparent,
so it cannot stop a same-shaped record being accepted nor carry methods. The
transforms only ever emit *structural* rows, so nominal identity
([refined types](12-refined-types.md)) is applied explicitly at the
ends, never threaded through the calculus:

```tel
newtype Report = { region: Text, total: EurAmt }
let r = Report.wrap(sales.group_by(by = region).agg(total = margin.sum))
```

## Reuse is monomorphic

The calculus is granted in **monomorphic position only**. User code composes the
primitives at *concrete* schemas; what Tel deliberately does **not** provide — and
this is a permanent scope line, not a deferred feature — is **row polymorphism**:
a function abstracting over an *unknown* row `R` and naming a transform in its
signature (`type Enriched[R] = extend[R, total, EurAmt]`). That would force row
variables and row constraints ("`R` lacks `total`", "every field of `R` is
`Summarisable`") into the surface language, buying no new expressive power — only
the ability to publish a combinator behind an abstract signature — at a real cost
to inference, error messages, and the frozen-language goal.

So reuse takes two monomorphic forms:

- **Factor a pipeline into a helper re-checked at each concrete call site**
  (composing with the [inline-function](../09-functions/06-closures-and-lambdas.md)
  story). The result row is recomputed per call; no row variable appears in any
  signature. This already lets users build a dataframe library by composition.
- **A column identifier may be a comptime / `inline` parameter**, specialised at
  each call like an array length in a [const generic](07-generics.md):

  ```tel
  inline fn znorm[comptime c: Label](t) = t.extend({c} = ({c} - {c}.mean) / {c}.std)
  znorm[price](sales)        # `c` specialised to `price`; row computed concretely
  ```

  Because `c` is specialised before the body is typed, `c ∈ dom ρ` is an ordinary
  concrete check — no row variable, no constraint syntax.

## A closed calculus, not open type-level programming

The seal is on the **operations and carriers**, not on what users build with them.
Users cannot add a sixth primitive or a new carrier — but they *can* compose the
five freely and supply any ordinary trait to `mapfields`, which composed far
enough is a user-written dataframe library. It is a **small, closed, first-order
type-level sublanguage** — bounded, decidable, and far from dependent types — and
a granted exception to Tel's stance against open-ended type-level programming, not
a new general tier.

`TODO(open): the closed set caps expressiveness — a user who wants a sixth schema
transform must get it into std. Confirm the five cover the real workloads, or
define how a new primitive is proposed.`
