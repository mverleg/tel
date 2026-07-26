# Destructuring and Object Construction

<!-- TODO: review -->

Tel makes it cheap to pull a record apart into named bindings, and equally
cheap to put one together from bindings already in scope. Both lean on
**field names**: a value's fields and the local names around them line up by
name, not by position. This is one of the features that makes data-conversion
code — Tel's bread and butter — short and safe.

## Destructuring: a value into bindings

A record can be taken apart in a binding:

```tel
let Point { x, y } = some_point
# now `x` and `y` are bindings holding the point's fields
```

Each field name becomes a binding of the same name. As with any `let`, the
bindings are immutable; a destructuring pattern is an explicit binding site, so
its names may [shadow](04-shadowing.md) an outer name (a bare assignment cannot).

TODO(open): a "spread into locals" form — turning *all* of a value's fields
into bindings at once (`*=Point{*}`, "a way to turn all fields into locals").
Its spelling and whether it is allowed outside
codegen-style use are unresolved.

Destructuring also appears in function parameters and in `match` arms; see
[Pattern Matching](../10-data-modelling/06-pattern-matching-in-depth.md).

### Sequence destructuring

Tel also supports (loosely) list-shape patterns à la Grain or
Rust:

```tel
match xs {
    [first, ...rest] => head_and_tail(first, rest)
    []               => empty()
}

let [a, b, c] = three_things()
```

`...rest` collects the remainder into a new sequence. The leading-`.` rule
for variant patterns (see
[Match Expressions](../08-control-flow/02-match-expressions.md)) does not
apply here — list patterns are bracketed, so there is no ambiguity with
variant names.

`TODO(open): list-pattern detail.` Whether patterns can match mid-list
(`[a, ..mid, z]`), and whether they apply only to specific collection types
or to anything iterable, is open. Defer to the pattern-matching topic.

## What may be destructured

Two independent things decide whether a value can be taken apart at a given
site.

