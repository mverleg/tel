# Entity Identity, Queries, and Projections

A small but recurring data-modelling problem: code that pulls structured data
out of an *external store* (a database the host owns, a remote service, a
spreadsheet) tends to mix two very different kinds of value:

- **Entities** — rows of a known table. Each has a stable identity (`Id[User]`,
  `Id[Order]`), can be re-fetched, and is the unit a store typically allows
  `insert` / `update` / `delete` on.
- **Query results** — whatever shape a *projection*, *join*, or *aggregate*
  yields. These have no inherent identity, may borrow fields from several
  entities, and are *read-only* — there is nothing to write them back to.

Tel does not ship a data-access framework (see
[`../17-standard-library/20-data-access-and-orms.md`](../17-standard-library/20-data-access-and-orms.md)
for why). This use case is not a language feature of its own — it is a worked
example of *applying* features covered earlier (phantom-typed IDs, nominal
records, untagged unions, copy-update) to a realistic problem. It shows how a
third-party library would model the entity/projection distinction safely, and
why Tel is a good fit for it.

## What — three patterns

### 1. Phantom-typed identifiers

A naked integer ID is a hazard. Two unrelated entities both use `Int64`-shaped
keys, and a stray copy-paste swaps an `Id[Order]` for an `Id[User]` with no
type error. Tel's answer is **phantom-typed IDs**: a wrapper type with a
type parameter that names what the ID identifies, even though the parameter
contributes nothing to the runtime representation.

```tel
# Sketch — see refined types for the precise machinery.
type Id[T] = newtype Int64

struct User  { id: Id[User],  name: Text }
struct Order { id: Id[Order], total: EuroAmt, owner: Id[User] }

# Compile error: Id[User] and Id[Order] are not the same type, even though
# both wrap Int64.
if some_user.id == some_order.id { ... }
```

The phantom parameter is just a name carried by the type. Tel already has
the pieces: refined / newtype wrappers
([`../05-types/12-refined-types.md`](../05-types/12-refined-types.md)) plus
generics ([`04-generic-data-types.md`](../10-data-modelling/04-generic-data-types.md)). `Id[T]`
is the canonical example both pages point at.

### 2. IDs are not display values

Two further disciplines on `Id[T]`:

- **No implicit conversion to text.** An ID is not a label, and printing it
  into a user-facing string is almost always a bug. The type does *not*
  implement the text-conversion trait by default; a script that needs to log
  or render an ID asks for it explicitly (`an_id.to_text()`,
  `Debug.format(an_id)`).
- **Equality and hashing only with the same phantom.** Two `Id[User]`s
  compare; an `Id[User]` and an `Id[Order]` do not — and a `Map[Id[User], _]`
  cannot accidentally be looked up with an `Id[Order]` key.

This is exactly the pattern the
[priorities](../02-philosophy/01-priorities.md) point at under *invalid
states should be unrepresentable*: a category mistake becomes a compile
error, not a midnight production alert.

TODO(open): default-trait policy. `Id[T]` deliberately should *not* get a free
`toString` / display impl, but Tel's wider stance on default implementations
for newtypes is still being settled
([`../17-standard-library/01-stdlib-organisation.md`](../17-standard-library/01-stdlib-organisation.md)
— "no default `equals`/`hashCode`/`toString` unless obvious"). Confirm that
the *anything a built-in can do, a wrapper can do too* promise in
[`../05-types/12-refined-types.md`](../05-types/12-refined-types.md) does
**not** mean "automatically get the same default trait impls" — the wrapper
inherits the *ability*, not the *defaults*.

### 3. Entities vs query results

If a library exposes a typed store, the *type* of a value should encode
whether it can be written back:

- An entity type (`User`, `Order`) is what `insert` / `update` / `delete`
  operate on. It has an `Id[Self]`.
- A query-result type is what `select` / `project` / `join` produce. It may
  share *fields* with one or more entities, but the type is distinct — a
  `UserWithRecentOrderTotal` is not a `User`, and the store will not let it
  be written.

The user does not need a special language feature for this — two ordinary
record types and a function that returns the right one is enough. The point
is that conflating them *is* a frequent design error: ORMs
that treat every row as an entity force write methods onto rows that have no
meaningful "write" semantics.

