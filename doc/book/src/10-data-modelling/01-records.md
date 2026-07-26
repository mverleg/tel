# Records

A **record** (also *struct*) is a named product type: a fixed set of named,
typed fields bundled into one value. Records are the everyday building block for
domain data — an `Order`, a `User`, a `Point`.

```tel
struct Point {
    x: Real64,
    y: Real64,
}

struct Order {
    id:    Id[Order],
    total: EuroAmt,
    lines: List[OrderLine],
}
```

## What — fields, construction, immutability

- A record type lists its fields with names and types. Field order is fixed by
  the declaration.
- A record is constructed by giving every field a value. There are no
  uninitialised fields and no `null` — every field is set when the record comes
  into being (see [antifeatures](../02-philosophy/04-antifeatures.md)).
- Records are **values**: immutable by default. "Changing" a field produces a
  new record — see [Copy-update](#copy-update-with) below.
- Records are **nominal**: `struct Meters { v: Real64 }` and
  `struct Feet { v: Real64 }` are different types even though their shapes match.
  Identity is the declared name, not the structure. (The one structural corner
  of the type system is unions — see
  [`../05-types/01-type-system-overview.md`](../05-types/01-type-system-overview.md).)

## Construction by name and `*`-spread

Data-transformation and schema-evolution code spends a lot of effort copying
fields between near-identical types. Tel makes that terse with **construction by
local name** and a **spread**:

```tel
let x = 4
let y = 2
let p = Point { z = 7, * }      // x and y filled from locals of the same name

# Spreading another record copies same-named fields:
let old: User_v3 = ...
let favorite_color = none
let new = User_v4 {
    created_ns = created_ms * 1000,
    favorite_color,             // filled from the local
    *old,                       // every remaining same-named field from `old`
}
```

The settled rules:

- A bare field name with no `= value` is filled from an **in-scope local of the
  same name**.
- `*other` fills every still-unset field of the new record from a **same-named
  field of `other`**, when the types match.
- It is a **compile error if any field of the new record is left unset** — a
  missing local or a too-narrow source record fails loudly.
- Fields of `other` that the new record does not have are **ignored**.

This is what makes API-evolution code (User_v3 → User_v4) short: the codegen, or
the author, writes only the fields that genuinely change and lets `*old` carry
the rest. The spread direction is *into* the new type, so it composes across
*different* record types, not just copies of one.

TODO(open): a real hazard — combined with optional fields, a
*renamed* field becomes silently unmatched and falls back to its default or
absence value instead of failing. The "every field must be set" rule catches a
missing *required* field, but not an optional one that silently went absent.
Decide whether `*`-spread should warn when a source field is dropped, or when an
optional target field is filled only by default. This matters because silent
drift is exactly what API-evolution tooling exists to prevent.

TODO(open): a `*=Point{*}` form was floated for the reverse — destructuring every
field of a record into locals of the same name. Useful for codegen symmetry.
Confirm whether Tel includes it.

## No constructor keyword

Tel has **no constructor keyword** — there is no `new`, no `make`, no special
construction form. A record is built by naming its type directly and supplying
its fields, exactly as in the forms above:

```tel
let p = Point { x = 1, y = 2 }
```

A type name in value position *is* the constructor. This keeps construction
uniform with the rest of the language — refined types and units are built the
same way by naming the type (`kg(100)`, see [Units](../05-types/13-units.md)) —
so "name the type" is the single rule for making a value, with nothing extra to
learn or spell.

## Copy-update (`with`)

Since records are immutable, Tel provides a copy-update form: produce a new
record equal to an existing one except for named fields.

```tel
let p2 = p with { x = 10 }       // p2 is p with x changed; p is untouched
```

Copy-update is the **non-mutating** path, and it coexists with the in-place
mutation of `!T` records described next ([TIP-0001](../tips/0001-mutability-and-borrowing.md),
Accepted): `with` always produces a fresh value and is available on any record;
in-place mutation is the separate story of a `!T` value held through a `uniq`
binding. `with` on a `!T` value is allowed but still returns a fresh value — it
does not mutate in place.

## Owned records: `!T` and `mut` fields

A bare record type `T` is **shareable and immutable**. Its **owned (affine)
form is `!T`** — the prefix `!` sigil names *unique ownership*, not mutation
(see [mutability](../06-bindings-and-scope/02-mutability.md#two-axes-ownership-and-reassignability)
and [references and aliasing](../12-memory-and-runtime/04-references-and-aliasing.md)).
You **declare the owned form** `!T`; its frozen, shareable projection `T` is
**auto-derived** — same fields, all final, freely shared. The derivation runs
`!T → T`: give up ownership, gain shareability.

```tel
record !Person { mut age: Nat, name: Text }    # declare the owned form

let uniq b = !Person { name = "M", age = 1 }   # owned, unique binding
b.age = 2                                       # ok — owned, and age is `mut`
let p = b.finish()   # !Person -> Person : zero-copy, consuming, one-directional
```

- `finish()` is the auto-generated one-way freeze. A non-consuming
  `snapshot()` (clone-to-immutable that leaves the owned value live)
  **allocates**, so it is opt-in via an explicit derive, never automatic.
- When the owned form needs **extra state** the frozen one does not — a
  `capacity`, a scratch buffer — it lives on the `!T` declaration and the
  derived `T` drops it. This is the `List`/`!List` split: `!List` carries growth
  state, `List` is its freeze.

### `mut` fields: reassignable slots

Ownership (`!`) and reassignability (`mut`) are **separate axes**. Owning a `!T`
makes mutation *safe*; `mut` says a given slot is *meant* to change. A slot is
reassignable only when both hold. Two independent things, spelled differently:

- **Interior mutation** — calling a held value's mutating methods — comes from
  the **field's type** (`!List` yes, `List` no). No annotation.
- **Reassignment** — repointing the slot at a new value — is marked **`mut` on
  the field**, the opposite of Java's `final` (final by default). It is
  **shallow**: `mut the_xs: List` makes the slot repointable, not the `List`
  contents.

```tel
record !Counter { mut the_n: Int64 }       # owned; the_n reassignable
record !Log     { mut the_entries: List[Text] }   # slot repointable; List still immutable
```

A record carrying a `mut` field (or a `!`-typed field) is **affine**, so it
**must be declared `!`** — `record Counter { mut the_n }` is an error, fixed by
naming it `!Counter` (the bare `Counter` is then its derived freeze).
Reassigning a `mut` field, like any mutating method, needs `&!` (exclusive)
access to the containing value; see the precise derivation rule in
[substructural types](../12-memory-and-runtime/08-substructural-types.md#affine-and-the-alias-capability).

## Record-level invariants

A record can carry an **invariant**: a relationship between its fields that must
always hold. The invariant is stated once on the type and enforced at
construction (and at copy-update), rather than re-checked in every method that
touches the record. This is design-by-contract applied to data, and it is the
record-level form of the refined types in
[`../05-types/12-refined-types.md`](../05-types/12-refined-types.md).

```tel
struct DateRange {
    start: Date,
    end:   Date,
    invariant start <= end,
}
```

A `DateRange` cannot exist with `start > end`: the only ways to make one — the
constructor and `with` — check the invariant. Code that *has* a `DateRange` may
rely on it unconditionally.

TODO(open): exact spelling of invariants, and whether they are checked always or
only in debug builds — tied to the design-by-contract open question in
[`../02-philosophy/03-features.md`](../02-philosophy/03-features.md).

TODO(open): invariants must also hold for values built by deserializers and at
the host boundary, not just by the in-language constructor — see the same point
for refined types in
[`../05-types/12-refined-types.md`](../05-types/12-refined-types.md).

A record with an invariant is a prime candidate for **declared example
values and counter-examples**: a `DateRange` example `{ start, end }` with
`start <= end`, and a counter-example with `start > end` that construction
must reject. The property runner seeds from the examples and checks the
counter-examples are refused; see
[`../17-standard-library/19-testing-utilities.md`](../17-standard-library/19-testing-utilities.md#declared-example-values-and-counter-examples).

## Bugs the construction rules prevent

The "every field must be set explicitly or by spread" rule, combined with the
constraint that deserializers go through the same constructor (see
[`../05-types/12-refined-types.md`](../05-types/12-refined-types.md)), is a
deliberate response to a recurring family of catalogue bugs:

- **Mongo patch that wiped the database.** A migration script used the same
  Java class for *input* and *output* and corresponded to the output schema.
  Reading the older input schema produced an instance where the new fields
  silently took defaults; persisting that overwrote real data with defaults.
  Tel forces input and output records to be separate types when their
  shapes differ; an unrecognised field is a deserialization error, not a
  silent default.
- **"Empty-style object" missing fields dependent on configurable model
  parameters.** A "no-impact" sentinel object had fewer fields than the
  general type; in some configurations the missing fields were the load-
  bearing ones, causing a precondition violation. The record-level
  invariant — and the rule that *all* fields are set, not optional unless
  the type says so — pushes the modelling toward a sealed union of distinct
  record types per case, not one record with sometimes-absent fields.
- **De/ser default-not-transitively-applied.** A custom deserializer applied
  defaults at the top level but not for nested fields. Because Tel's
  construction rule is that every field is set — by spread, by local, or
  by explicit value — there is no "I'll just default this" escape hatch a
  deserializer can quietly take.
- **New required field not provided by a generated builder.** A class with a
  generated builder gained a new required field. Code in another repo
  still compiled (the builder API hadn't changed) but failed at runtime
  because the field was unset. Tel's "every field must be set" rule
  forbids the generated builder from compiling without it; the failure
  moves from a runtime exception in production to a compile-time error in
  the dependent repo.
- **Builder used after `unmodifiableList` removal during merge.** A `new
  HashMap(...)` defensive copy was deleted in a final revision; the
  merged code mutated an immutable map. Tel's transitive immutability
  rule (see [`../06-bindings-and-scope/02-mutability.md`](../06-bindings-and-scope/02-mutability.md))
  removes the implicit "this is mutable because Java doesn't say
  otherwise" assumption.

## Records and unions together

Some languages (Gleam) merge records and unions into one construct: a "union"
declares its variants inline, and a record is just the one-variant case.

```text
# Gleam-style — NOT how Tel does it.
type SchoolPerson {
    Teacher(name: Text, subject: Text)
    Student(name: Text)
}
```

Tel keeps the two **separate**: a `struct` is a record, a `type X = (A | B)` is a
union over independent types. The Gleam style is an interesting idea
but rejected for Tel — probably not wanted here. The reason it still works out
well: Tel's untagged unions take *existing*
record types as members, so you get the same Teacher/Student modelling by
declaring two records and a union over them — without the variants being trapped
inside one enum.

```tel
struct Teacher { name: Text, subject: Text }
struct Student { name: Text }
type SchoolPerson = (Teacher | Student)
```

The one feature the Gleam example shows that Tel keeps: a field present in every
variant with the same name and type — here `name` — is accessible on the union
directly. That is the shared-fields rule in
[`02-union-types.md`](02-union-types.md), and it is a general consequence of
untagged unions, not a special case of a combined record/union construct.

## Computed fields

When a record field and a method share a name, the *method* is
preferred — so an immutable field can later become a computed property without
breaking callers (`point.x` keeps working whether `x` is stored or computed).
This is a forward-compatibility convenience and is detailed with field access in
the syntax/methods chapters.

TODO(open): confirm field-vs-method precedence and how it interacts with
externally-defined functions used in method position.

## Records as dataframe rows

A `struct` is **nominal** (identity by name, above), but the **structural**
record form — the anonymous, compared-by-shape record written `{ a: A, b: B }`
(see [Tuples and Arrays](../05-types/04-tuples-and-arrays.md)) — is the carrier of
the [dataframe](../10a-dataframes/01-overview.md) feature. A `Table[R]` is one such
row shape over many rows, and the schema-changing operations (`extend`, join,
`group_by`/`agg`, `pivot`) **derive a fresh row type** the user never writes by
hand. That auto-derived schema is plain structural-record machinery: a `type`
alias names it and unifies by shape; a `newtype` is the explicit nominal cap. See
[The Record-Shape Calculus](../05-types/15-record-shape-calculus.md).

## See also

- [Dataframes](../10a-dataframes/01-overview.md) — the structural row type carried
  over many rows, and the record-shape calculus that transforms it.
- [Union Types](02-union-types.md) — records as union members.
- [Generic Data Types](04-generic-data-types.md) — records with type parameters.
- [Refined and Newtype Types](../05-types/12-refined-types.md) — single-field
  records and constrained wrappers.
- [Equality and Hashing](07-equality-and-hashing.md).
- [Entity Identity, Queries, and Projections](../19-use-cases/09-entity-identity-and-projections.md)
  — `Id[T]`, the entity-vs-query-result split, and the projection problem.

TODO: review
