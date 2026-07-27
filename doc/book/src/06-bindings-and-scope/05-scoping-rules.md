# Scoping Rules

<!-- TODO: review -->

Scope is the region of source over which a name refers to a particular
binding. Tel's scoping is **lexical** (block-structured) and its **write
rule** is Python-like: an assignment declares in the *current* scope unless a
keyword says otherwise.

## What

### Reading a name

To resolve a name being *read*, Tel looks outward: the innermost enclosing
scope that declares the name wins. Because [shadowing](04-shadowing.md) is
forbidden, at most one enclosing scope can declare any given name, so this
lookup is unambiguous.

### Writing a name

A bare assignment `name = value` resolves as follows:

- `name` is **already declared in the current scope** → reassignment (legal only
  if that binding is `uniq` — see [Mutability](02-mutability.md)).
- `name` is **declared only in an enclosing scope** → **compile error**. A bare
  assignment never reaches outward and never shadows. Write `let name = value`
  to introduce a new inner binding ([Shadowing](04-shadowing.md)), or
  `outer name = value` to reassign the enclosing one.
- `name` is **not declared anywhere visible** → declares a fresh immutable local
  in the current scope.

{{#spec BARE_ASSIGN_REASSIGNS_OR_DECLARES}}

So a write never *silently* touches an outer-scope binding: reaching out is
always explicit. To deliberately reassign a binding from an enclosing scope, an
explicit keyword is required:

```tel
let uniq total = 0
for an_item in a_cart {
    outer total = total + an_item.price   # explicitly targets the outer `total`
}
```

TODO(open): the keyword that "reaches the outer scope" is not named in the
input — it describes the behaviour ("a keyword can expose the outer scope",
"essentially similar to Python", cf. Python's `nonlocal`/`global`). `outer` is
used here as a placeholder. The exact spelling, and whether it can reach more
than one level out, are unresolved.

This is the write half of the [shadowing](04-shadowing.md) rule: shadowing is
allowed, but only through an explicit binding (`let`, a parameter, a pattern),
never through a bare assignment. That is why `name = value` against an
outer-only name is an error rather than a silent new binding — the programmer is
forced to say which they mean: `let` to shadow, `outer` to reassign.

## Why

The driving concern is that *renaming an outer variable should not silently
change what an inner block does*. Two rejected alternatives:

- **Nearest-scope-wins for writes** — an inner block writing `total` would
  reach out and clobber an outer `total` the moment the names happen to match.
  Renaming is dangerous at a distance.
- **Conflict-only failure** — safe against the above, but makes a function's
  compilation depend on names declared far away; small edits cause spooky
  breakage.

Tel's rule — *a bare write reassigns in the current scope or declares a fresh
name, and reaching an outer scope is always explicit (`outer` to reassign, `let`
to shadow)* — keeps naming **local and predictable**. You can read a block and know which bindings it
creates and which it touches, without scanning the whole file. This serves the
maxim *if it looks correct, it is correct*.

## Scope depth

Scopes nest with blocks: function bodies, `if`/`match`/loop arms, and bare
`{ ... }` blocks each introduce a scope. Unlike call-stack depth, **scope depth
is known at compile time**, so the compiler resolves every name statically; no
name lookup happens at runtime.

## Loop iterations introduce fresh bindings

A `for` (or `while-let`) loop binding is **fresh in each iteration**, not a
reused slot that successive iterations overwrite. The iteration variable
`for item in items { ... }` declares a new `item` for every pass.

This matters most for closures built inside a loop: a lambda that captures
`item` captures the *value* of `item` for that iteration, not a shared cell
that later iterations also write to. The classic "all my handlers print the
last name" bug from JS and Python 2 is therefore not constructible in Tel —
see [Closures and Lambdas](../09-functions/06-closures-and-lambdas.md#the-loop-variable-capture-trap-and-why-tel-avoids-it).

## Reaching the outermost (file / module) scope

`TODO(open): leading `.` for absolute name lookup.` This borrows a
convention from Google's protobuf — *"a leading `.` (for example,
`.foo.bar.Baz`) means to start from the outermost scope instead."* The intent
is to bypass nested scopes and the local imports of a function and look up a
name from the file (or module) root. This is useful when an inner scope has
shadowed a familiar name and you want to be explicit, but it overlaps with the
`outer` keyword above and with [Modules](../11-modules-and-packages/) absolute
imports. Decide whether Tel needs both `outer` and `.name` (probably not —
`outer` covers stack-style "one frame up", and qualified module names cover
absolute lookups).

## How it looks

```tel
fn classify(scores: List[Int64]) -> Text {
    let uniq high = 0          # declared in the function scope
    let uniq low  = 0
    for s in scores {
        if s >= 50 {
            outer high = high + 1   # reach the function-scope `high`
        } else {
            outer low = low + 1
        }
        let seen = s      # fresh, local to the loop body — fine
    }
    "high " & high & ", low " & low
}
```

## See also

- [Shadowing](04-shadowing.md) — when an inner name may reuse an outer one (`let` only).
- [Mutability](02-mutability.md) — only `uniq` bindings may be reassigned.
- [No Global Mutable State](07-no-global-mutable-state.md) — the outermost
  scope holds only constants.
