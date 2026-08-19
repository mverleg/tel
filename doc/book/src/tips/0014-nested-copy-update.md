# TIP-0014: Nested copy-update without nesting

**Status:** Draft

**Created:** 2026-08-19
**Touches:** `06-bindings-and-scope/06-destructuring.md` (§"`with`-copies", the
`*old`-vs-`with` open question), `06-bindings-and-scope/02-mutability.md`
(§"Copy-with-update: `with`"), `10-data-modelling/01-records.md`
(§"Copy-update (`with`)", record invariants),
`07-expressions/07-field-and-index-access.md` (no implicit setter; the
field-vs-getter ambiguity), `04-syntax/04-precedence-and-associativity.md`
(where `with` sits), `11-modules-and-packages/03-visibility.md` (field
visibility per hop), [TIP-0001](0001-mutability-and-borrowing.md) (the owned
form as the alternative to copying).

<!-- TODO: review -->

## Summary

`with` is a binary operator on **one** value, so updating a field two levels
down forces the writer to nest the operator and to name the path twice:

```tel
let fresher = context with { user = context.user with { last_active = now } }
```

Breadth is already cheap; **depth** is the whole problem. This TIP proposes a
package of three changes that keep a nested update on one flat line, and
records the alternatives:

1. **Chaining is left-associative** — `a with { … } with { … }` (settles a
   question the book never answers, and removes the sibling case entirely).
2. **Dotted paths on the left of `=`** —
   `context with { user.last_active = now }`, desugaring to the nested form
   with **one rebuild per level**.
3. **`it` names the source inside the block** — for updates that read the old
   value, without colliding with punning.

Plus a non-syntactic answer that caps how much the syntax has to carry: when a
value is edited deeply and often, build it in the **owned form** and freeze
once.

## What Tel says today

