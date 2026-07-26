# Data Formats and Serialization

<!-- TODO: review -->

## What

`std` provides parsing and serialization for common data formats — JSON
foremost — so a script can read and produce structured data without a
third-party crate.

This topic is a stub: data formats are touched on only lightly so far. The
points below are what is settled.

## Schema-first, code-generated

Tel does not provide a reflection- or macro-driven serializer in the style of
Java annotation processors or Rust's `serde`. **Datamodels are schema-first**:
the data model is described, and the
data classes are *code-generated as a separate module*. This keeps
serialization out of the language's metaprogramming surface — Tel has
deliberately little of that
([`../02-philosophy/04-antifeatures.md`](../02-philosophy/04-antifeatures.md))
— and keeps generated code navigable, in its own files rather than woven into
hand-written types.

`TODO(open): the schema-first/codegen workflow is a tooling-and-language story
that spans more than this topic — where the schema language lives, how
codegen is invoked, how generated modules are kept in sync. Coordinate with
the build/tooling chapters; this topic only commits to "not reflection, not
serde-style macros".`

## A reusable data-format sublanguage

The data-literal / data-model notation could be a
**separate parser / sublanguage**, reusable on its own — for example to embed
data snippets inside other host formats (YAML, etc.) with little escaping.
This lines up with Tel's DSL-friendliness and with the original Apivolve use
case (describing schema evolutions). `TODO(open): whether the data-format
notation is a distinct sublanguage or just a subset of Tel's expression
syntax is unresolved.`

## Binary formats

