# Data Access and ORMs

<!-- TODO: review -->

## What

Tel does **not** ship an ORM, a query DSL, a connection pool, or any other
data-access framework in `std`. The host owns the database — its schema, its
connection lifecycle, its migration story — and exposes whatever the script
is allowed to do as a *capability*
([`../02-philosophy/03-features.md`](../02-philosophy/03-features.md)). What
`std` does provide is the underlying *language and library* support that a
third-party data-access library can stand on if a host wants one.

This topic explains the boundary: why no shipped ORM, and which language
features a third-party one would lean on.

## Why no shipped ORM

A Diesel- or JPA-style ORM as part of `std` was considered, and the
answer the priorities settle on is *no*. Several of Tel's commitments push
in the same direction:

- **One good way over many clever ones.** A general-purpose ORM is a
  framework — it shapes how every record is declared, how queries are
  written, how transactions are scoped. That is bigger than a *library* and
  smaller than a *language*, and it puts the standard library in the
  position of dictating data modelling for every script.
- **The host owns deployment.** Tel does not own the build, the process,
  the connection pool, or the migration tool
  ([`../02-philosophy/04-antifeatures.md`](../02-philosophy/04-antifeatures.md)).
  An ORM that does any of those usefully has to *also* own them, which
  conflicts with the embedding model. A backend host already has its ORM;
  a browser host has IndexedDB; an in-memory test host has a fixture map.
  One bundled ORM is unlikely to be the right answer for any of them.
- **Schema is rarely the script's to dictate.**
  Developers usually do *not* own the database schema — DBAs do, other
  programs share it, or it is defined by an external service. A library
  that insists on declaring the schema in script code is at war with the
  reality of the systems it has to talk to.
- **Stability.** A framework large enough to be useful is hard to freeze.
  Tel's stability commitment ([`../02-philosophy/01-priorities.md`](../02-philosophy/01-priorities.md))
  makes the cost of a half-right ORM in `std` very high — the next breaking
  change would have to be a whole new language.

