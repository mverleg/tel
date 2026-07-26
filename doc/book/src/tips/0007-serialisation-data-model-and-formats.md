# TIP-0007: A built-in serialisation data model with library-provided formats

**Status:** Draft
**Created:** 2026-06-07
**Revised:** 2026-06-27 — renamed the bridge traits to `Serialize`/`Deserialize`;
recast L1 so the *sink* (visitor) is definitional and `Data` is its reference
sink; added a **static shape** surface (typestate sink + compile-time format
compatibility). This revision deliberately keeps **both** the static-typed and
the dynamic/materialised surfaces in scope — see *Scope: keep both for now*.
**Touches:** `17-standard-library/13-data-formats.md` (resolves the *multi-format
façade vs single parsers* open question), `17-standard-library/18-tel-as-data.md`
(the codegen path that produces per-type mappings),
`15-metaprogramming/01-macros.md` and `03-derive-and-attributes.md` (the
"no serde-style derive" stance and the frozen derivable set),
`10-data-modelling/02-union-types.md` (how an untagged union is tagged on the
wire), `17-standard-library/20-data-access-and-orms.md`,
`02-philosophy/04-antifeatures.md` (no reflection, no native serialisation),
`0001-mutability-and-borrowing.md` (the affine `!` types the typestate sink uses).
**Downstream of:** the *multi-format façade vs single parsers* `TODO(open)` in
[`13-data-formats.md`](../17-standard-library/13-data-formats.md#multi-format-fa%C3%A7ade-vs-single-parsers),
and row #2 of
[`inputs/metaprogramming-use-cases.md`](../../../inputs/metaprogramming-use-cases.md).

## Summary

Rust's `serde` owes its success to **one decoupling**: the type being
(de)serialised never names a format, and the format never names a user type.
Between them sits a fixed, universal **data model**, and two traits
(`Serialize`/`Deserialize`) that bridge user types to it. That architecture is
genuinely good and worth having in Tel.

What Tel rejected from serde is **only its *derive*** — a procedural macro that
runs *before* type information exists, which is why serde cannot emit a JSON
schema from the same declaration that drives serialisation, and why it lives in
the metaprogramming surface Tel keeps empty (see
[`01-macros.md`](../15-metaprogramming/01-macros.md)).

This TIP proposes keeping serde's good half and replacing its bad half:

- **Build into the language/stdlib the *data model* and the two bridge traits**
  (`Serialize` / `Deserialize`) — the user-facing "API to convert a value to the
  model, and a mapping back to Tel types." This is the façade the data-formats
  chapter left open.
- **Let crates implement *formats*** by implementing one `Format` trait over
  the data model (`Data ↔ bytes`). JSON ships in `std`; CSV in `std`; CBOR,
  MessagePack, a bespoke wire format, etc. are ordinary crates. A format author
  never sees a user type; a user type never names a format.
- **Produce the per-type mapping without a proc-macro.** The structural 99% case
  is a fixed `derive(Serialize, Deserialize)` (a compiler builtin against the
  declared shape, like `derive(Eq)`); anything divergent (renames, input≠output)
  is a **hand-written `impl`**. Schema evolution is out of scope — defer to a
  schema-first tool such as Apivolve. None of these is a macro or reflection.

The result is serde's ergonomics and its format ecosystem, with Tel's
schema-first guarantees and zero metaprogramming surface.

## Naming

The bridge traits are **`Serialize` / `Deserialize`** (method `serialize` /
`deserialize`). Two alternatives were considered and rejected:

- **`ToData` / `FromData`** — layer-honest (L2 only ever touches the `Data`
  model, never bytes) but it forces every user to notice a decoupling they do
  not care about, and `Data` is a vague name. The layering is better carried by
  the *separate* `Format` trait than by an awkward trait name on L2 — which is
  exactly how serde keeps `Serialize` (L2) and `Serializer` (L3) clean.
- **`Encode` / `Decode`** — *rejected because it collides*: `encode`/`decode` are
  already the **byte-boundary** verbs on `Format` and on the façade functions. An
  L2 trait named `Encode` would make "map my type to the model" and "turn the
  model into bytes" the same word for two different jobs — the precise conflation
  the three-layer split exists to prevent.

The middle model type is named `Data`. (`Repr` was floated; cosmetic — the trait
names no longer lean on it, so `Data` is fine.)

## Scope: keep both surfaces for now

This TIP intentionally specifies **two surfaces at once**, and the decision to
keep both is *provisional*:

1. a **dynamic / materialised** surface — `value` ⇄ a `Data` value — that is
   simple to author and debug; and
2. a **static-typed** surface — a typestate *sink* whose type carries the exact
   primitives a type emits (`Shape`), enabling compile-time format-compatibility
   checks.

The second is more powerful (it handles unsized collections, streaming, large
documents, and turns "this format can't represent this type" into a *compile*
error) but heavier. Rather than guess, **build both, then evaluate usability and
cut back** if the static surface does not earn its complexity. The two are not
rivals — see the next section: the materialised surface is a *special case* of
the sink surface, so carrying both is mostly free in the simple path.
`TODO(open):` after a usability pass, decide whether the static `Shape` surface
stays, is narrowed (e.g. encode-only), or is dropped to materialised-only.

## Core tenets

Five decisions fix the design; the rest follows.

1. **Cross-format via a fixed set of serializable primitives.** A small,
   format-agnostic data model (`Data`) sits in the middle: user **types map
   to/from the primitives**, and **formats map the primitives to/from bytes**.
   The decoupling of Rust serde and Swift `Codable` — a new type works with
   every format, a new format with every type.
2. **The sink (visitor) is definitional; `Data` is its reference sink.** A
   `Serialize` impl *drives a sink*; materialising a `Data` value is just driving
   the one sink that allocates a tree, and any `Data` tree can be *replayed* into
   any other sink. So the simple "build a `Data`" path is a **special case** of
   the streaming path, not a parallel API (see L1).
3. **Codegen, never reflection.** Every mapping is ordinary, readable,
   ahead-of-time Tel source. Runtime type introspection stays an antifeature
   ([`02-reflection.md`](../15-metaprogramming/02-reflection.md)).
4. **The type↔primitives mapping is a compiler-supported `derive`, or
   hand-written.** `derive(Serialize, Deserialize)` gets the same special
   compiler support the other builtin derives get (`Eq`, `Hash`): a
   *parameterless* mapping against the declared shape — not a proc-macro. When
   the structural default does not fit, you **write the trait impl by hand**.
   There is no field-attribute mini-language (no serde `#[serde(rename = "...")]`).
5. **No schema evolution in the library — at all.** `Serialize`/`Deserialize`
   are a *point-in-time* mapping. Versioning, renames-with-fallback,
   add-field-with-default, and reader/writer reconciliation are **out of scope**;
   reach for a schema-first tool such as **Apivolve** that generates the Tel
   types *and* their mappings from a versioned schema.

## Recommended outcome (one-line summary)

- **Three layers, separated by construction:** a built-in **data model**
  (`Data`) + a **sink** that defines how primitives are emitted, built-in
  **bridge traits** (`Serialize`/`Deserialize`) that drive the sink, and a
  **`Format` trait** (a sink/source over bytes) that *crates* implement. Serde's
  decoupling, made explicit.
- **The bridge impls come from `derive` or are hand-written, never a proc-macro.**
  Add `Serialize`/`Deserialize` to the frozen derivable set for the purely
  structural case; write the `impl` by hand for anything divergent. Versioning is
  not handled here — generate types and impls from a schema-first tool (Apivolve).
- **Two authoring styles, one wire contract.** Simple types return/consume a
  materialised `Data` (or use `derive`); streaming/large/unsized types drive the
  sink directly. A format cannot tell which the author used.
- **Conversions are capability-free; only I/O needs a capability.**
  `value.to_data()` and `json.encode(data)` are pure; writing the bytes to a file
  or socket is where a capability appears.
- **This resolves the data-formats façade question:** the multi-format façade is
  exactly this data model, and it *consumes generated/derived mappings* rather
  than synthesising them from runtime reflection (which stays an antifeature).

## The three layers serde conflates

Serde presents two traits and "a data model," but there are really **three**
independent jobs. Naming them is the whole design:

| Layer | Job | serde | **Tel: who owns it** |
|---|---|---|---|
| **L1 Data model** | the universal vocabulary every value flattens into | the 29-type serde data model (push/visitor) | **built in** — a `Data` type *plus* the `Sink` that defines emission |
| **L2 Type bridge** | this *type* ↔ the data model | `Serialize`/`Deserialize`, impls via **derive proc-macro** | **built in traits** `Serialize`/`Deserialize`; impls via **`derive` builtin or codegen** |
| **L3 Format** | the data model ↔ bytes/text | `Serializer`/`Deserializer`, one per format crate | **`Format` trait, implemented by crates** (a `Sink`/source over bytes) |

Serde fuses L2's *impl mechanism* (the derive) into the language via a macro.
That is the only part Tel changes: **L1 and L3 are unchanged in spirit; L2's
trait stays, but its impls are produced the schema-first way.**

The user's request — *"build in an API to convert data to, and a mapping to Tel
types, but let libraries implement the specific datasets"* — is exactly **L1 + L2
built in, L3 in libraries.**

## L1 — the data model (`Data`) and the sink

### The closed value type

A single closed type that any value can be flattened into and any format can read
back, format-agnostic. Richer than JSON (so binary formats lose nothing), but
small.

```tel
# Sketch — names/shapes loose. Built into std.
union Data =
    | Null
    | Bool(Bool)
    | Int(Int64)            # TODO(open): width/bignum policy — see below
    | Real(Real64)
    | Dec(Decimal)          # exact decimal, distinct from Real
    | Text(Text)
    | Bytes(Bytes)          # binary blob, distinct from Text
    | Seq(List[Data])       # ordered sequence
    | Map(List[(Data, Data)])   # ordered key→value (keys not just strings)
    | Record(List[(Text, Data)])  # named fields, order preserved
    | Variant(Text, Data)   # a tagged case: name + payload (for unions)
```

Design notes:

- **`Bytes` and `Dec` are first-class**, distinct from `Text`/`Real`. JSON will
  approximate them (base64 / number); CBOR can represent them exactly. Modelling
  them at L1 means lossy mappings are the *format's* documented choice, not a hole
  in the model.
- **`Record` preserves field order and is distinct from `Map`.** A record is "a
  fixed set of named fields" (maps onto a Tel `record`); a map is "arbitrary
  key→value." Serde keeps this split (`struct` vs `map`) and it matters for
  formats like CSV and for deterministic output.
- **`Variant` carries the union tag explicitly** because Tel unions are *untagged
  in the type system* (the type is the tag — see
  [`02-union-types.md`](../10-data-modelling/02-union-types.md)) but a wire format
  needs a discriminator. How that tag is spelled on the wire (external/internal/
  adjacent, in serde's terms) is a **format and policy** decision — see the open
  question below. This is the single hardest interaction in the TIP.

### The sink is definitional; `Data` is the reference sink

The one real L1 fork in serde — *materialised value* vs *push/streaming visitor* —
is **not a fork here**. The sink is the definition, and the materialised `Data`
value is the reification of a sink's call sequence. The two bridge mechanically:

- **materialise** = drive a value into the one sink that allocates a tree
  (`DataSink`). The result is a `Data`.
- **replay** = walk a `Data` tree, emitting the identical sink calls into *any*
  other sink (a format, say). So "return a `Data`" is sugar for "drive a sink."

```text
serialize:  YourType ──drive──▶ Sink
                                ├─ DataSink     → builds a Data tree   (simple case)
                                └─ JsonSink/…   → writes bytes, no tree (streaming)

      and:  Data ──replay──▶ Sink     (any tree can drive any sink)
```

This is precisely how serde relates `serde_json::Value` to `Serializer`: a
`Value` is produced by a visitor and replayed into one. The payoff is the whole
reason to carry both surfaces:

- The **simple author** returns a `Data` (or uses `derive`) and never touches the
  sink; the runtime replays it. No streaming complexity is imposed on the 90%.
- The **streaming author** (large documents, unsized/lazy sequences) implements
  the sink driver directly and skips the tree — the *native* case, not a bolted-on
  second surface.
- A **format never knows which style the author used**; it only receives sink
  calls (live, or replayed from a tree).

So unlike serde, the visitor is not "the primary trait you must implement" (the
complexity trap); it is the *definition*, and the materialised value is its
canonical, ergonomic special case.

### The static shape (`Shape`) — typestate sink

The sink is **affine** (`!`, see
[`0001-mutability-and-borrowing.md`](0001-mutability-and-borrowing.md)) and
parameterised by the *primitives still owed*. A `Serialize` impl therefore
declares a static **`Shape`** — a type-level description of exactly what it emits —
and the type checker proves the driver emits that and nothing else:

```tel
trait Serialize {
    type Shape                          # type-level description of emitted primitives
    fn serialize(self, sink: !Sink[Self.Shape]) -> Filled
}
```

Each sink method consumes one obligation and returns the sink with the rest; the
terminal `done()` is reachable only when no obligations remain. Affinity means the
sink cannot be duplicated or partially reused to fabricate a different shape, and
dropping it early simply yields no `Filled` — so a format never observes a
half-built shape:

```tel
#[derive(Eq, Hash, Serialize, Deserialize)]
record Point { x: Int64, y: Int64 }

# what derive(Serialize) emits (sketch):
impl Serialize for Point {
    type Shape = Record["x": Int64, "y": Int64]
    fn serialize(self, s: !Sink[Shape]) -> Filled =
        s.field("x", self.x)     # : !Sink[Record["y": Int64]]
         .field("y", self.y)     # : !Sink[Record[]]   (no obligations left)
         .done()                 # : Filled            (only reachable when empty)
}
```

Materialising is the special case, and so is replay:

```tel
fn to_data[T: Serialize](v: T) -> Data = v.serialize(DataSink()).into_data()
fn replay[S](d: Data, sink: !Sink[S]) -> Filled = ...   # checked against S
```

**This is where Tel beats serde's visitor:** serde's serializer methods are
untyped against the value's shape, so a hand-written impl can emit an inconsistent
structure. Here the `Shape` is declared and checked, so a hand-written
`serialize` is as safe as a derived one — and it unlocks the compile-time
format-compatibility check in L3.

> `TODO(open):` `Shape` needs type-level structural records (`Record["x": …]`).
> Confirm the type system can express and check these, or scope `Shape` to a
> coarser lattice (e.g. just `Sized` vs `Unsized`, see below) for Tel1.

### Avoiding `Shape` declarations

Hand-writing a `type Shape = Record["x": Int64, …]` for a large object would be
tedious, and that tedium is the headline objection to the whole static surface. It
mostly dissolves, because **you should almost never write `Shape` by hand.** Two
tiers, and a big nested object touches only the first:

1. **Structural / nested → `derive`, declare nothing.** `derive(Serialize)`
   computes `Shape` from the declared fields and **recurses**: a `Person`'s `Shape`
   is composed from the derived `Shape` of its `Address`, of each `List` element,
   all the way down. A deeply nested record gets its entire `Shape` synthesised
   with **zero annotations** — derive writes both the body and the `Shape`. The
   tedium only ever arises at the *nodes you hand-write*, and their derived
   children still contribute their `Shape`s automatically.

2. **Hand-written → infer `Shape` from the body.** `type Shape` is **optional**;
   when omitted the compiler infers it from the `serialize` body, exactly like
   return-type inference. It is mechanically free: the typestate chain already
   threads the emitted structure through the types, so the type at `.done()` *is*
   the `Shape`, read off the end of the chain.

   ```tel
   s.field("x", self.x)   # : !Sink[…"x": Int64 emitted…]
    .field("y", self.y)   # : !Sink[…"x","y" emitted…]
    .done()               # the type here *is* the inferred Shape
   ```

   (This requires the sink to *record* what it emits, not only consume
   pre-declared obligations — the natural model, but pin it down; see the `TODO`.)

   The trade-off is the usual inference-vs-contract one, and it is mild because
   inference **keeps option B**: an inferred `Shape` is still a concrete type, so
   `msgpack.encode(x)` still checks `Shape: Accepts` at the **call site**. What you
   give up is only the **definition-site** contract — refactor the body to emit an
   unsized run and nothing flags it *there*; you find out when a `msgpack.encode`
   call stops compiling. An explicit `type Shape = …` moves that error back to the
   definition. So: **inference is the default; an explicit annotation is opt-in**
   for the few impls that want the contract pinned where it is written.

Consequence for the *Static `Shape` survival* open question: the declaration
tedium — its main objection — largely goes away if **inference is the default and
derive recurses**. The remaining cost is the type-system machinery (the
`Record["x": …]` `TODO` above), not authoring effort.

`TODO(open):` a per-type *opt-out* of static shaping (e.g. `type Shape = Dynamic`,
falling back to runtime sized/unsized handling) was considered and **set aside for
now** — unclear it pays for itself, and it risks a two-worlds split. Revisit only
if a concrete need appears.

### Sized vs unsized collections

A streaming producer may not know a sequence's length up front, while some formats
(MessagePack array headers) need the count before the elements. Three options were
weighed:

- **(A) Format buffers.** The format materialises the unsized run to learn the
  count, then writes the header and flushes. Always works; costs an allocation.
- **(B) Static rejection.** The `Shape` records sizedness; a length-prefix format
  bounds its input to `Sized` shapes, so `msgpack.encode(lazy_seq)` is a
  **compile error**, while CBOR (indefinite-length) and JSON accept unsized.
- **(C) Capability negotiation.** A format advertises whether it needs lengths;
  the runtime buffers (A) only when an unsized source meets a length-needing sink.

**Resolution:** these are not exclusive, and which you get depends on whether the
static `Shape` surface survives the usability pass.

- The **default behaviour is A folded into C**: a format declares whether it needs
  lengths; if it does and the source is unsized, the runtime buffers
  automatically. Never a hard error — buffering is a fine fallback, and you do not
  want to forbid `msgpack`-ing a lazily produced sequence.
- **B is available as the stricter, opt-in typed mode** for code that wants the
  guarantee — and it *falls out for free* from the `Shape` work, since sizedness
  is already in the type. B and the typestate sink are the **same bet**: keep them
  together, evaluate them together.

## L2 — the bridge traits (`Serialize` / `Deserialize`)

The built-in API. Two small traits, no format anywhere in sight. `Serialize`
drives a sink (with materialisation as the easy default); `Deserialize` reads a
materialised `Data` (Tel1) — see the asymmetry note.

```tel
trait Serialize {
    type Shape                          # see L1
    fn serialize(self, sink: !Sink[Self.Shape]) -> Filled
    # convenience: fn to_data(self) -> Data = self.serialize(DataSink()).into_data()
}

trait Deserialize: Sized {
    # Total, explicit error. Unknown fields / missing fields are *errors by
    # default* (see the bug list in 13-data-formats.md), not silent defaults.
    fn deserialize(d: Data) -> Result[Self, DataError]
}
```

These are the *only* things user code and generic code touch:

```tel
# Generic over any type and any format — this is the façade.
fn encode[F: Format, T: Serialize](fmt: F, value: T) -> Bytes =
    fmt.run(value)                      # drives the format's sink (no tree needed)

fn decode[F: Format, T: Deserialize](fmt: F, bytes: Bytes) -> Result[T, DecodeError] =
    T.deserialize(fmt.decode(bytes)?)
```

`Decimal` exact handling, `Option` ↔ `Null`, nested records, `List`/`Map`, and
union `Variant` mapping all live in these impls.

### Encode/decode asymmetry (the part to scope carefully)

The two directions are **not** equally hard, and the cost of the static surface is
lopsided:

- **Encode is the easy direction.** The value *owns* the structure; the impl
  pushes. This is where the typestate sink and explicit `Shape` shine, and where
  the "safer than serde" claim is strongest.
- **Decode is where serde's pain lives.** The *format* owns the structure (it is
  discovering shape from bytes and pushing into you) — the source of serde's
  `Visitor` callbacks and `'de` lifetimes. The materialise/replay symmetry still
  holds (`deserialize(d: Data)` = "the source is an in-memory tree" vs "the source
  is bytes"), so a streaming *source* is expressible, but it carries the real
  complexity.

**Tel1 cut:** typestate **sink on encode**, **materialised-only `Deserialize`**
(`deserialize(d: Data)`) on decode. A streaming source is added later only if
profiled. Decode's "static type" is simply the declared target `T`; because
decoding is many-to-one, no typestate *source* is needed for the 99% case.

### Streaming decode (post-Tel1 shape)

Two cases that look similar on decode are very different in cost, and only one is
hard:

- **Unsized but bounded** (length unknown up front, but the whole value is read
  into memory). Needs **nothing new** — the format reads to the terminator and
  builds the collection; that is the ordinary materialised `deserialize(d: Data)`
  path. Drop it from the "hard" pile.
- **Incremental** (hand back elements *before* the document is fully read, in
  bounded memory). This is the only genuinely new decode capability and the
  concrete shape of the deferred *streaming source*.

**`Stream[T]` is the streaming target.** It is a stdlib type whose `Deserialize`
impl is **source-driven** instead of tree-driven: it holds the byte source and, on
each step, parses and **fully materialises one `T`**, blocking on I/O.

```tel
# materialised path (Tel1):
fn deserialize(d: Data) -> Result[Self, DataError]

# streaming path: a source-driven impl, only for stream-shaped targets.
impl[T: Deserialize] Stream[T] {
    fn deserialize_from(src: !Source) -> Stream[T]   # lazy, blocking pull; yields Result[T]
}
```

You stream the *sequence*, but each `T` is buffered — covering the dominant case
(JSONL, a huge top-level array, log records) in constant memory. No `Shape`/
typestate machinery is involved: decode is many-to-one, so the target type
`Stream[T]` *is* the declaration.

Unlike materialised decode, streaming decode is **not pure** — a `Stream[T]`
carries the **I/O capability** for its whole lifetime, because it reads lazily.
That is a real semantic split from `deserialize(d: Data)`.

**Nesting is chosen per level by the target type, and affine `!` keeps it sound.**
There is a single underlying cursor, so:

- **`Stream[List[T]]`** (any non-`Stream` inner) — *stream the outer, buffer the
  inner*, to any depth. The inner is materialised, so no cursor is shared. Simple,
  no linearity. This is the "stream only one level" case — you pick the level.
- **`Stream[Stream[T]]`** — *stream both levels*, but consumption must be **linear
  and depth-first**. The inner `!Stream[T]` borrows the single cursor; advancing
  the outer requires the inner to be **drained or explicitly `skip`ped** first.
  Affine `!` makes that compile-time-checked:

  ```tel
  for inner in outer {        # inner : !Stream[T] — borrows the cursor
      for x in inner { ... }  # drain it...
  }                           # ...or inner.skip(); pulling `outer` while `inner` is live = compile error
  ```

  You give up random access and holding sibling sub-streams: it is a depth-first
  pull parser, type-checked. `skip`ping an *undrained* inner requires the
  **format** to support structural skip (JSON skips by structure, a
  length-delimited binary format by length; a format that cannot skip forces full
  draining).

The rule: **the streamed/materialised boundary is wherever you write `Stream` in
the target type** — `Stream` ⇒ lazy and linear, a collection ⇒ buffered and
free-access. Same "target type drives decode" principle as the rest of the TIP.

**Eager prefetch is an opt-in wrapper, not a second mode.** An impl may **move**
the source into a Task (affine move carries the capability with it) that pulls and
fully deserialises ahead into a channel, overlapping I/O+parse with downstream
processing. The default stays lazy blocking-pull. Prefetch composes only with the
flat/buffered shapes (`Stream[T]`, `Stream[List[T]]`): to race ahead past a nested
`Stream` it would have to buffer that inner — which is just `Stream[List[T]]`
again. That collapse is the expected tradeoff, not a defect.

`TODO(open):` all of the above is **post-Tel1**; the Tel1 cut remains
materialised-only decode. Open sub-points: exact `Source`/`skip` trait surface,
whether `deserialize_from` is generated automatically or only for the stdlib
stream types, and error granularity on a partially consumed stream.

### Where the impls come from — the no-proc-macro answer

This is the crux, and where Tel diverges from serde hardest.

1. **Structural 99% case → `derive(Serialize, Deserialize)`** *(proposed addition
   to the frozen derivable set).* For a plain `record` whose wire shape is "field
   name → key, recursively," the mapping is as mechanical as `derive(Eq)` and is a
   **compiler builtin against the declared shape — not a macro** (the distinction
   the metaprogramming chapter draws). It honours *derive is opt-in, never
   automatic*: you write the line, you accept the structural default — including
   the derived `Shape`.

   ```tel
   #[derive(Eq, Hash, Serialize, Deserialize)]
   record Point { x: Int64, y: Int64 }
   ```

2. **Anything divergent → a hand-written `impl`.** The moment you need a field
   rename, `skip`, or input-shape ≠ output-shape, you have left what a fixed
   derive can express. Serde reaches for *field attributes*
   (`#[serde(rename = "...")]`) — author-run code on a declaration, which Tel does
   not have. You write the `Serialize`/`Deserialize` `impl` by hand instead; it is
   ordinary, readable code, and the declared `Shape` keeps it honest. **Versioning
   and evolution are not in scope** (tenet 5): when the shape changes over time,
   generate the types and their impls from a schema-first tool such as Apivolve —
   the same readable `.tel` you would have written by hand. See
   [`18-tel-as-data.md`](../17-standard-library/18-tel-as-data.md).

This split is what lets Tel claim serde's ergonomics **and** the schema-first
bug-prevention list in
[`13-data-formats.md`](../17-standard-library/13-data-formats.md#bugs-the-schema-first-stance-prevents):
distinct input/output types, unknown-field-is-an-error, explicit
unrecognised-variant policy — none of which serde's derive gives you, because all
of them need information the derive doesn't have at expansion time.

`TODO(open):` decide whether `Serialize`/`Deserialize` actually enter the
**frozen** derivable set, or whether *all* mapping (even structural) goes through
codegen for uniformity. Lean: derive the structural case (it is 90%-wanted and
clearly right, the bar for a builtin), codegen the rest. The risk of the derive is
that "just one attribute" pressure re-grows a serde-attribute surface on it — guard
against that by keeping the derive **parameterless** (no `rename`/`default`/`skip`
options on it at all; the moment you need one, you are in codegen).

## L3 — formats, implemented by crates

A format implements one trait over L1. It never names a user type. Because the
sink is definitional, a format **is a sink** (plus a finish step), and can stream
straight to bytes without a tree:

```tel
trait Format {
    type EncodeError
    type DecodeError
    type Accepts                       # the Shapes this format can represent

    # Streaming encode: drive the value into this format's sink directly.
    fn run[T: Serialize where T.Shape: Accepts](self, value: T)
        -> Result[Bytes, Self.EncodeError]

    # Decode stays materialised for Tel1 (see L2 asymmetry).
    fn decode(self, b: Bytes) -> Result[Data, Self.DecodeError]

    # Convenience over a pre-built tree (replay):
    # fn encode(self, d: Data) -> Result[Bytes, Self.EncodeError]
}
```

- **In `std`:** `json` (and `csv`), because the data-formats chapter already
  commits to them in core. JSON's `Accepts` includes unsized sequences.
- **In crates:** CBOR/CDDL, MessagePack, a project's bespoke binary wire
  format, an Avro bridge — each is "implement `Format`," nothing more. This is the
  user's *"libraries implement the specific datasets."*
- **Compile-time compatibility (option B).** `Accepts` bounds what a format can
  encode. A length-prefix format (MessagePack arrays) sets `Accepts = Sized`, so
  encoding a value whose `Shape` is unsized **fails to typecheck**; CBOR/JSON
  accept unsized. A format may instead opt to *buffer* (declare it accepts unsized
  and materialise internally to get the count) — option A/C, the default when the
  static guarantee is not wanted.
- **Format options** (pretty-print, key ordering, integer width policy) are
  fields of the format value, passed at construction: `Json(pretty = true)`. They
  are L3 concerns and never leak into L1/L2.
- **Text vs binary:** `Format` works over `Bytes`; a text format (JSON) emits
  UTF-8. Keeps one trait for both. A `to_text()` convenience can wrap the
  UTF-8-guaranteed formats.

Because L3 only ever sees `Data`/sink calls, **a new format instantly works with
every type that has a bridge impl, and a new type instantly works with every
format** — the serde combinatorial win, preserved.

## Capability model

Conversions are **pure**: `value.to_data()`, `Json().encode(data)`, and
`Point.deserialize(d)` need **no capability** — they are value→value. A capability
appears only at the I/O boundary (writing the bytes to a file, reading from a
socket), where it already does in Tel. Consequence: serialisation logic is
**unit-testable without granting any I/O capability**, which is a real ergonomic
win over reflection-based serializers that often reach for ambient I/O.

## What this explicitly keeps out

- **No runtime reflection.** The bridge impl is derived or generated source, read
  like any other code — never runtime type introspection (still an antifeature,
  [`02-reflection.md`](../15-metaprogramming/02-reflection.md)).
- **No proc-macro / no author-run field attributes.** The parameterless derive
  and the schema codegen replace `#[serde(...)]` entirely.
- **No native/self-describing object serialisation** (`pickle`/`gob`/Java
  `Serializable`) — remains an antifeature; `Data` is an *explicit, inspectable*
  model, not an opaque type-locked blob.
- **No zero-copy/borrowed deserialisation in the API.** Serde's `Deserialize<'de>`
  lifetimes are a major complexity sink and entangle with Tel's unresolved
  borrowing model ([TIP-0001](0001-mutability-and-borrowing.md)). `Deserialize`
  yields **owned** values for Tel1; revisit only if a profiled need appears. (Note
  the *encode* sink is affine `!`, which Tel1 already has — that is a different,
  settled mechanism from borrowed *decode*.)

## Round-trip convention

`Serialize` is a function (one `Data` per value); `Deserialize` is many-to-one
(several `Data` shapes may decode to one value, or error). State the laws as a
*convention*, not an enforced guarantee:

- **Intended (holds for derived impls):**
  `T.deserialize(v.to_data()) == Ok(v)` — the model round-trip preserves the
  value. A `std` property-test helper can check it for hand-written impls.
- **Deliberately does *not* hold:** `to_data(T.deserialize(d)?)` ≠ `d` in general —
  it canonicalises (drops key order, integer-encoding slack, absent-vs-`Null`).
  That slack is exactly what a format is free to exploit.

Neither is statically enforced; the first is property-testable.

## Decision table (proposed for Tel1)

| Question | Verdict |
|---|---|
| Adopt serde's data-model decoupling? | **Yes** — it is the good half |
| Build the **data model** (L1) into std? | **Yes** — a closed `Data` type |
| Build the **bridge traits** (L2) into std? | **Yes** — `Serialize`/`Deserialize` |
| Trait names? | **`Serialize`/`Deserialize`** — `Encode`/`Decode` rejected (collides with byte verbs); `ToData`/`FromData` rejected (vague, over-exposes layering) |
| Materialised `Data` or streaming visitor as primary? | **Neither is "primary":** the **sink is definitional**, `Data` is its reference sink; materialise/replay bridge them |
| Static `Shape` (typestate sink, compile-time format compat)? | **In — provisionally.** Keep both static + dynamic; re-evaluate after a usability pass |
| **Formats** (L3) in language or crates? | **Crates** (json/csv in std; rest external) via a `Format` trait |
| Reproduce serde's **derive proc-macro**? | **No** — never |
| Structural mapping via `derive(Serialize, Deserialize)`? | **Lean yes** — a *parameterless* builtin derive in the frozen set |
| Rename/skip/custom mapping? | **Hand-written `impl`** — ordinary code, no attributes |
| Sized vs unsized collections? | **A+C default** (format buffers / negotiates); **B** (compile-time reject) opt-in, falls out of `Shape` |
| Streaming on decode? | **No for Tel1** — `deserialize(d: Data)` only; add a source later if profiled |
| Schema evolution / versioning in the library? | **No** — out of scope; use a schema-first tool (Apivolve) |
| serde-style **field attributes** (`rename`, `default`)? | **Reject** — write the `impl` (or generate it from a schema) |
| Conversions need a capability? | **No** — pure; only I/O needs one |
| Runtime reflection / native serialisation? | **Reject** — unchanged antifeatures |
| Zero-copy/borrowed *deserialisation*? | **Reject for Tel1** — owned values only |

## Open questions

- **Does the static `Shape` surface survive?** Carried provisionally alongside the
  materialised surface (see *Scope*). After a usability pass, decide: keep, narrow
  to encode-only, reduce `Shape` to a `Sized`/`Unsized` flag, or drop to
  materialised-only. The typestate sink and option B stand or fall together. The
  *declaration-tedium* objection is largely answered (see *Avoiding `Shape`
  declarations*: derive recurses, inference is the default), so the live cost is
  the type-system machinery, not authoring effort. A `Dynamic` per-type opt-out was
  set aside for now.
- **Type-level structural records for `Shape`.** Does the type system express and
  check `Record["x": Int64, …]`? If not, scope `Shape` to a coarser lattice.
- **Wire-tagging of untagged unions (hardest).** A Tel union is untagged in the
  type system but needs a discriminator on the wire. `Variant(name, payload)` at
  L1 carries it, but *which* representation (external `{"Circle": {...}}`, internal
  `{"type":"Circle", ...}`, adjacent) is a per-format/per-schema choice. Where is
  it decided — the schema (codegen) or a format option? Lean: the **schema**
  declares the tagging strategy (it is a data-shape decision), the format obeys
  it. Coordinate with [TIP-0002](0002-untagged-unions-and-sealed-traits.md) and
  the **non-exhaustive union** / unrecognised-variant policy in
  [`13-data-formats.md`](../17-standard-library/13-data-formats.md#bugs-the-schema-first-stance-prevents).
- **`Data` numeric policy.** One `Int(Int64)` vs distinct widths vs a bignum case;
  how `Real`/`Dec` interact with JSON's single number type and CBOR's exactness.
  Affects round-trip fidelity. Decide the smallest model that loses nothing for the
  committed formats. Interacts with `Shape` (widths would surface there).
- **Does `Serialize`/`Deserialize` enter the *frozen* derive set**, or is even the
  structural case codegen-only? (See L2 `TODO`.) The freeze is permanent — weigh
  carefully.
- **Streaming source (decode).** Shape now sketched in *Streaming decode
  (post-Tel1 shape)* above — `Stream[T]` as a source-driven target, per-level
  streamed/buffered boundary, affine-linear nesting, opt-in Task prefetch. Still
  deferred past Tel1; remaining sub-points are the `Source`/`skip` trait surface,
  auto- vs stdlib-only `deserialize_from`, and partial-stream error granularity.
- **Schema evolution — decided: out of scope** (tenet 5). `Serialize`/`Deserialize`
  are a point-in-time mapping; no evolution algebra lives in the library. The
  evolution discussion in
  [`13-data-formats.md`](../17-standard-library/13-data-formats.md#schema-evolution)
  should defer to an external schema-first tool (Apivolve) rather than threading
  rules into a generated `deserialize`. TODO(open): update 13-data-formats to drop
  built-in evolution and point at the schema-first path.
- **Error model.** One `DataError` (L2) vs per-format `DecodeError` (L3) and how
  they compose; how much positional/path context an error carries ("field
  `user.id` expected Int, got Text").
- **`Map` key types.** L1 allows non-`Text` map keys; JSON does not. Is that a
  format-level restriction (json rejects non-text keys) or an L1 restriction? Lean:
  L1 permits it, the format documents/raises if it cannot represent it (or, with
  `Shape`, bounds `Accepts`).

## See also

- [`17-standard-library/13-data-formats.md`](../17-standard-library/13-data-formats.md)
  — the schema-first stance, the façade open question this TIP resolves, and the
  bug list the design must keep preventing.
- [`17-standard-library/18-tel-as-data.md`](../17-standard-library/18-tel-as-data.md)
  — the codegen path that produces non-structural bridge impls.
- [`15-metaprogramming/03-derive-and-attributes.md`](../15-metaprogramming/03-derive-and-attributes.md)
  — the frozen derivable set `derive(Serialize, Deserialize)` would join.
- [`15-metaprogramming/01-macros.md`](../15-metaprogramming/01-macros.md) — why the
  serde *derive* (proc-macro) is rejected while the data model is welcomed.
- [`10-data-modelling/02-union-types.md`](../10-data-modelling/02-union-types.md)
  and [TIP-0002](0002-untagged-unions-and-sealed-traits.md) — untagged unions and
  the wire-tagging question.
- [`0001-mutability-and-borrowing.md`](0001-mutability-and-borrowing.md) — the
  affine `!` types the typestate sink threads.
- [`17-standard-library/20-data-access-and-orms.md`](../17-standard-library/20-data-access-and-orms.md)
  — the same schema-first, code-generated stance for database rows.
- [`inputs/metaprogramming-use-cases.md`](../../../inputs/metaprogramming-use-cases.md)
  — row #2 (serialisation), which this TIP expands.

TODO: review
