# Function Declaration

<!-- TODO: review -->

A function crates a piece of behaviour behind a name and a typed signature.
In Tel a function is **a value bound to a name** — there is no separate
"function declaration" concept distinct from a binding.

## What

```tel
fn double(arg: Int64) -> Int64 {
    2 * arg
}
```

- Parameters are typed. The return type follows `->`.
- The body is a [block expression](../07-expressions/08-block-expressions.md):
  its value is the value of its last expression, so a one-expression function
  needs no `return`.
- A top-level `fn` is just a [constant](../06-bindings-and-scope/03-constants.md)
  whose value is a function. That is what makes it safely importable: the
  binding can never be reassigned.

Because a function is a value, these two forms are the same thing:

```tel
fn double(arg: Int64) -> Int64 { 2 * arg }
let double = fn(arg: Int64) -> Int64 { 2 * arg }
```

A function used as an expression — a [lambda](06-closures-and-lambdas.md) — is
the same construct without a name.

TODO(open): keyword spelling (`fn` vs `fun` — both are in play) is not
pinned down; `fn` is used here for consistency. Settle in `03-lexical-structure`.

`TODO(open): `=` for function body, Hare-style.` Could a
function whose body is a single expression use `=` instead of `{ ... }`,
matching Hare's `fn double(x: Int64) -> Int64 = 2 * x`. Pros: less brace noise for
one-liners, and `=` already means "the value of this name is …" Cons: it
overloads `=` (which is also assignment and named-argument syntax), it does
not extend to multi-statement bodies, and the brace form is already as terse
as `{ 2 * x }`. Lean: keep one body form (`{ ... }`), reject the `=` shortcut.

## Types on signatures

Tel is statically typed. The relevant rule:

- **Public functions require explicit parameter and return types.** A signature
  others depend on must be spelled out, not inferred — inference is an easy way
  to break backwards compatibility, and an explicit signature is documentation.
- Types on a small local helper may be inferable, but Tel's inference is local
  and one-directional, so annotations are often still needed.

TODO(open): whether the `pub`/public distinction even applies to an embedded
language is itself an open question in
[`02-philosophy/03-features.md`](../02-philosophy/03-features.md). The rule
above is "explicit types on anything other code depends on"; the exact trigger
follows the visibility decision.

## Pre- and post-conditions

A function's signature can carry **contracts**: a `requires` clause is a
*pre-condition* on the arguments, and an `ensures` clause is a *post-condition*
on the result. They sit between the signature and the body, so a reader sees
the promise before the implementation.

```tel
fn abs(x: Int64) -> Int64
    ensures result >= 0
{
    if x >= 0 { return x }
    -x
}
```

- `requires <cond>` must hold of the arguments at every call — the checked form
  of *"this function assumes …"*. Where the same obligation cannot be proven and
  is stated in prose for a reviewer instead, it is a
  [caller requirement](../18-tooling/07-linter.md#requirements-on-callers).
- `ensures <cond>` must hold of the return value (named `result`) on every path
  out. The `abs` post-condition says the result is never negative, whichever
  branch produced it.

Contracts are part of the **type story**, not bolt-on asserts: the compiler
**checks them statically when it can prove them** and inserts a **runtime check
at the boundary when it cannot** (the assertion spectrum in
[`../17-standard-library/19-testing-utilities.md`](../17-standard-library/19-testing-utilities.md#compile-time-assert-and-require)).
A contract the compiler can *disprove* is a compile error; one it cannot settle
either way becomes a guard that fires loudly if violated
([panics and aborts](../13-error-handling/04-panics-and-aborts.md#provable-panics-warn-do-not-reject)).

This is the function-level member of a family that also includes
[refined-type constraints](../05-types/12-refined-types.md) on values and
[record invariants](../10-data-modelling/01-records.md) on data; together they
let *"rule out the bug at construction or at the boundary"* replace *"remember
to check."* The design-by-contract policy and its open questions live in
[`../02-philosophy/03-features.md`](../02-philosophy/03-features.md).

`TODO(open): exact spelling (`requires`/`ensures` vs `pre`/`post`), whether a
post-condition may reference the *original* argument values (an `old(...)`-style
form for relational guarantees), and the rule that contract expressions call
only [pure](../05-types/05-function-types.md#effects-belong-on-the-function-type)
functions — they must, or a contract could itself do I/O. Lean:
`requires`/`ensures`, pure-only contract expressions, and `old(...)` only if a
concrete case demands it.`

## Why

- **Functions are values** — uniform with the rest of the language, and the
  basis of [closures](06-closures-and-lambdas.md) and
  [higher-order functions](07-higher-order-functions.md).
- **Top-level functions are constants** — see
  [Constants](../06-bindings-and-scope/03-constants.md); a host or importer
  sees a stable, immutable thing.
- **Explicit signatures** serve stability and readability — a frozen language
  cannot afford signatures that drift with inference.

## How it looks

```tel
fn score(an_order: Order, a_clock: Clock) -> Result[Score, Reject] {
    if an_order.total <= EuroAmt(0) {
        return Err(Reject.NonPositiveTotal)
    }
    let age = a_clock.now().days_since(an_order.placed_at)
    Ok(Score.from(an_order, age))
}
```

## See also

- [Parameters and Arguments](02-parameters-and-arguments.md)
- [Return Values](03-return-values.md)
- [Closures and Lambdas](06-closures-and-lambdas.md)
- [Let Bindings](../06-bindings-and-scope/01-let-bindings.md)
- [`const` Functions](../15-metaprogramming/04-compile-time-evaluation.md)
