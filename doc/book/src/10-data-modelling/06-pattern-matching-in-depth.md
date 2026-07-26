# Pattern Matching In Depth

`match` is Tel's primary tool for dispatching on shape. It is what makes
[untagged unions](02-union-types.md) safe to use — exhaustive matching is what
guarantees every case is handled — and it doubles as Tel's destructuring form,
the way records, tuples, and arrays are pulled apart.

This page focuses on how `match` *works*; the dispatching-on-type story for
unions is in [`02-union-types.md`](02-union-types.md), and the basics live in
the syntax/control-flow chapter.

## What — type, then optional shape

A `match` arm has two parts: a **type pattern** that says *which variant* the
arm handles, and an optional **shape pattern** that destructures the value.

```tel
match value {
    c: Circle           => c.radius * c.radius * pi,        # bind whole value, typed
    Rectangle { w, h }  => w * h,                            # type plus field destructuring
    Triangle { base = b, height = h } => b * h / 2,         # rename while destructuring
}
```

Two spellings, one per intent:

- **Bind the whole value, typed** — `name: Type`. This is the same
  `name: Type` form used by [`let` bindings](../06-bindings-and-scope/01-let-bindings.md)
  and [parameters](../09-functions/02-parameters-and-arguments.md), so it adds
  no new syntax — `c: Circle` reads "a `Circle`, bound as `c`". Use `_: Type`
  to match a member type while binding nothing.
- **Destructure** — `Type { … }` for records; the type leads and the braces
  reach inside. A *positional* form `Type(a, b)` is **tentative**
  — `TODO(open): whether to allow positional destructuring of named-field
  types at all, or require `{ … }` so fields are always matched by name.`

The type pattern is the *primary* dispatch: a value of a union has exactly one
member type, and that picks the arm. Tel's untagged unions mean the type
literally *is* the tag — there is no separate discriminant to look at.

The shape pattern lets the arm reach inside the matched value without an extra
`let` step. Shape patterns are available for:

- **Records** — by field name. Field-name-only binds to a local of the same
  name (`{ w, h }` ≡ `{ w = w, h = h }`).
