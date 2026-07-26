# Equality and Hashing

Two values are **equal** when they represent the same thing. They have the
same **hash** when a hash-using container (a `Map`, a `Set`) needs to compare
or place them quickly. In Tel, equality and hashing are *value-shaped* — they
work on what the value *is*, not on which heap location it lives in.

## What — structural equality, derived by default

Tel has **no reference equality**. Two records that hold the same fields are
equal regardless of how each was constructed. Reference identity is not
exposed to user code; there is no `is` operator that distinguishes "two names
for one object" from "two objects with equal contents".

```tel
let a = Point { x = 1.0, y = 2.0 }
let b = Point { x = 1.0, y = 2.0 }

a == b      # true — same fields, same values
```

For built-in composites the rules are the obvious ones:

- **Records** are equal when corresponding fields are equal.
- **Tuples** are equal when each position is equal *and* each named field is
  equal; the order named fields were written does not matter (the named tail is
  an unordered set). Differently-shaped tuples are different types and do not
  compare — see
  [tuples](../05-types/04-tuples-and-arrays.md#identity-equality-and-narrowing).
- **Arrays and lists** are equal when same length and pairwise equal.
- **Maps and sets** are equal when they contain the same key/value pairs (or
  members) — order does not contribute.
- **Unions** compare equal when both sides have the same member type *and* the
  values within that type are equal.

A type that wants equality is therefore equal "for free" if all its parts
are; the compiler synthesises the implementation. This is the
*derive* story (see [`03-traits-or-interfaces.md`](03-traits-or-interfaces.md)):
the user writes a request — `impl auto Eq for TheType` (see
[Derive and Attributes](../15-metaprogramming/03-derive-and-attributes.md)) —
and the compiler fills in the boilerplate. Equality is **not** inherited automatically by
a newtype, though — a wrapper lists `Eq` (or any capability) explicitly, like
every other trait (see
[trait inheritance for newtypes](../05-types/12-refined-types.md#trait-inheritance-for-newtypes)).

TODO(open): a **test-only structural equality**, separate from domain `Eq` — the
equality analogue of how `Debug`/`Show` gives a diagnostic representation
independent of any domain formatting. The motive: a type may give `==` a custom
domain meaning (normalised, case-insensitive, …) or deliberately have no `Eq` at
all, yet a test still wants to assert two values are *structurally* identical,
field by field. A derived test-only equality would serve assertions without being
exposed as `==`. The blocker is **differentiation**: how a reader and the type
system tell domain `Eq` from the test-only one at the use site, and what keeps the
test-only form out of production comparisons (test-build-only? no operator,
reachable only from assertion helpers?). Lean: probably not — two notions of
"equal" is a readability cost; revisit only if test ergonomics demand it.

## Why — equality is a property of values, and built-in types set the bar

Tel's promise that **anything built-in types can do, user-defined types can
do too** (see [`../05-types/12-refined-types.md`](../05-types/12-refined-types.md))
applies most visibly to equality. A `EuroAmt` newtype wrapping a `Decimal`
gets equality, because `Decimal` has equality. A `Pair[Int64, Text]` gets
equality, because both parts do.

The reverse direction matters too: equality is a *trait* (the `Eq` bound, or
its eventual spelling) generic code can require. A `Set[T]` only makes sense
for types that have equality and hashing; the bound is what the type system
checks at the use site.

## Eq, PartialEq, and the NaN problem

Equality has a well-known wart: `Real64` (IEEE 754 float) has values that are
not equal to themselves (`NaN != NaN`). This is a real concern:
*bitwise* equality and *mathematical* equality come apart for floats, and a
type system that does not distinguish them gets surprising hashmap behaviour.

Tel's working answer follows the now-standard Rust/Haskell pattern:

- **`PartialEq`** — equality that may not be reflexive. `Real64` is `PartialEq`
  but not `Eq`: `NaN` is not equal to itself, but `1.0 == 1.0`.
- **`Eq`** — full equality: reflexive, symmetric, transitive. `Int64`, `Text`,
  records of `Eq`-able fields. `Real64` does **not** satisfy `Eq`.
- **Hash** requires `Eq`. A `Map[K, V]` therefore cannot use `Real64` as a key;
  if you want to, wrap it in a refined type that excludes NaN and re-define
  equality there.

TODO(open): final names — `Eq` / `PartialEq` are Rust's; Haskell calls them
`Eq` / no-counterpart. Lean: the Rust naming, because the partiality is real
and worth surfacing.

TODO(open): should `==` call `PartialEq` (returning
`Bool` and accepting that NaN comparisons are *false*) or `Eq` (rejecting
types that lack reflexivity at compile time). Lean: `==` calls `PartialEq`,
because `Real64 == Real64` should always type-check; uses that require
reflexivity (hashing, ordering) bound on `Eq` separately.

## Implementing `Eq` from `Ord`

**Implementing `Ord` automatically gives `Eq`**: two
values are equal iff `cmp(a, b)` returns `Equal`. This is a small bit of
useful inference — the type only spells the ordering out once. A type may
*specialize* `Eq` for performance (a faster equality check than the full
comparison), but the default falls out.

See [`08-ordering.md`](08-ordering.md) for the ordering side.

TODO(open): confirm the auto-`Eq`-from-`Ord` derivation, and that
`PartialEq` / `PartialOrd` get the same treatment. Lean: yes.

## Hashing

Hashing is what makes a value usable as a key in a `Map` or member of a `Set`.
Like equality, hashing is derived structurally:

- A record hashes as a combination of its fields' hashes.
- A tuple hashes as the combination of its elements' hashes — positional slots
  in index order, then named fields **in name order**, so a named-field tuple
  hashes the same no matter the order its fields were written.
- A union hashes as a combination of a *member-type discriminator* and the
  contained value's hash.

The hash function for primitives is fixed by the language (so `Text.hash` is
the same across hosts). For user types it is derived; an author may opt to
write a custom `Hash` impl when the structural one is wrong (e.g. a record
where one field is incidental and should not contribute).

### The Eq–Hash contract

The classical contract holds: if `a == b` then `hash(a) == hash(b)`. The
compiler enforces this when both are derived (it cannot enforce it when one
is hand-written and the other is derived; that combination is the usual place
the contract slips). This is a place
[`derive` should refuse to mix](#hash-and-eq-must-be-derived-together) hand-
written and synthesised implementations.

### Hash and Eq must be derived together

If a type derives `Eq`, it should derive `Hash`. If a type writes a custom
`Eq`, it must write a custom `Hash` consistent with it. Mixing — derived `Eq`
plus custom `Hash` — is a likely silent bug; lean toward a hard compile error.

TODO(open): confirm "either both derived or both hand-written" as a hard
rule. Lean: yes; it catches the classical contract violation at the
type-system level.

### One declared key set, not two functions

The "derive both or hand-write both" rule still leaves the hand-written
case able to drift — two functions, kept consistent only by discipline.
Tel narrows that case to a form that *cannot* drift: instead of writing
an `Eq` body and a separate `Hash` body, the author declares the **set
of properties that identify the value**, and the compiler derives *both*
equality and hashing from that one list.

```tel
# Only `id` and `version` identify a Release; `cached_etag` is incidental.
struct Release {
    id:          Id[Release],
    version:     SemVer,
    cached_etag: Text,

    identity = (id, version)     # eq and hash both derive from this list
}
```

This covers the common reason to reach for a custom impl — *"one field
is incidental and must not contribute"* — without ever writing two
parallel functions that can disagree. A fully hand-written `Eq`/`Hash`
pair stays possible for the rare case where identity is not a subset of
the fields (a normalised comparison, say); there the "both-or-neither"
rule above still applies.

`TODO(open): spelling of the key-set declaration (`identity = (...)` in the
type body, or `impl auto Eq, Hash for T from (...)`). A `@key(...)`
attribute is off the table now that attributes are advisory-only — a key
list changes equality semantics (see
[Derive and Attributes](../15-metaprogramming/03-derive-and-attributes.md)).
The point is one list, two derivations. Decide whether ordering (`Ord`) can
hang off the same list.`

### Hash and Eq are constant per type

A value's hash and equality must not depend on *which module is looking at it* — a
`Set` or `Map` built in one place and read in another must agree on where a key
belongs. Tel gets this from the general
[orphan rule](03-traits-or-interfaces.md#coherence-the-orphan-rule-and-specialisation):
`Eq`/`Hash`/`Ord` are implementable only by the crate owning the trait or the
type, and any overlap across crates is a rejected conflict, so there is exactly
**one resolved impl per type** in a program. No special single-owner rule is
needed — these traits follow the same coherence rules as any other.

What the author still owns is each impl's *internal* consistency — the **Eq–Hash
contract** (`a == b ⇒ hash a == hash b`), and any same-owner specialisation
agreeing with its general impl. The [`identity` key-set](#one-declared-key-set-not-two-functions)
and `derive` keep that automatically; a hand-written pair keeps it by discipline.
See [TIP-0005](../tips/0005-trait-coherence-and-the-orphan-rule.md).

## Hashing needs immutability

A value used as a `Map` key or `Set` member must not change while the
container holds it: a key whose hash shifts after insertion is lost in
its own map — the classic mutable-key corruption. Tel's
[immutable-by-default model](../06-bindings-and-scope/02-mutability.md)
makes this a non-issue for ordinary values — a record never changes in
place, so "changing" a key produces a *new* value (copy-with-update) and
the stored key is untouched.

The rule for explicitly mutable data:

- A `uniq` (mutable) binding is **not** `Hash`, so it cannot be used as a
  key or set member directly. Hashing a value that can change out from
  under the container is a contradiction the type system refuses, rather
  than a runtime footgun.
- To key on mutable data, take its **immutable snapshot** first — the
  copy is a plain immutable value with the usual derived hash. Because
  immutable values do not expose identity, the snapshot's hash depends
  only on its contents.
- Equality of `uniq` data is still available where it is meaningful
  (comparing two working buffers); a type is only `Hash` on its immutable
  form. `Eq`-without-`Hash` is the mutable case, `Eq`-and-`Hash` the
  immutable one.

`TODO(open): whether `uniq` data is `Eq` at all, or only comparable
through an explicit `.snapshot()`. Lean: `Eq` yes (comparing two mutable
buffers is meaningful), `Hash` no. Confirm against the mutability model
and the substructural-types chapter
([`../12-memory-and-runtime/08-substructural-types.md`](../12-memory-and-runtime/08-substructural-types.md)).`

## Per-host hash determinism and the `random` effect

A subtlety: a typical `HashMap` is **randomised** at the seed
level to defend against hash-flood attacks. That randomisation is observable
when *iterating* a map, even though it is invisible for `get` / `insert` /
`contains`. For Tel's reproducible-by-default goal (see
[the maxims](../02-philosophy/02-maxims.md)) and any deterministic-simulation
host, this matters.

Tel's working answer:

- **Hash-using operations track a `random` effect** when their observable
  result depends on the seed. `Map.iter()` does; `Map.get(k)` does not.
- **Iteration in insertion order** is available as a separate type
  (`LinkedHashMap` / `OrderedMap`), which is *free of* the `random` effect at
  iteration time. Is a `LinkedHashSet` free of the
  random effect? Yes, because iteration order is no longer seed-dependent.
- The host **supplies** the seed (or the choice of "deterministic seed for
  this run"), the way it supplies a clock or RNG, so a script running in a
  simulation host can fix the seed for repeatability.

TODO(open): the exact set of map operations that carry the `random` effect.
Lean: just iteration and anything that derives from iteration order
(`first`, `to_list` without an explicit sort, structural equality of the
key set treated as a sequence). `get`, `insert`, `contains`, `len`, equality
as sets, summing values, etc., do *not*. Confirm.

TODO(open): whether a user can implement a `Map`-like type whose `get` does
not surface a `random` effect even though the internals use randomness.
Resolution likely: yes, by the implementation declaring the effect contained
internally; flag as a refinement of the effect system in
[`../05-types/05-function-types.md`](../05-types/05-function-types.md).

## Equality of refined and newtype values

A [refined type](../05-types/12-refined-types.md) inherits equality from its
underlying type. `EuroAmt(19.99) == EuroAmt(19.99)` is true; `EuroAmt(19.99)
== UsdAmt(19.99)` does not even type-check (different types).

There is also a worry about equality across borrowed-vs-owned forms in
languages with explicit references. Tel does not expose user-visible
references today (see [antifeatures](../02-philosophy/04-antifeatures.md)), so
this is not a user-facing concern; if a reference model lands later, equality
between an owned `T` and a borrow of `T` must be defined as the equality of
the underlying values.

## Bugs derived-equality prevents

A pair of catalogue cases drive why equality and hashing are derived by
default for ordinary value records, and why mixing a derived and a
hand-written implementation is treated as a hazard:

- **"`HashMap` merge with a portfolio that didn't implement `equals`."** A
  portfolio was being subscribed twice with different weights; the merge
  used a `HashMap` to combine them, but the portfolio class did not
  override `equals`/`hashCode`, so reference-equality semantics applied
  and the merge silently double-counted. In Tel a record's equality is
  derived from its fields by default, and a `Map[Portfolio, _]` key uses
  *that* equality — the "I forgot to implement `equals`" bug class
  disappears.
- **"Inconsistent equals across object variants."** Two types representing
  the same conceptual entity had different equality implementations (one
  compared time, one didn't, for backwards-compatibility reasons). Code
  that crossed both kinds saw inconsistent keys and produced duplicate-key
  errors. Tel's rule that equality and hashing must be derived together
  (and that an out-of-band custom equality requires a custom, consistent
  hash) makes the contract a compile-time concern, not a documentation
  one.

## Equality and the host boundary

When a Tel value crosses to the host or back, equality must remain consistent
— two values that compared equal on the way out compare equal on the way back
in. This implies host bindings adopting Tel types must round-trip equality
faithfully; binary representations differ across hosts, but observable
equality does not.

TODO(open): how `Real64` equality and `NaN` are surfaced across the boundary —
e.g. does a host that does not distinguish `NaN` payloads collapse them.
Defer to the host-boundary chapter.

## See also

- [Ordering](08-ordering.md) — `Ord` / `PartialOrd`, and the auto-derivation
  story.
- [Traits or Interfaces](03-traits-or-interfaces.md) — the `derive` mechanism.
- [Collection Types](09-collection-types.md) — `Map` and `Set` consumers of
  the contract.
- [Primitive Types](../05-types/02-primitive-types.md) — the `Real64` / `NaN`
  background.
- [Refined Types](../05-types/12-refined-types.md) — inherited equality.

TODO: review
