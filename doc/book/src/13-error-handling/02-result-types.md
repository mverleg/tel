# Result Types

A function that can fail returns a **`Result`-shaped type**: a value that is
*either* a success carrying the normal return value *or* a failure carrying an
error. This is how Tel represents [errors as values](01-philosophy.md).

## The shape

`Result` is not a magic built-in; it is an ordinary
[union type](../10-data-modelling/02-union-types.md) over two wrapper structs:

```tel
struct Ok[T](T)
struct Err[E](E)

Result[T, E] = (Ok[T] | Err[E])
```

A function declares it can fail by returning a `Result`:

```tel
fn lookup_rate(table: RateTable, region: Region) -> Result[Rate, LookupErr] {
    match table.get(region) {
        some(rate) -> Ok(rate),
        none       -> Err(LookupErr.UnknownRegion),
    }
}
```

The caller cannot use the `Rate` without first confronting the `LookupErr` —
the value is an `Ok` *or* an `Err`, and only a [`match`](../08-control-flow/02-match-expressions.md)
or the [propagation operator](03-error-propagation.md) gets the inner value out.

## Why wrapper structs and not a bare union

Tel's unions are **untagged** — a value's type *is* its tag. A bare
`(T | E)` union has two problems that the `Ok`/`Err` wrappers solve:

- **Disambiguation.** If `T` and `E` were ever the same type — `Result[Int64, Int64]`
  — a bare `(Int64 | Int64)` is just `Int64`, and "success" and "failure" become
  indistinguishable. Wrapping makes `Ok[Int64]` and `Err[Int64]` genuinely distinct
  types. This case is exactly where the wrapping earns its keep.
- **Methods.** A nameable `Ok`/`Err` pair gives a place to hang helpers like
  `map`, `map_err`, and the [`todo`-style methods](#unimplemented-and-the-todo-helpers)
  below. An untagged `(T | E)` has no such handle.

This is the same reason `Option` is `(some(T) | none)` rather than `(T | none)`.
See [union types](../10-data-modelling/02-union-types.md).

## `Result` and `Option`

- **`Result[T, E]`** — *succeeded with a `T`* or *failed with an `E`*. Use it
  when the failure has information worth carrying.
- **`Option[T]`** — *present* (`some(T)`) or *absent* (`none`). Use it when the
  only "failure" is absence and there is nothing more to say. There is no
  `null` in Tel; `Option` is how optionality is expressed.

A missing lookup is naturally an `Option`; a rejected order is naturally a
`Result` with a reason. Converting between them — `Option` to `Result` by
attaching an error, `Result` to `Option` by discarding the error — should be
cheap and explicit.

## Exhaustive matching and non-exhaustive error unions

Matching a `Result` is [exhaustive](../08-control-flow/02-match-expressions.md):
the compiler checks that both `Ok` and `Err` are handled.

The error type `E` is often itself a union of specific failures:

```tel
LookupErr = LookupErr.UnknownRegion | LookupErr.StaleTable | LookupErr.RateOutOfBounds
```

Whether such an error union should be **exhaustive** is a real design choice:

- An *exhaustive* error union lets every caller `match` each variant — but then
  adding a new failure mode is a **breaking change** for every caller.
- A *non-exhaustive* error union lets a library add failure modes later;
  callers must include a catch-all arm.

Tel makes this opt-out per union. For a library's public error type, lean
non-exhaustive (it can grow); for a script's own internal error enum,
exhaustive is fine and the compile errors on a new variant are a feature.

## Unimplemented, and the `todo` helpers

Tel provides a `todo` facility for code that is not written yet — useful while
sketching a script, and a clean alternative to a fake return value.

- A top-level **`todo` function** that, when called, immediately
  [aborts](04-panics-and-aborts.md) the task with an "unimplemented" message. It
  stands in for any expression: `let rate = todo()`. It type-checks as any type,
  so a half-written function still compiles. (The name has no hyphen — it is a
  plain identifier.)
- **`todo`-style methods on `Option` and `Result`** that extract the inner
  value but abort if the value is the *bad* case — `none`, or `Err`. This is the
  "I have not handled this path yet, fail loudly if it happens" tool.

```tel
let rate = lookup_rate(table, region).todo()   # aborts on Err — placeholder
```

These are deliberately abort-based, not error-returning: a `todo` marks code
that *should not run yet*. Reaching one is a bug in the program's state, which
is the [abort](04-panics-and-aborts.md) category, not the expected-error
category.

TODO(open): exact names. There is a top-level `todo` "that just throws"
plus "todo-like methods on Option and Result." Decide the method spelling —
`todo()`, `unwrap()`, `expect(msg)` — and whether there is also a non-aborting
`or_default`-style helper. Note Tel's terminology is *abort*, not *throw*; the
docs use *abort* (see [panics and aborts](04-panics-and-aborts.md)).

TODO: review — new section; helper naming is the main open point.
