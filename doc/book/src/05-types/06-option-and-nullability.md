# Option and Nullability

Tel has **no `null`**. A value that might be absent has a type that says so.
This is a direct consequence of the safety priority — "invalid states should be
unrepresentable" — and removes the single largest class of runtime errors in
mainstream languages.

## What — optionality is a type

An optional value has type **`Option[T]`**: it either holds a `T` or is the
explicit absence value (call it `None` for now — final spelling open). To use
the inner value you must first establish it is present, normally by matching.

```tel
fn find_user(id: Id[User]) -> Option[User] { ... }

match find_user(id) {
    u: Some => greet(u),
    None    => "no such user",
}
```

There is no way to "forget" the `Option` and reach the `T` directly; the
compiler will not let an `Option[User]` be used as a `User`.

## Why — and how it builds on untagged unions

`Option[T]` is not a magic built-in. It is an ordinary union (see
[`../10-data-modelling/02-union-types.md`](../10-data-modelling/02-union-types.md)):

```tel
type Option[T] = (Some[T] | None)
```

Untagged unions give a real bonus here, noted in the design: because `Some[T]`
is itself a type, a function or trait method declared to return `Option[T]` can
have an implementation that *always* returns `Some[T]` — and a caller holding
that concrete type statically knows there is no `None` to handle. The optional-
ness genuinely disappears when it is not needed. See
[`09-subtyping-and-variance.md`](09-subtyping-and-variance.md).

## The Option[Option[T]] collapse problem

This is the subtle reason `Option[T]` must be defined with **wrapper types**,
not as a bare `(T | None)`.

Untagged unions flatten and deduplicate. If `Option[T]` were `(T | None)`, then:

- `Option[Option[T]]` would be `((T | None) | None)` = `(T | None)` = `Option[T]` —
  the two layers collapse, and "present but holding absence" becomes
  indistinguishable from "absent".
- Worse, if `T` is itself nullable or `T = None`, then `(None | None) = None`, and
  a "present" value cannot be told from an absent one at all. A `is None` check
  no longer means what it says.

Wrapping the present case in a `Some[T]` newtype fixes this:

```tel
struct Some[T] { value: T }
type   None
type   Option[T] = (Some[T] | None)
```

Now `Option[Option[T]]` is `(Some[(Some[T] | None)] | None)` — three genuinely
distinct shapes, no collapse. `Some[T]` acts as the tag for the present case.
This is the same wrapper-struct technique generic unions need in general; see
[`07-generics.md`](07-generics.md).

This trade-off — untagged unions need manual wrapper structs to avoid
collapse — is recorded honestly. It is the price of untagged unions; the design
accepts it because the wrappers are cheap (see
[`12-refined-types.md`](12-refined-types.md)) and because untagged unions buy the
subtyping and flattening benefits elsewhere.

## Ergonomics

Optionality is common in data-transformation code, so it must be terse:

- A **fallback operator** for "use this value, or that one if absent" — short,
  because data transforms lean on it heavily. The candidate is an `or`-style
  operator (Python/Bash flavour). It applies to `Option`, not to truthy/falsy
  values — Tel has no truthiness.
- Operations like `is_some` / `map` should work *through* an `Option`. The
  goal is that `Option[Okayable]` has an `is_ok` that returns a sensible
  value for `None` and delegates otherwise — a special case of implementing
  methods on a parameterised type.
- A **`to-do`-style accessor** that extracts the value and aborts if absent, for
  the cases where the author truly knows it is present. Loud failure, never a
  silent `null`-deref.

TODO(open): exact set and spelling of `Option` ergonomics — the fallback
operator, `map`/`and_then`, the abort-if-absent accessor. Keep it terse but
explicit, per "error handling is explicit but terse".

TODO(open): final names — `Option` / `Some` / `None`, or `?T` sugar, or
lowercase `none` as in some notes. Pick names a Rust/Kotlin/Swift reader finds
unsurprising.

## Bugs this prevents

The catalogue records dozens of
`null`-related production incidents. Each falls into one of a few shapes:

- **`null` as "no change" colliding with `null` as "the value zero".** A GUI
  sent parameter overrides as a diff in one mode and absolute values in
  another. `null` meant "cleared" in diff mode but was an actual prior value
  in absolute mode; `0` meant "no change" in one direction and "the literal
  zero" in the other. The fix the catalogue suggests is a [tagged
  union](../10-data-modelling/02-union-types.md) per kind of update — which
  is exactly what Tel forces, because there is no overload of "absent" and
  "value" in one type.
- **NPE in production because an upstream algorithm produced data half the
  time.** Code assumed a value was always present; in a different setup it
  was not. Tel forces the consumer to handle `none` at every use site, so
  the contract is never "I think this is always present" — it is "the type
  says it might be absent."
- **`null` not allowed in `ConcurrentHashMap` keys or values.** Migration
  from `HashMap` to a concurrent variant broke at runtime. With `Option[V]`
  the absent case is a distinct value, not a hole in the map; the collection
  doesn't have to pick a representation for "missing" that conflicts with a
  legitimate stored value.
- **Two `null` values in a set of identifiers that was supposed to be
  unique.** A change relied on `(source, expiry)` being unique, but several
  rows had `null` expiries. In Tel the type of an "expiry that may be
  absent" is `Option[Expiry]` and the uniqueness invariant is stated on the
  refined collection, not derived from "the database happens not to have
  duplicates."
- **The revert that broke persisted data.** When the above change was
  reverted, the new enum variant the change introduced disappeared from the
  code but stayed in the data already written. Tel cannot prevent a revert
  from removing a needed variant, but the
  [non-exhaustive union](../10-data-modelling/02-union-types.md) opt-out
  means a caller cannot silently treat a new variant as `null` /
  "everything else".

## See also

- [Union Types](../10-data-modelling/02-union-types.md) — the mechanism `Option`
  is built on.
- [Generics](07-generics.md) — the wrapper-struct pattern in general.
- [Antifeatures — no null](../02-philosophy/04-antifeatures.md).

TODO: review
