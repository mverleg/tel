# The Never Type

Tel has a **never type** (the *bottom* type): the type with *no* values. It is
the declared return type of a function that never returns normally — one that
always aborts, loops forever, or hands control away. It is the dual of the
[unit type](02-primitive-types.md#the-unit-type) (one value that says nothing)
and gets its own chapter because its rules — uninhabitedness and the
subtype-of-everything coercion — stand apart from the ordinary value-carrying
primitives.

```tel
fn unreachable() -> Never { abort("should not happen") }
```

## No instance can ever be constructed

`Never` is **uninhabited**: there is no literal, no constructor, and no
expression that *produces* a `Never`. The only way to "reach" the type is to
not produce a value at all — by diverging. `abort(...)`, an infinite `loop`, a
`return`/`break` that leaves the current expression, a `panics` call that ends
the task: each has nowhere to hand a value back, so it is typed `Never`. You
can name the type and pass it around in signatures, but you can never hold one.

This is what distinguishes `Never` from the unit type, which the two are easy to
confuse: the unit type has *one* value that carries no information; `Never` has
*zero* values and cannot be produced at all. A value of `Unit` tells you
nothing; a value of `Never` cannot exist to tell you anything.

## A `Never` expression fits anywhere

`Never` is **special in one way**: an expression of type `Never` is assignable
to — usable in — a context expecting *any* type `T`. `Never` is a subtype of
every type.

```tel
# `abort` returns Never, so it satisfies the `Int64` arm too — the code
# after it is unreachable, so there is no value to type-check.
let n: Int64 = match parse(text) {
    Ok(v)  => v,
    Err(_) => abort("bad input"),
}
```

This is sound precisely *because* `Never` is uninhabited: the assignment can
never actually run (the producing expression diverges first), so letting a
`Never` expression stand in for any `T` cannot ever yield a wrong-typed value.
There is nothing to coerce — only unreachable code.

This coercion rule is the *only* magic `Never` needs. Uninhabitedness itself is
structural (a type with no constructible values), and recognising that
`abort`/`loop`/`return` diverge is ordinary reachability analysis the compiler
already does for match exhaustiveness and dead-code checks. So `Never` behaves
like an ordinary uninhabited stdlib type plus the single language rule
"a provably-uninhabited value is usable as any type" — it is not a heavily
blessed primitive.

TODO(open): final name for the never type. Candidates seen: `Never` / `!` /
`Nothing`. Keep a name that a Python/Java/Rust reader finds unsurprising — per
the familiarity priority. Settle alongside the unit type's name (see
[Primitive Types](02-primitive-types.md#the-unit-type)).

## See also

- [Primitive Types — the unit type](02-primitive-types.md#the-unit-type) — the
  dual: one value vs zero values.
- [Subtyping and Variance](09-subtyping-and-variance.md) — `Never` as the
  bottom type.
- [Function Types](05-function-types.md) — `-> Never` as a diverging return.
- [Return Values](../09-functions/03-return-values.md) — functions that never
  return.
- [Pattern Matching In Depth](../10-data-modelling/06-pattern-matching-in-depth.md)
  — an aborting arm contributes no value.

TODO: review