```tel
# An entity — has an Id[Self], supports CRUD via the store capability.
struct Order { id: Id[Order], total: EuroAmt, owner: Id[User] }

# A query result — a different record type, no Id[Self], read-only.
struct OrderSummary {
    owner_name:   Text,
    order_count:  Int64,
    grand_total:  EuroAmt,
}
```

A library that wants to make this watertight can layer it on the type system:
make the write-side methods part of a trait every entity record implements,
and simply not implement the trait for query-result records. Nothing in the
language has to know that "entity" is a category — the trait carries it.

## Why — the *projection problem*

The hardest part of any typed store is what to do when a query yields a
shape the user did not declare ahead of time. A `SELECT name, total FROM
orders JOIN users ...` produces a record with *some* fields from `users` and
*some* from `orders` — a new type, used once.

Rust struggles here for a telling reason: joins and
projections give rise to **new record types**, and the only way to support
this naturally is **anonymous records in the host language**. Tel's current
design has nominal records (every record has a declared name —
[`01-records.md`](../10-data-modelling/01-records.md)). That is by deliberate choice — *familiar,
readable, one good way* — but it is also exactly what the projection problem
needs the language to relax.

Three plausible answers, in increasing order of language change:

1. **Generate a named record per query.** The library / build pipeline emits a
   real `struct OrderWithUserName { … }` and the query call site uses it.
   This fits Tel's *schema-first, code-generated* stance for data-formats
   ([`../17-standard-library/13-data-formats.md`](../17-standard-library/13-data-formats.md))
   and reuses the Tel-as-data AST surface
   ([`../17-standard-library/18-tel-as-data.md`](../17-standard-library/18-tel-as-data.md)).
   It is the most boring answer, and probably the right one for a *frozen*
   language: every query result has a real, navigable name.
2. **Tuple-shaped results.** A projection of `name, total` is a
   `(Text, EuroAmt)` tuple, accessed positionally. Cheap, no codegen, but
   loses field names at the call site — a regression in readability.
3. **Anonymous records as a language feature.** A `{ name: Text, total:
   EuroAmt }` type that has no declaration anywhere — *structural* records.
   Powerful, but pulls Tel toward a structural type system everywhere it
   currently is nominal, and breaks the "you can find every type by name"
   discipline.

TODO(open): which of (1), (2), (3) Tel commits to for query-style projection.
Per the [priorities](../02-philosophy/01-priorities.md), (1) is the obvious
default — *one good way over many clever ones*, lines up with schema-first
codegen, no new type-system machinery. But without *some* answer here,
a typed query DSL is impossible to build as a
third-party library. Confirm (1), or escalate to a structural-record
proposal in [`../05-types/`](../05-types/).

TODO(open): tuple-projections (option 2) compose poorly with the
nominal-record bias of the rest of the language; the case for them is brief
and one-off scripts. Decide whether they are an actively supported pattern
or merely an emergent consequence of the tuple types in
[`../05-types/04-tuples-and-arrays.md`](../05-types/04-tuples-and-arrays.md).

## Subset-of-fields projection

A related but smaller problem: a query that returns *a subset of one
entity's fields* — `SELECT id, name FROM users` rather than the whole row.
The desired type is "a `User` with only `id` and `name` known".

Tel's existing tools cover this without new machinery:

- **Declare a small record** for the projection (option (1) above).
- **Use `Option<…>` for absent fields** if the projection genuinely shares
  the entity shape but leaves some fields unknown — though this is usually
  the wrong choice because `Option.None` and "not selected" are different
  states.
- **The `*`-spread** in record construction
  ([`01-records.md`](../10-data-modelling/01-records.md)) makes building a projection record
  from an entity terse — `UserSummary { *a_user }` fills every same-named
  field, and the compile error on missing fields catches drift.