- **Tuples** — by position. `(a, b)` binds the two elements.
- **Arrays** — by position, optionally with rest-binding. `[first, ..rest]` —
  see [array destructuring](#arrays-and-headtail).
- **Refined types** — by the underlying value, with the refinement statically
  in force inside the arm.

## Why — `match` is the safety net for unions

Every place in Tel that "one of N things" matters, `match` is the construct
that handles it. Exhaustive matching is what makes unions safe to evolve: add
a member to an exhaustive union, every `match` over it fails to compile until
the new arm is added. That is the *desired* breaking change — the compiler
points at every place a decision must be made about the new case.

For unions that opt into [non-exhaustive](02-union-types.md), a `_` catch-all
arm is required and adding members is non-breaking. The match still tells you
what you decided about "everything else".

## Exhaustiveness in detail

A `match` over an exhaustive union must cover every member type. Coverage is
*by type* — `(Circle | Rectangle | Triangle)` requires three arms, one per
member. Coverage is checked statically; a missing arm is a compile error with
the missing type named.

Several niceties fall out:

- **`Never` arms.** A union narrowed by an earlier check may collapse to a
  smaller set. An arm whose type is impossible at that point is unreachable;
  the compiler may either complain or accept it (lean: complain — dead arms
  are a smell).
- **Shared-field access without a `match`.** When *every* member of a union
  has the same field of the same type, that field is accessible on the union
  directly (see [`02-union-types.md`](02-union-types.md)). No `match` needed
  for the common case.
- **`match` as an expression.** Every arm produces a value of the same type;
  the `match` itself is that value. Combined with the
  [never type](../05-types/14-never-type.md), an arm that aborts does not
  disturb the inferred result type.

```tel
let label: Text = match shape {
    _: Circle    => "round",
    _: Rectangle => "boxy",
    _: Triangle  => "pointy",
}
```

## Refined types in patterns

When the matched value is a [refined type](../05-types/12-refined-types.md),
the refinement stays in scope inside the arm. A `match` on a `Result[T, E]` —
`(Ok[T] | Err[E])` — gives the inner branches `T` and `E` directly.

There is also a powerful intended interaction with **constraint
narrowing**: matching on a structural condition can introduce a narrower
type *inside* the arm. The canonical example: matching `x: Real64` against
`> 0` makes `x` a `Real64 > 0` inside that arm, so the arm can call into
functions whose signatures require positivity without a checked conversion.

```tel
# Sketch — syntax not settled.
match x {
    > 0    => sqrt(x),          # x: Real64 > 0 inside this arm
    == 0   => 0.0,
    < 0    => -sqrt(-x),
}
```

TODO(open): how far Tel's `match` narrows refined types. The simple case
(matching a literal against a unit-type variant) is uncontroversial; the
constraint-narrowing case ties into the open
[constraint-propagation question](../05-types/12-refined-types.md) and may be
limited to a built-in set of predicates.

## Arrays and head/tail

Tel explicitly wants **head/tail destructuring on lists**, ML-style:

```tel
match items {
    Empty           => "nothing",
    [first]         => "one",
    [first, ..rest] => "many, starting with " & show(first),
}
```

The mechanics:

- `[a, b, c]` matches a list of exactly three elements and binds them.
- `[a, ..rest]` matches a non-empty list, binds the first to `a` and the rest
  (as a list of the same element type) to `rest`.
- `[..init, last]` symmetrically matches "everything except the last".
- `Empty` matches the empty list explicitly.

This is the syntactic form; behind the scenes a list is a recursive type (see
[`05-recursive-types.md`](05-recursive-types.md)) and the head/tail pattern is
just `Cons` destructuring with a friendlier spelling.

TODO(open): exact spelling of head/tail. `[first, ..rest]`, `first :: rest`,
or both. Lean: bracket-spread (`[first, ..rest]`) — it matches the array /
tuple destructuring style and avoids introducing a new operator. The
ML/Haskell `:: ` form is concise but unfamiliar to most target audiences.

## Guards

A pattern may carry a **guard**: an additional condition that must hold for
the arm to fire.

```tel
match shape {
    r: Rectangle if r.w == r.h => "square",
    r: Rectangle               => "oblong",
    _: Circle                  => "round",
    _: Triangle                => "pointy",
}
```

Guards do not contribute to exhaustiveness — the compiler treats a guarded arm
as not necessarily covering its type, so a fall-through arm is needed. Guards
should be the exception, not the norm; when a guard is doing real work, the
distinction usually deserves a refined type or a separate variant.

TODO(open): final spelling of guard (`if expr`, `where expr`). Lean: `if`,
familiar to most readers.

## Nested patterns

Patterns nest: a tuple of records, a record with a union-typed field, a
recursive type — all may be matched in one shape.

```tel
match (mode, response) {
    (Quiet, _)            => Unit,
    (Verbose, Ok { body }) => print(body),
    (Verbose, Err e)      => warn(e.message),
}
```

This is where `match` earns its weight — destructuring a multi-level value in
one expression beats a stack of `if let` checks both for readability and for
exhaustiveness reasoning.

## `match` and `if let` shortcuts

For the common case of "if it is *this* variant, do something, otherwise skip"
a long `match` is overkill. Tel offers an `if let` /
`when let` form, the way Rust and Kotlin do:

```tel
if let Some u = find_user(id) {
    greet(u)
}
```

This is a *shortcut*, not a different mechanism — it desugars to a
single-arm `match` plus a default. The exhaustiveness check does not apply,
because the form is explicitly "match or skip".

TODO(open): final spelling (`if let`, `when`, `match let`). Lean: `if let`
for Rust/Kotlin familiarity.

## Bindings, defaults, and `_`

- `_` is a wildcard pattern that matches anything and binds nothing. Used in
  catch-all arms, and to ignore parts of a destructured value.
- `name` (a bare identifier) binds the matched value (or component) to a
  local of that name, of the corresponding type.
- `{ field = pattern, .. }` matches a record by some of its fields; `..`
  ignores the rest.

The shape of these is intentionally close to the construction syntax —
construction and destructuring read symmetrically.

## Pattern matching as the destructuring primitive

`let` bindings can also use patterns, not just `match`. A `let` with a
pattern is equivalent to a single-arm `match` that aborts on mismatch — the
irrefutable cases (records, tuples, arrays of fixed
length, the single-variant case) just work without a `match`:

```tel
let (q, r)            = divmod(17, 5)
let Point { x, y }    = origin
let [first, ..rest]   = items     # aborts if items is empty
```

For potentially-refutable patterns, `let` requires a fallback or aborts; `if
let` / `match` is the explicit form.

TODO(open): whether `let` with a refutable pattern is a compile error (forcing
`if let`/`match`) or aborts at runtime. Lean: compile error — surprises here
have caused real bugs in other languages, and "compile errors should teach"
per the [maxims](../02-philosophy/02-maxims.md).

## What Tel does *not* do

- **No active patterns / view patterns** (F#-style user-defined patterns). They
  add real expressiveness but turn pattern matching into a partial-function
  language — the compiler can no longer reason about exhaustiveness or
  reachability once any arm may be an arbitrary user predicate; rejected for
  [one good way](../02-philosophy/02-maxims.md). The recommended alternative is
  to [decompose the value and match on it directly](#extracting-from-stringly-data)
  — binding captures in the arms — and to lift the result into a typed union
  when it needs to be a value.
- **No catch-all that silently extends an exhaustive union.** Adding a `_` arm
  to an exhaustive `match` works, but the user has now opted out of the
  compile-time check for *this* `match`; the union as a whole stays
  exhaustive.

## Extracting from stringly data

Tel has no user-defined capturing patterns (no regex-that-binds, no F#
`(|Route|_|)` extractor — see [above](#what-tel-does-not-do)). To turn an
unstructured value such as a URL path into bound, typed fields, **decompose it
into a shape `match` already understands** — for a path, a list of segments —
and bind the captures directly in the arms. Literal segments select the arm;
bare names capture. The whole `match` is the function's tail expression, so its
value is the result — no `return`, no intermediate type:

```tel
fn dispatch(req: Request) -> Response {
    match (req.method, req.path) {
        ("GET",  ["policies"])               => Response { status = 200, body = list_policies() },
        ("GET",  ["policies", id])           => Response { status = 200, body = get_policy(id) },
        ("POST", ["policies", id, "claims"]) => Response { status = 201, body = open_claim(id, req.body) },
        _                                    => Response { status = 404, body = "no route" },
    }
}
```

`id` is captured by ordinary [array destructuring](#arrays-and-headtail) — a
literal segment (`"policies"`) selects; a bare name (`id`) binds. This relies on
the request exposing its path as a segment list rather than a raw string.

### When to name the route: parse, then match

Keep parsing and dispatch merged like that while the example is simple. Lift the
parsed route into its own **closed union** only when it needs to be a *value* —
reused across handlers, tested on its own, or produced by one layer and consumed
by another. That is the *parse, don't validate* form: the refutable "might not
match" work happens once, in a function returning a typed `Route`, and the
dispatch `match` downstream is total and dispatches on type.

```tel
type Route =
    | PolicyList
    | PolicyById { id: Text }
    | OpenClaim  { id: Text, body: Text }
    | NotFound

fn parse_route(req: Request) -> Route {
    match (req.method, req.path) {
        ("GET",  ["policies"])               => PolicyList,
        ("GET",  ["policies", id])           => PolicyById { id },
        ("POST", ["policies", id, "claims"]) => OpenClaim { id, body = req.body },
        _                                    => NotFound,
    }
}

fn dispatch(req: Request) -> Response {
    match parse_route(req) {
        PolicyList             => Response { status = 200, body = list_policies() },
        PolicyById { id }      => Response { status = 200, body = get_policy(id) },
        OpenClaim { id, body } => Response { status = 201, body = open_claim(id, body) },
        NotFound               => Response { status = 404, body = "no route" },
    }
}
```

Naming the route buys exhaustiveness on the dispatch side — add a `Route` member
and `dispatch` fails to compile until its handler exists — and isolates all the
stringly parsing in one place, with the capture now a plain typed field
(`PolicyById { id }`). For a one-off router, the single merged `match` above is
the simpler answer.

## See also

- [Union Types](02-union-types.md) — the main consumer of `match`.
- [Recursive Types](05-recursive-types.md) — head/tail and tree matching.
- [Records](01-records.md) — record destructuring.
- [Tuples and Arrays](../05-types/04-tuples-and-arrays.md) — positional
  destructuring.
- [Refined Types](../05-types/12-refined-types.md) — narrowing inside arms.

TODO: review