The antifeatures chapter already lists ORMs as the kind of thing that
belongs in a library, not `std`: see the open question in
[`../02-philosophy/04-antifeatures.md`](../02-philosophy/04-antifeatures.md)
("a web framework, ORM, or REST client clearly belong in libraries, not
the language"). This topic confirms that lean.

## What Tel provides instead

A third-party data-access library — written in Tel or written in the host
and exposed across the boundary — has the following ingredients to work
with. Each is documented elsewhere; the point here is that nothing extra is
needed.

### Capabilities for the connection

The host gives the script a `Store` (or `Connection`, `Db`, whatever the
library names it) the same way it gives a `Clock` or a `File`. The script
cannot reach out for one on its own — there is no ambient connection pool,
no `Db.default()`. See
[`08-io-and-filesystem.md`](08-io-and-filesystem.md) for the capability
mechanics; the database one is the same shape.

```tel
fn run(an_orders_store: OrderStore) -> Result[Report, DbError] {
    an_orders_store.select_pending().map(make_report)
}
```

### Phantom-typed identifiers

`Id[User]`, `Id[Order]` — distinct types, both wrapping the same primitive,
incomparable with each other, and *not* implicitly convertible to text. See
[`../19-use-cases/09-entity-identity-and-projections.md`](../19-use-cases/09-entity-identity-and-projections.md).
A typed library reuses this pattern; nothing in `std` needs to know what an
ID is.

### Schema-first, generated record types

Per [`13-data-formats.md`](13-data-formats.md), the recommended workflow for
schemas the script *receives from outside* is to generate Tel record types
from the schema, not to declare them by hand and hope they stay in sync.
The same machinery works for database tables: a build-time tool reads the
schema, emits one record type per table (the *entity* type), and the
generated module is committed alongside the script.

This is exactly the *schema-in-two-places* problem at the heart of the
ORM antipattern: declaring the schema in code and *also* in the database
forces two sources of truth. Tel's answer is to put it in *one* place — the
database (or the external schema file) — and *derive* the code.

The Tel-as-data AST surface in
[`18-tel-as-data.md`](18-tel-as-data.md) is what a schema-to-records
generator emits. It does **not** belong inside `std` for any specific
schema language; that is a tool / crate concern.

### Projection types

Joins and projections produce record types that are not the same as any
entity type. The third-party library either:

- generates a named record per query result (recommended, fits the
  schema-first stance), or
- returns a tuple, with field access by position (terse, less readable).

See the
[entity-identity-and-projections topic](../19-use-cases/09-entity-identity-and-projections.md)
for the trade-offs and the open language-design question (whether Tel ever
grows *structural / anonymous* records to make this nicer — the lean is
**no**, generated named records are the obvious one good way).

### A clear entity / query-result split

A library that wants to distinguish "this record is a row I can write back"
from "this record is a query result, read-only" can express that with a
trait — `Entity`, `Persistable`, whatever it picks — implemented only by
the generated entity records. Nothing in `std` needs to formalise the
category; it falls out of ordinary trait dispatch
([`../10-data-modelling/03-traits-or-interfaces.md`](../10-data-modelling/03-traits-or-interfaces.md)).

## What does *not* survive

Some ORM-adjacent problems are real but do **not** translate
into Tel language or library features:

- **When to load FK data, lazy vs eager loading, identity maps, dirty
  tracking, unit-of-work** — these are *library design choices* for whatever
  data-access framework the host wires up. They are not the language's to
  pick.
- **Caching of database results, multi-node cache invalidation** — these are
  *host operational concerns*. The persistent-cache capability sketched in
  [`13-data-formats.md`](13-data-formats.md) is the script-side surface a
  library would build on; the cross-node story is a host problem.
- **Inheritance for table mapping** — Tel has no class inheritance
  ([`../02-philosophy/04-antifeatures.md`](../02-philosophy/04-antifeatures.md));
  table-per-class / single-table-inheritance mapping problems do not arise.
  Variant rows are modelled as a union over distinct record types, the
  ordinary way.

TODO(open): compile-time WHERE-clause analysis (the
library inspects a query expression and adds indices for it, or rejects
unsafe ones). This is the kind of thing a typed query DSL could do *if*
Tel exposed enough at compile time. Tel deliberately keeps metaprogramming
narrow (see [`../02-philosophy/04-antifeatures.md`](../02-philosophy/04-antifeatures.md));
between `derive`-style attributes and the Tel-as-data AST, a library has
enough to emit type-safe query builders, but cannot rewrite or inspect
user-written query expressions at compile time the way LINQ does in C#.
This is a *real limitation* on what a Tel-native ORM can do, and it should
be called out wherever the metaprogramming line is finalised.

TODO(open): runtime-statistics-driven indexing (the library learns from
which queries actually run and adjusts indices) is a host-side concern,
not a language one — the script does not own the database. Note for
completeness; nothing to design.

TODO(open): some hosts will reasonably want to expose their existing ORM
*as* the capability (e.g. a JVM host exposing JPA, a Rust host exposing
Diesel) rather than have a Tel-side library reimplement one. The bridge
shape — turning a host ORM's entity type into a Tel record type with
matching `Id[T]` and entity/projection split — is a host-binding-API
question and probably belongs in a future *host integration* chapter.
Flag here for now.

## What about Apivolve?

The original use case for Tel was Apivolve, a schema-evolution tool
([`../01-overview/01-introduction.md`](../01-overview/01-introduction.md)).
Schema evolution overlaps the database/ORM space — both are about *types
of data over time* — but it is a *separate* concern: Apivolve describes
how a schema *changes*, not how a runtime program *queries* it. The
evolution machinery already has a home in
[`13-data-formats.md`](13-data-formats.md). A future data-access library
would *consume* Apivolve-style schemas; it would not duplicate them.

## See also

- [Entity Identity, Queries, and Projections](../19-use-cases/09-entity-identity-and-projections.md)
  — `Id[T]`, entity vs query-result, projection types.
- [Data Formats and Serialization](13-data-formats.md) — schema-first
  code generation, persistent cache, schema evolution.
- [Tel-as-data](18-tel-as-data.md) — the AST surface a typed query library
  would emit projection records into.
- [I/O and Filesystem](08-io-and-filesystem.md) — the capability model the
  database connection follows.
- [Antifeatures](../02-philosophy/04-antifeatures.md) — the open question on
  bundled tooling, where the no-shipped-ORM stance is recorded.
