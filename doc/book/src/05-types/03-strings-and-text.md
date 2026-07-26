# Strings and Text

Text is a primitive concern for Tel — data transformation is a core use case, so
strings must be pleasant and predictable. This page covers the *type* of text;
syntax for string literals and interpolation belongs in the syntax chapter.

## What — the text type

Tel has one canonical immutable text type, **`Text`**. It is what most other
languages call `String`; Tel uses the name `Text` deliberately, because
`String` tends to imply a specific in-memory encoding and `Text` does not.
A `Text` value is a sequence of Unicode characters and, like all Tel values, is
immutable: operations that "change" text produce a new value.

Tel does **not** commit to a single storage encoding. Because the same script
runs across many hosts, each host stores `Text` in whatever form suits it —
UTF-8 is the common choice, but UTF-16 or another encoding is permitted. A
script only ever sees Unicode text and the behaviour defined below; it cannot
observe, or depend on, the underlying bytes (see
[representation is hidden](#why--representation-is-hidden-behaviour-is-not)
below).

A separate **mutable text builder** type exists for the building-up case — see
[Mutability and the builder pattern](#mutability-and-builders) below.

TODO(open): the unit of indexing and length — bytes, Unicode scalar values, or
grapheme clusters. This must be pinned down because Tel runs the *same script*
across many hosts and the answer must be identical everywhere (see
[Goals](../01-overview/03-goals-and-non-goals.md)). This is undecided;
treat it as a spec-level decision the multi-host portability requirement forces.

## Why — representation is hidden, behaviour is not

Following the same principle as numeric types
([`02-primitive-types.md`](02-primitive-types.md)), the *representation* of a
string is an implementation detail of the host runtime. A script sees `Text`; it
does not see, and cannot depend on, whether the host stores it as UTF-8, UTF-16,
small-string-optimised on the stack, or interned.

Several representation ideas are on record — capacity as the next power of
two so length alone suffices, short-string optimisation, storing a prefix or
hash inline so comparison-heavy code (database keys, tree nodes) avoids pointer
chasing. These are **implementation-note material**, not language surface: they
must not leak into observable behaviour, because two conforming hosts may make
different choices. They are recorded in `impl-notes/`, not here.

What the language *does* guarantee is behaviour: comparison, ordering, equality,
and hashing of `Text` are defined and identical across hosts. See
[`../10-data-modelling/07-equality-and-hashing.md`](../10-data-modelling/07-equality-and-hashing.md)
and [`../10-data-modelling/08-ordering.md`](../10-data-modelling/08-ordering.md).

## Mutability and builders

Building a string piece by piece with an immutable type is quadratic if done
naively. Tel's answer is the same as for collections (see
[`../10-data-modelling/09-collection-types.md`](../10-data-modelling/09-collection-types.md)):
the **owned form** `!Text` (affine, mutable in place), whose scope is small and
local, which produces a shareable `Text` when done.

```tel
let uniq sb = !Text()
for word in words {
    sb.append(word)
    sb.append(" ")
}
let line: Text = sb.finish()
```

Mutability is thus a property of the *type* — `!Text` vs `Text`, the `!T` sigil
([TIP-0001](../tips/0001-mutability-and-borrowing.md)) — not a modifier on a
`Text`. This is the same mechanism collections use (`!List` / `List`); it keeps
the immutable text type freely shareable while still allowing efficient
construction. (The earlier `TextBuilder` name is retired in favour of `!Text`.)

## Refined text types

Because refined types are cheap (see [`12-refined-types.md`](12-refined-types.md)),
domain text is normally a named wrapper rather than a bare `Text`, optionally
with a value constraint:

```tel
type NonEmptyText = Text where len(self) > 0
type Iso3166     = newtype Text   // a country code; cannot be mixed with bare Text
```

One open idea is modelling "strings with the same encoding" — see the
table/column discussion in
[`../10-data-modelling/09-collection-types.md`](../10-data-modelling/09-collection-types.md)
for storing a shared property once per collection rather than per element.

## See also

- [Primitive Types](02-primitive-types.md)
- [Refined Types](12-refined-types.md)
- [Equality and Hashing](../10-data-modelling/07-equality-and-hashing.md)

TODO: review