**The `Unpack` capability — is destructuring sanctioned for this type?**
Destructuring is a member of the substructural
[capability family](../12-memory-and-runtime/08-substructural-types.md#unpack--publishing-destructuring-as-the-public-use).
Ordinary (`Discard`) data is `Unpack`, so everyday records and tuples come apart
freely; a value type can decline it (`not Unpack`) to be readable only through its
methods. A relevant resource such as a `Db` or `Txn` is `not Unpack`, so it cannot
be quietly retired by being unpacked and must go through its consuming methods
(`close`, `commit`). Inside the defining module destructuring is always
available; `Unpack` is what *publishes* it to outside code, making it the public
spelling of the "use" a type would otherwise expose as a named method. This —
not a field trick — is the intended way to control destructuring.

**Field visibility — can you name the fields here?**
Destructuring is name-based, so it is *also* floored by field
[visibility](../11-modules-and-packages/03-visibility.md): a pattern can never
bind, skip, or ignore a field invisible at the use site. So a single `private`
field likewise blocks destructuring from outside the defining module — even a
dummy `private the_seal: ()` would — but reach for `not Unpack` first; visibility
is an encapsulation tool that merely happens to have this effect.

Either route also stops external **construction** by field literal (the
unnameable field cannot be supplied), so such a type provides a constructor for
outside callers.

### Skipping fields

A pattern need not bind every field, but a dropped field must be safe to drop. A
field may be **left unbound only if it is not relevant** — i.e. it is `Discard`.
Skipping a **relevant** field is a compile error: dropping a must-use value
silently is exactly what relevance forbids, so a relevant field must be bound
(and then used or moved onward).

Skipping is **explicit**, via a trailing rest `..`, never silent omission —
discarding is always written out, in keeping with "an error is never dropped
silently":

```tel
let Config { host, port, .. } = cfg   # binds host/port, drops the rest — OK if all Discard
```

Two earlier rules still hold and compose with this one:

- **`Unpack`** must permit destructuring at all (or you are inside the defining
  module).
- **Binding** a field requires it to be **visible** at the site (you name it);
  `..`, by contrast, drops without naming, so it can absorb fields that are not
  visible — *provided they are `Discard`*.

The corner that falls out: a type with an **invisible relevant field** cannot be
unpacked from outside its module even when it is `Unpack` — you can neither name
that field (to discharge it) nor `..` it away (it is relevant), so its consuming
method stays the only exit. A type whose hidden fields are all `Discard` is fully
`..`-skippable regardless of their visibility.

## Construction by field name

A record is built with `{ field = value, ... }`. Three shorthands make this
terse:

### 1. Punning — use a local of the same name

If a field's value is just a local binding with the same name, the `= value`
may be dropped:

```tel
let x = 4
let y = 2
let p = Point { x, y }          # same as Point { x = x, y = y }
let p = Point { z = 7, x, y }   # mix explicit and punned
```

### 2. Spread — copy same-named fields from another value

`*other` inside a constructor copies every field of `other` whose name matches
a field of the type being built, except fields already given explicitly:

```tel
let new = User_v4 {
    created_ns     = created_ms * 1_000_000,
    favorite_color,                # punned from a local
    *old,                          # everything else from `old`
}
```

The rules for `*old`:

- Fields of the target given **explicitly** (or punned) take precedence;
  `*old` does not override them.
- `*old` supplies every *remaining* target field that `old` also has, **by
  name and matching type**.
- It is a **compile error if any target field is left unfilled** — e.g. if
  `favorite_color` were omitted and `old` had no such field.
- Fields present on `old` but absent from the target are **ignored**.

Crucially, `old` and the target need **not be the same type**. Copying
same-named fields *between different classes* is a first-class operation:

```tel
let summary = OrderSummary { *full_order }   # picks the overlapping fields
```

### `with`-copies

`with` produces a modified copy of an existing value:

```tel
let louder = settings with { volume = 8 }
```

It is the immutable alternative to mutation (see [Mutability](02-mutability.md)).
`x with { f = v }` yields a new value equal to `x` except for field `f`. Like
spread, `with` can also target a *different* type, copying the same-named
fields and overriding the listed ones — making cross-type conversion a
one-liner.

TODO(open): whether `*old` (spread in a fresh constructor) and `with`
(copy-update of an existing value) are two surface forms of one mechanism, or
two distinct constructs, is not settled. They overlap heavily, and both are
in play. `04-syntax` and [`10-data-modelling/01-records.md`](../10-data-modelling/01-records.md)
should converge on one story.

## Why

- **API evolution is the motivating use case.** Converting `User_v3` to
  `User_v4` should state only what *changed* and let same-named fields flow
  across automatically. Writing every unchanged field by hand is boilerplate
  and a place for mistakes.
- **Great for code generation.** A generator can wrap user code with
  `*old`-style spreads at the start and a field-named constructor at the end
  without knowing the field list — the names do the wiring.
- **Encourages precise types.** When converting between near-identical types is
  this cheap, there is no pressure to reuse one loose type everywhere; each
  stage of a pipeline can have its own exact record type.
- **Name-based, not position-based** — reordering fields in a type definition
  does not silently re-wire construction sites.

### The optional-field hazard

Name-based copying interacts badly with optional fields. If a field is
optional and the *source* field is renamed, the names stop matching and the
field is **silently dropped** to its default/`none` instead of producing an
error.

TODO(open): how to defend against silently-lost optional fields under `*old`
/ `with` across renames. Candidates: require optional target fields to be
listed explicitly when spreading from a different type, or a lint. Unresolved.

## See also

- [Records](../10-data-modelling/01-records.md) — the types being constructed.
- [Substructural Types](../12-memory-and-runtime/08-substructural-types.md#unpack--publishing-destructuring-as-the-public-use)
  — the `Unpack` capability that sanctions destructuring and, for relevant
  values, makes it a use.
- [Visibility](../11-modules-and-packages/03-visibility.md) — the field-visibility
  rules that also gate who may name a value's fields.
- [Mutability](02-mutability.md) — `with` as the immutable update model.
- [Pattern Matching](../10-data-modelling/06-pattern-matching-in-depth.md) —
  destructuring in `match`.
- [Default and Named Arguments](../09-functions/04-default-and-named-arguments.md) —
  the same name-based idea for function calls.