The candidate binary format is **CBOR + CDDL** — plain CBOR (RFC 8949)
covers the self-describing case, and CDDL (RFC 8610) adds schema-awareness
when present. Leaning toward binary-with-names-not-reserved (so schema
field renames don't break wire compatibility) over name-as-tag formats.
See the "Concrete format coverage" section below for the open question.

`TODO(open): the binary format is not yet committed. Revisit when a
real use case forces the call; if shipped, exactly one binary format goes
in, per "one good way".`

## Multi-format façade vs single parsers

The library exposes two distinct surfaces:

- **Per-format parsers/serialisers** — `json`, `csv` (and the binary
  equivalents if a binary format is added) — each with its own functions,
  error types, and options. A script that *knows* it deals with JSON
  imports `json` and uses it directly.
- **A multi-format façade** — a `serde`-shaped interface where the
  format is a parameter and the *value shape* is the user's data type.
  This is the form that lets a script accept several formats from the
  same code path without conditionals.

`TODO(open): this is unresolved. The schema-first stance above pushes
against `serde`-style auto-derivation; the multi-format façade pushes
for it. The likely resolution: the façade *consumes* code generated from
schemas, rather than synthesising the mapping from runtime types. Confirm
once the schema-codegen workflow is designed.`

## Concrete format coverage

The library aims to cover the formats embedded scripts actually meet — no
more, no less. The committed list:

- **JSON** — parser, serialiser, streaming reader for large documents.
- **CSV / TSV** — streaming reader and writer, with a documented
  quoting/escaping policy.
- **A `jq`-style stream-of-JSON pipeline** — leveraging the stream
  adapters in
  [`05-iteration-and-streams.md`](05-iteration-and-streams.md).

Formats explicitly **out of scope** for `std`:

- **XML** — outdated for 2026 workloads; crate-ecosystem concern.
- **TOML, YAML** — config-shaped formats; if a script needs them, a
  crate supplies them. JSON covers the in-`std` case.
- **Native (non-portable) object serialization** (`pickle`, `gob`, Java
  `Serializable`) — antifeature: version-fragile, language-locked,
  attack surface.
- **Archive formats (zip / tar)** — not common enough in 2026 to belong
  in core.
- **Domain-specific encodings** (Avro, Parquet, Arrow), image formats,
  encryption envelopes — all live in the crate ecosystem.

`TODO(open): a single **portable structured binary** format. Leading
candidate: CBOR (RFC 8949) paired with CDDL (RFC 8610) for schema. JSON
covers the schemaless case; binary's whole point is schema-aware, so the
preferred mode is schema-known. Decision deferred; if shipped, exactly one
binary format goes in.`

## SemVer

`std` provides parsing, ordering, and range matching for
[Semantic Versioning 2.0.0](https://semver.org/) strings. Tel's package
manager (see [`../11-modules-and-packages/06-versioning.md`](../11-modules-and-packages/06-versioning.md))
uses it, and it is small and stable enough to belong in `std` rather than
a crate.

```tel
let v = SemVer.parse("1.4.0-rc.2+build.5")?
v.major == 1 and v.minor == 4 and v.patch == 0
v.is_prerelease()
v < SemVer.parse("1.4.0")?               # prerelease < release
let req = SemVer.range("^1.4")?
req.matches(v)
```

Pure data; no capability needed.

`TODO(open): which range syntaxes to support — npm-style `^`/`~`, Cargo
ranges, Maven brackets. Lean: a single canonical form, with the others
available as named parsers a script opts into.`

## Schema evolution

**Schema evolution** is a first-class concern, not something each format
reinvents. The shape:

- A data model can be *versioned*; an older binary on disk reads into a
  newer code shape (added fields default, removed fields ignored,
  renamed fields aliased).
- The same machinery serves database migrations, CLI flag changes, and
  network message versioning. The library exposes one *evolution
  algebra* (add field with default, remove field, rename field, widen
  type, narrow type with explicit migration), and each format binds it.
- A normal data class can be *promoted* to an evolving one without a
  rewrite — the migration history is added alongside, not woven into
  every field.

`TODO(open): schema evolution overlaps with the original Apivolve use
case ([`../01-overview/02-when-to-use-tel.md`](../01-overview/02-when-to-use-tel.md)).
Decide whether the evolution machinery lives in `std`, in a separate
sub-format (Tel-data), or in tooling. Lean: stdlib API plus a tool that
applies it.`

## Compile-time escaping and stability of generated code

Generated serialisation modules are *code*, not configuration — they live
in source control, they show up in diffs, and a schema change becomes a
visible code change. The generated module's public surface is held to the
same stability rules as hand-written code: a field rename is a breaking
change, surfaced by the API-summary file
([`../02-philosophy/03-features.md`](../02-philosophy/03-features.md))
rather than discovered in production.

## Caching of serialised values

The library exposes a small **persistent cache** facility: store a
serialisable value between runs, keyed by user, version, or context, with
an age limit and a size limit. The cache backend is a capability the host
grants (filesystem, browser local storage, an in-memory store for tests);
the cache *interface* is the same across hosts.

`TODO(open): "persist between runs" is one half of a bigger story (the
other half is hot reload of a script's own state). Decide whether this
is `std` or a separate library. Lean: stdlib *interface*, host-supplied
backend.`

## Diffing

Swift's standard library has diffing as a first-class operation, and it is
useful enough to include here. `std` exposes a generic
`diff(a, b)` producing an ordered list of edit operations, plus
specialised diff for text (line- and word-level) and for ordered
collections. `TODO(open): the algorithm choice (Myers vs patience vs
Hunt-McIlroy) is implementation detail; user-facing API is the
question. Decide whether diff lives here or in a small "data
algorithms" topic.`

## Bugs the schema-first stance prevents

A few recurring catalogue cases that
push the design toward distinct input/output types, schema-driven codegen,
and rejecting silent defaults:

- **"One Java class for input and output schemas."** A Mongo patch read
  data into a class that matched the *output* schema; missing fields
  silently became defaults; the patch then wrote the defaults back.
  Real data was lost. Tel-side fix: input and output are distinct
  generated record types when the schema differs; unknown fields are an
  error by default, not silently skipped.
- **"Deserializer silently skipped fields with no type info."** Took a
  long time to diagnose. The default is to *warn or fail*, never to
  drop silently.
- **"Old JSON failed because a field stopped being nullable."** A change
  to an Avro schema made a previously-optional field required; older
  rows no longer parsed. The schema-evolution algebra above is meant to
  make these changes explicit: a field that *was* optional and is
  becoming required must declare a default for older inputs or refuse
  the older shape with a clear error.
- **"Enum variant added in a config GUI, older GUIs couldn't deserialize
  it."** New variant on a non-extensible enum broke older clients. The
  Tel answer is non-exhaustive unions
  ([`../10-data-modelling/02-union-types.md`](../10-data-modelling/02-union-types.md))
  for cases that may grow, and the rule that schema changes carry a
  declared compatibility direction.
- **"Persisted data outlasted the field that produced it."** A revert
  removed an enum variant from code but it was still present in
  persisted data; the data could no longer be read. The evolution
  algebra needs an explicit *unrecognised variant* policy at read
  time — usually "preserve as opaque" or "fail loudly", never
  "silently coerce."
- **"Avro fingerprint hashes collide; outer generics lost."** An external
  serialization library produced the same fingerprint for distinct
  generic shapes. Tel's schema-first stance keeps the schema explicit
  and verifiable — there is no fingerprinting heuristic implicit in the
  type system that can shake itself loose.
- **"Avro buffer shared between messages."** A library exposed an
  underlying buffer that contained several messages; deserializing the
  first moved the read pointer past the second. Tel's stdlib parsers
  do not expose mid-stream buffer state to callers; a streaming reader
  is the *interface*, not a thin wrapper over a byte buffer.
- **"Map with no setter broke JSON but not Avro."** A datamodel field
  was changed to use an immutable list with no setter; one format
  worked, the other failed. With Tel's immutable-by-default and
  schema-first codegen, the deserializer is generated from the same
  schema as the data class — the deserializer cannot drift out of
  sync with the field's mutability.

## See also

- [Standard Library Organisation](01-stdlib-organisation.md)
- [Strings and Text](06-strings-and-text.md) — escaping helpers
- [JSON Schema Validator](../19-use-cases/02-json-schema-validator.md)
- [Networking](11-networking.md)
- [Tel-as-data](18-tel-as-data.md) — typed representation of Tel source
- [Data Access and ORMs](20-data-access-and-orms.md) — the same schema-first,
  code-generated record stance applied to database access
