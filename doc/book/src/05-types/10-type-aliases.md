# Type Aliases

A **type alias** is a second name for an existing type. Aliases are a
**documentation and readability tool**, not a way to introduce a new type:
two aliases for the same underlying type are interchangeable in every context.

```tel
type UserName = Text
type UserMap  = Map[Id[User], User]

# UserName and Text are the same type — no constructor, no conversion.
let n: UserName = "alice"
let t: Text     = n               # fine, same type
```

When you need a *new* type — one that does **not** silently mix with its
underlying type — use a [refined type / newtype](12-refined-types.md) instead.

## What aliases are for

- **Naming a long generic.** `type Histogram = Map[Bucket, Int64]` saves repeating
  the parameters and lets the meaning of the parameters carry through reads.
- **Naming an effect set.** A function-type alias bundles
  a recurring effect combination so signatures stay readable — see
  [`05-function-types.md`](05-function-types.md):

  ```tel
  # Sketch — syntax not settled. Bundles a recurring effect combination.
  type Fallible = fn + alloc + panics
  fn handle(req: Request) Fallible -> Response { ... }
  ```

- **Naming a parameterised structural type.** Aliases for tuple shapes,
  function signatures, or unions keep the call site short:

  ```tel
  type Listener = (Event) -> Unit
  type Loc      = (Real64, Real64)
  type Json     = (Null | Bool | Int64 | Real64 | Text | List[Json] | Map[Text, Json])
  ```

- **Hiding a host-runtime choice behind a name** the script controls. If the
  host exposes one of a small family of types under different names, an alias
  pins the local name the script uses.

## What aliases are not for

- **Domain distinctness.** `type EuroAmt = Decimal` as an alias does **not**
  stop a script from passing a bare `Decimal` where a `EuroAmt` is expected —
  the alias is just another name. Use `type EuroAmt = newtype Decimal` (a
  refined type) for that.
- **Changing the semantics of a type.** An alias cannot add operators, remove
  methods, or attach a constraint. Anything more than renaming is a refined
  type.

The split is deliberate: a reader who sees `type X = Y` should know exactly
that `X` and `Y` are interchangeable, with no other consequence.

## Generic aliases

An alias may take type parameters of its own and forward them:

```tel
type StringMap[V]  = Map[Text, V]
type ReqResult[T]  = Result[T, RequestError]
```

A generic alias is, again, *just* a name: `StringMap[Int64]` and `Map[Text, Int64]`
are the same type.

TODO(open): whether an alias may *partially apply* a generic — e.g.
`type IntList = List`. That edges into higher-kinded territory, which Tel does
not adopt (see [`07-generics.md`](07-generics.md)). Lean: no — an alias must
name a fully formed type, not a type constructor with holes.

## How an alias differs from a newtype, at a glance

| | `type Alias = X` | `type New = newtype X` |
|---|---|---|
| New type identity | no | yes |
| Mixes with `X` | yes | no |
| Adds a constraint | no | yes (via refined types) |
| Operators / traits of `X` | inherited as-is | inherited but applied to the new type |
| Cost at runtime | zero | zero |

See [`12-refined-types.md`](12-refined-types.md) for the newtype side.

## Aliases and inference

Aliases are erased before inference looks at them, so an inferred type prints
in terms of the underlying type unless the alias is on a *public* signature
that pins it. This has a readability consequence: an IDE displaying an
inferred type as `Map[Text, Int64]` when the codebase usually writes
`StringMap[Int64]` is mildly jarring. The recommended habit is to *annotate*
local bindings with the alias when the alias is the term the script means
("use `StringMap[Int64]` everywhere this concept appears"), even where inference
would also work.

TODO(open): whether the compiler / IDE should *prefer* an alias in displayed
types when one is in scope. Tooling concern; defer to the LSP design.

## See also

- [Refined Types](12-refined-types.md) — for distinct *new* types.
- [Function Types](05-function-types.md) — aliases as effect-set names.
- [Type Inference](08-type-inference.md).

TODO: review