- A record is immutable, and the copy-update form is infix: `x with { f = v }`
  yields a new value equal to `x` except for `f`
  ([records](../10-data-modelling/01-records.md#copy-update-with),
  [destructuring](../06-bindings-and-scope/06-destructuring.md#with-copies)).
  It may target a *different* type, copying same-named fields.
- There is **no implicit setter**: `point.x = 3` does not mutate a field of an
  arbitrary value
  ([field access](../07-expressions/07-field-and-index-access.md)). Sugar that
  rewrote `a.x = 1` into `let a = a with { x = 1 }` was considered and
  **dropped** (2026-08-19) — it reintroduces setter-shaped syntax for what is
  really a rebinding, which is the confusion the antifeature list avoids.
- Mutation happens only through a `uniq` binding on an owned `!T`, and freezing
  is one-directional ([mutability](../06-bindings-and-scope/02-mutability.md)).
- Records may carry **invariants** checked at construction
  ([records](../10-data-modelling/01-records.md)).
- Nothing in the book says a path may appear on the left of `=` inside a `with`
  block, and nothing says whether `with` chains.

## The problem precisely

Two distinct costs, and they compound with depth:

- **The braces nest.** Every level adds one `with { … }` around the last.
- **The path is written twice.** `context.user` appears as both the target
  field and the source of the inner copy.

At three levels the expression stops being readable, which is where the
pressure to reach for a mutable model comes from:

```tel
# today
let fixed = order with {
    shipping = order.shipping with {
        address = order.shipping.address with { postcode = pc },
    },
}
```

There is a third, quieter cost. Writing two sibling updates by hand invites
rebuilding the same intermediate twice:

```tel
let c = context with { user = context.user with { last_active = now } }
let c = c       with { user = c.user       with { visits = c.user.visits + 1 } }
```

That builds two `User` values, and — because construction is where a record's
invariant is checked — it checks the invariant against a **transient** `User`
that the program never meant to exist. Any flat form has to say what it does
here, not just how it looks.

## Prior art

- **F# 8** added *nested* record copy-and-update: `{ x with A.B = 10 }`,
  precisely because the nested form was the standing complaint about an
  otherwise-loved construct. The closest match to what is proposed here, from
  the same immutable-record starting point.
- **Elixir** has `put_in(ctx.user.last_active, now)` / `update_in(…, fun)` as
  **macros** that expand at compile time to the nested puts. Same shape, one
  layer out: the path is syntax, not data.
- **Clojure** has `assoc-in` / `update-in` with a *runtime* key vector. Flat
  and general, but the path is data — nothing checks at compile time that the
  keys exist. Un-Tel: the compiler cannot see which fields a call site writes.
- **Haskell / Scala optics** (`lens`, Monocle) make paths first-class values
  and compose beautifully, at the cost of a second vocabulary for data access.
- **Swift** gets `context.user.lastActive = now` for free from value semantics
  plus mutating setters — the ergonomics Tel wants, obtained the one way Tel
  has ruled out.
- **Kotlin** `data class` `copy()` nests exactly like Tel's `with`, and the
  ecosystem's answer is an optics library. A cautionary example: the pain is
  real enough to spawn a dependency.
- **Elm** (`{ r | x = 1 }`), **OCaml** (`{ r with x = 1 }`), **Rust**
  (`..base`), and **Gleam** all stop at one level. Elm rejected both nesting
  and lenses and told users to flatten their models instead — a live option,
  but one that pushes the shape of the data around to suit the syntax.

The split is clean: the languages that made the path *syntax* (F#, Elixir) kept
static checking; those that made it a *value* (Clojure, optics) bought
composability with a second concept. Tel should take the first.

## Proposed direction

### 1. `with` chains, left-associatively

```tel
let c = context with { retries = 0 } with { trace_id = id }
```

`with` is a postfix operator on the value to its left, so a chain stays flat
and reads in application order. This costs nothing to specify and removes the
sibling case from the problem. `with` needs a stated place in
[precedence](../04-syntax/04-precedence-and-associativity.md) regardless, since
the book's rule is that there is no ladder across operator kinds.

### 2. Dotted paths on the left of `=`

```tel
let fresher = context with { user.last_active = now }
```

The desugaring rebuilds **each intermediate exactly once**, so sibling updates
under one subtree group:

```tel
context with { user.last_active = now, user.visits = 41, retries = 0 }

# ≡
context with {
    user = context.user with { last_active = now, visits = 41 },
    retries = 0,
}
```

The grouping is load-bearing, not an optimisation: one rebuild per level means
**one invariant check per level**, so the transient half-updated `User` of the
hand-written version never exists.

Rules that come with it:

- **Records only, all the way down.** Every hop must be a stored field whose
  type is a record. A hop through an `Option`, a union member, or a
  `Map`/`List` index is a compile error (see open questions).
- **Stored fields, not computed members.** On the left of `=` a
  zero-arg-function member is meaningless, so a path hop resolves to stored
  fields only. That is a welcome constraint on the [field-vs-getter
  question](../07-expressions/07-field-and-index-access.md#field-access-versus-function-call):
  the write side gets an answer while the read side is still open.
- **Visibility per hop.** A path requires exactly the access the hand-written
  nested form requires, at every level. `with { user.last_active = now }` must
  not launder [visibility](../11-modules-and-packages/03-visibility.md).
- **No punning of paths.** `with { user.last_active }` stays an error; punning
  is a shorthand for a *local name*, and `user.last_active` is not one.
- **Cheap to parse.** The left side is `IDENT ('.' IDENT)*` before `=`, decided
  with lookahead 1 — no change to the
  [LL(1) strategy](../04-syntax/01-grammar-notation.md).

### 3. `it` names the source inside the block

Most nested updates are *relative* (`visits + 1`), which today means writing
the full path again inside a `with` on the root. Bind the source value as `it`,
matching the implicit-`it` convention of value lambdas
([TIP-0010](0010-lambda-receivers-and-builder-dsls.md)):

```tel
context with { user.visits = it.user.visits + 1 }
```

Deliberately **not** "the source's fields are in scope in the block"
(`with { visits = visits + 1 }`): that collides with punning, where
`with { user }` means *take the local named `user`*. Field-scoping would turn
every pun into a silent identity no-op. `it` keeps both, and keeps bare-name
resolution lexical — the same conclusion TIP-0010 reached for receivers.

### 4. Deep and often? Change ownership, not syntax

Not a chaining idea, but it bounds the syntax's job. A value edited deeply in a
loop should be built owned and frozen once
([TIP-0001](0001-mutability-and-borrowing.md)):

```tel
let uniq c = !Context { *template }
c.user.age = 2
let context = c.finish()
```

The caveat belongs next to it: freezing is one-directional, so this applies
only when the value **starts** owned. It is the answer for a hot loop, not for
touching one field of a `Context` you were handed.

## Considered: an inner `with` inside the block

```tel
let fresher = context with { user with { last_active = now } }
```

The braces still nest, but the path is written once and the language keeps
**one** construct instead of two spellings. It parses unambiguously (after a
field name, `=` versus `with` decides), and it composes to any depth without
repeating a prefix — which the status quo cannot do.

Kept as the fallback if dotted paths overreach. The reason to prefer paths: at
depth three the inner-`with` form still stacks three sets of braces, and it
offers no natural place to express the one-rebuild-per-level grouping that the
invariant argument needs.

## Rejected

- **A bare `{ … }` on the right** —
  `context with { user = { last_active = now } }`, meaning "copy-update the
  current value". Braces are for **bodies**, never for data
  ([blocks](../04-syntax/03-blocks.md#braces-are-for-bodies-not-data)); bare
  braces meaning "copy-update" is a third reading, and worse than a keyword.
- **First-class optics** — `Lens[Context, Instant]` as a value. Composable,
  but it is a second language for data access, and it hides which fields a
  call site touches. Against *readability over writability*.
- **String or symbol paths** — `update_in("user.last_active", now)`. The
  compiler cannot see the write, so visibility and existence go unchecked.
- **Setter sugar** — `context.user.last_active = now` rewriting to a
  rebinding. Removed from the field-access chapter on 2026-08-19: it reads as
  mutation of a shared value, and Tel's mutability story is that mutation is
  visible in the source.
- **"Flatten your records instead"** (the Elm answer). Rejected because Tel
  pushes the other way — cheap cross-type copying exists precisely so each
  stage can have its own exact record type
  ([destructuring](../06-bindings-and-scope/06-destructuring.md#why)); telling
  users to flatten would undo that.

## On acceptance — documentation to update

- `06-bindings-and-scope/06-destructuring.md` — the §"`with`-copies" section
  gains paths, chaining, and `it`. This also touches the standing
  `*old`-vs-`with` open question: if a path form lands on `with` only, the two
  constructs stop being interchangeable and that question mostly answers
  itself.
- `10-data-modelling/01-records.md` — the copy-update section, plus the
  one-check-per-level rule next to record invariants.
- `06-bindings-and-scope/02-mutability.md` — copy-with-update stays the
  default model; add the "deep and often ⇒ owned form" pointer.
- `04-syntax/04-precedence-and-associativity.md` — `with` as a
  left-associative postfix operator.
- `07-expressions/07-field-and-index-access.md` — a line saying the immutable
  way to change a *nested* field is a path in a `with`-block.
- Spec anchors once settled: one for the per-level rebuild rule, one for the
  visibility-per-hop rule. Not before — an anchor is a promise.

## Open questions

TODO(open): indexing in a path — `with { items[0].qty = 2 }`. It needs an
answer for a missing key that `with` does not currently have (error? no-op?
`Result`?), and a separate one for `Map` versus `List`. Deferred, not rejected.

TODO(open): a path that crosses an `Option` or a union member. "Error" is the
proposed rule, but the natural reading of `with { user.address.city = c }`
where `address` is optional is *update it if present*, which is exactly the
silently-lost-field hazard the destructuring chapter already worries about.

TODO(open): cross-type `with` plus paths. `with` may target a different type;
does a path on the left name fields of the *target* while the unlisted
same-named subtree still flows from the *source*? Probably yes, but the
interaction with the optional-field hazard needs writing out.

TODO(open): whether an update that reads the old value deserves a dedicated
form (`with { user.visits <- |v| v + 1 }`) rather than `it`. `it` is one
concept fewer; the arrow avoids repeating the path and is what `update_in`
exists for in Elixir and Clojure.

TODO(open): does the `it` binding shadow an enclosing lambda's `it`, or is
`with` disallowed inside a lambda body that uses the implicit `it`? Same class
of question TIP-0010 settled for receivers, and it should get the same answer.