TODO(open): a fourth option for ORMs — projection
as a *subtype* of the entity: `User[id, name]` is a value of `User` that
the type system knows only has `id` and `name` populated. This is a real
feature in some languages (TypeScript's `Pick[T, …]`) and ties to the
anonymous-records question above. Lean: defer — declare the small record
for now.

## Cross-entity references — Id, not pointer

A record that refers to another entity holds an `Id[Other]`, not a *pointer*
to the other entity's record. This avoids three traps:

- **Bidirectional foreign keys.** A `User` having `orders: List[Order]` and
  every `Order` having `owner: User` is a structure no plain value type can
  represent without a cycle, and a cycle is a problem the host has to solve
  (GC, weak refs, arenas) on its own. Storing `Id[User]` on the order and
  `Id[Order]` on the user — or just one direction — keeps every value a
  tree.
- **When to load the referenced data.** A value-typed reference forces the
  store to *always* load the other end; an `Id[Other]` lets the script ask
  for it explicitly. Lazy / eager loading becomes a method call, not a
  feature of the type.
- **Equality of references.** `order.owner == user.id` compares two
  `Id[User]` values — well-typed, cheap, no walk into the referenced record.

TODO(open): cycle-bearing in-memory shapes (a tree with parent pointers, a
graph of mutually-referencing entities) do come up. The relevant tools live
in [`05-recursive-types.md`](../10-data-modelling/05-recursive-types.md) and the arena story in
[`../17-standard-library/04-core-collections.md`](../17-standard-library/04-core-collections.md);
they are deliberately *not* the default for data pulled from an external
store.

## Bugs the entity/projection split prevents

A few concrete catalogue cases that
drive these patterns:

- **"Same ID type used for different entities."** A change relied on
  `(source, expiry)` being unique; in some setups duplicate `null`
  expiries appeared and the assumption silently broke. With phantom-typed
  IDs and refined-collection uniqueness, the uniqueness invariant lives
  in the type, not in "the data happens to be unique today."
- **"Portfolio merge using `HashMap` on a class without `equals`."**
  Reference-identity for entities sharing identity-by-value silently
  double-counted. With `Id[Portfolio]` keys and derived equality, the
  merge is structurally correct.
- **"Portfolio tree: subscribing A and B (B ⊂ A) subscribed B twice."**
  A recursive subscription walked types without de-duplicating by ID.
  With a phantom-typed ID, the dedup is by `Id[Portfolio]` — a value
  comparison rather than a type-relationship walk.
- **"Round-trip changed the transferId because the original wasn't passed
  through."** A "round-trip tracker" failed because an intermediate API
  rewrote the ID on the way through. Phantom-typed IDs make "we
  preserved the same ID across this boundary" something the type system
  can express.
- **"`UserBasic + free UserAuth?` partial-load pattern."** Different
  callers wanted different subsets of a `User`. Tel's answer is the
  projection type: a `UserBasic` is a different record from `User`, with
  the fields the caller actually needs — not a `User` with maybe-absent
  fields the caller has to guess about.

## How it looks — putting the pieces together

```tel
# An entity type and its identifier kind.
type Id[T] = newtype Int64

struct User {
    id:    Id[User],
    name:  Text,
    email: Text,
}

struct Order {
    id:    Id[Order],
    owner: Id[User],
    total: EuroAmt,
}

# A query-result type — distinct from both entities.
struct UserOrderRollup {
    user_id:     Id[User],
    user_name:   Text,
    order_count: Int64,
    grand_total: EuroAmt,
}

# A hypothetical third-party store capability the host hands in.
fn report(a_store: OrderStore) -> List[UserOrderRollup] {
    a_store.query(
        # The store's query DSL builds a typed result. It cannot return a
        # `User` from a projection that drops fields — the projection record
        # is what the user asked for.
        select_rollup_by_user()
    )
}
```

The store is a *capability* the host provides
([`../02-philosophy/03-features.md`](../02-philosophy/03-features.md)), not
ambient access. Tel does not own the connection, the schema, or the
migration story — those are the host's
([`../17-standard-library/20-data-access-and-orms.md`](../17-standard-library/20-data-access-and-orms.md)).

## See also

- [Records](../10-data-modelling/01-records.md) — the nominal record story, `*`-spread, copy-update.
- [Generic Data Types](../10-data-modelling/04-generic-data-types.md) — `Id[T]` is the canonical
  phantom-parameterised wrapper.
- [Refined and Newtype Types](../05-types/12-refined-types.md) — the
  newtype mechanism `Id[T]` is built on.
- [Equality and Hashing](../10-data-modelling/07-equality-and-hashing.md) — why `Id[User]` and
  `Id[Order]` are incomparable.
- [Data Access and ORMs](../17-standard-library/20-data-access-and-orms.md) —
  why no shipped ORM, and what a third-party one would build on.
- [Tel-as-data](../17-standard-library/18-tel-as-data.md) — the codegen
  surface a typed query library would emit projection records into.

TODO: review
