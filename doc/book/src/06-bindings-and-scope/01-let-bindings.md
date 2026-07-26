# Let Bindings

<!-- TODO: review -->

A *binding* attaches a name to a value. A binding is **immutable by default**:
once a name is bound, it keeps its value for the rest of its scope. A binding is
introduced either by a bare assignment to a fresh name (`count = 3`) or
explicitly with `let`. **`let` is optional** — it is required only when the
binding **shadows** an outer name or carries a **modifier** such as `uniq` (see
below).

## What

```tel
let count = 3
let label = "orders"
let total: EuroAmt = EuroAmt(0)
```

- The name on the left is bound to the value of the expression on the right.
- A binding is **always initialised** where it is introduced. There are no
  uninitialised variables and no `null` — see
  [antifeatures](../02-philosophy/04-antifeatures.md).
- A type annotation (`: EuroAmt`) is optional where the compiler can infer the
  type and required where it cannot. Tel's inference is deliberately local —
  type information flows one way — so a binding's type is fixed the moment it
  is introduced.

By default a binding is immutable. The full set of forms:

- **`name = expr`** — a bare assignment. When `name` is not yet in scope this
  declares a fresh *immutable* local. (When `name` is already declared in the
  current scope it is a reassignment instead; see
  [Scoping Rules](05-scoping-rules.md).) This is the lightest form and the
  common case for short code.
- **`let name = expr`** — an explicit immutable binding. Identical to the bare
  form for a fresh name, but **required** when the binding **shadows** a name
  from an enclosing scope (see [Shadowing](04-shadowing.md)) — a bare assignment
  is not allowed to shadow.
- **`let uniq name = expr`** — a *unique/mutable* binding (see
  [Mutability](02-mutability.md)). A **modifier such as `uniq` requires `let`**.
- **`name: T = expr`** — a type-annotated binding; the `:` marks it as a
  declaration just as `let` does, so an annotated binding is always a fresh one.

```tel
count          = 3             # bare — fresh immutable local
let limit      = 100           # explicit; identical here, but needed to shadow
let uniq tally = 0             # mutable — the `uniq` modifier requires `let`
total: EuroAmt = EuroAmt(0)    # annotated declaration
```

So **`let` is optional, and carries information precisely because it is**: it
appears only where a binding **shadows** an outer name or takes a **modifier**.
Everywhere else a bare assignment suffices. This was settled in
[TIP-0001](../tips/0001-mutability-and-borrowing.md) (binding surface) together
with the [shadowing](04-shadowing.md) rule. It replaces both the earlier
"mandatory `let`" lean and the separate `local` shadow-escape keyword, which is
no longer needed — `let` *is* the shadow opt-in.

The parser always knows whether it is reading a declaration or an assignment: a
leading `let`, or a `:` type annotation, marks a declaration; a bare `name =`
is resolved against the current scope (reassign-or-declare) at name-resolution
time, exactly as Python does.

## Why

- **Immutable by default** matches the maxim that *mutability's scope should be
  minimized* ([maxims](../02-philosophy/02-maxims.md)). The common case — a
  name that never changes — needs no extra ceremony, and the rare case that
  does is visibly marked.
- **Always initialised** removes a class of bugs. A reader never has to scan
  upward to learn whether a name has a value yet.
- **One name, one meaning *within a scope*.** A bare assignment cannot hide a
  visible name (see [Shadowing](04-shadowing.md)), so a name's meaning only ever
  changes where an explicit `let` says so — and that `let` is right there to see.

## How it looks

```tel
fn describe(an_order: Order) -> Text {
    let id = an_order.id
    let when = an_order.placed_at
    # `id` and `when` cannot be reassigned; they are facts about this order.
    "order " & id & " placed " & when
}
```

A binding can hold any value, including a function — functions are values
bound to names like anything else (see
[Function Declaration](../09-functions/01-function-declaration.md)).

```tel
let double = fn(x: Int64) -> Int64 { 2 * x }
double(21)   # 42
```

## See also

- [Mutability](02-mutability.md) — the `uniq` keyword and the immutable-plus-copy model.
- [Constants](03-constants.md) — top-level bindings, which are always constant.
- [Scoping Rules](05-scoping-rules.md) — where a binding is visible.
- [Destructuring](06-destructuring.md) — binding several names at once.
