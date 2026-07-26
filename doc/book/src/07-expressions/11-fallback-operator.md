# Fallback Operator

<!-- TODO: review -->

Tel provides a short binary operator for supplying a **fallback value** when
the left-hand side is absent. Because data-transformation code constantly deals
with "use this, or else that," the operator is deliberately terse.

## What

The fallback operator takes a value that *might be missing* on the left and a
replacement on the right:

```tel
let name = lookup(id) ?? "unknown"
let port = config.port ?? 8080
```

If the left side yields a present value, that value is the result. Otherwise
the result is the right side.

- It operates on **`Option`-shaped values** — Tel has no `null`, so "missing"
  means `none` (see [Option and Nullability](../05-types/06-option-and-nullability.md)).
- The right-hand side is **lazy**: it is evaluated only when the left side is
  missing. So `expensive_default()` in `x ?? expensive_default()` does not run
  when `x` is present. This is an instance of
  [lazy arguments](06-function-application.md#lazy-arguments).
- It chains: `a ?? b ?? c` yields the first present value.

TODO(open): the operator spelling is not pinned down — the goal is only
"an easy operator for fallbacks, like `or` in Python/Bash … should be a short
one." Two candidates:

- **`??`** (used here as the running placeholder) — familiar from Kotlin, C#,
  Swift; visually distinct from logical `or`.
- **`or`** keyword — reusing the logical OR keyword as the fallback operator
  on `Option`-shaped values. Reads naturally (`name or "guest"`) and is short.
  Risk: the same keyword behaves *very* differently on `Bool` vs `Option`, and
  Tel does not want truthy/falsy coercion.

Lean: `??`. Keeping `or` for `Bool` only preserves *one good way* per type.
Confirm.

## No truthy/falsy fallback

Should fallback trigger on *all falsy values* (Python's
`or`, which falls back on `0`, `""`, empty list)? Tel says **no**: it has no
truthy/falsy coercion ([antifeatures](../02-philosophy/04-antifeatures.md)).
Fallback triggers on **absence only** (`none`), never on a present-but-"empty"
value. `0 ?? 5` is `0`. Falling back on `0` or `""` would silently discard real
data — exactly the DWIM behaviour Tel rejects.

If a script genuinely wants "empty counts as missing," it converts explicitly
(e.g. map an empty string to `none` first).

## Fallback on `Result`, not just `Option`

The same operator works on a
[`Result`](../13-error-handling/02-result-types.md): it triggers on `Err` and
yields the right-hand side, so `parse(text) ?? 0` supplies a default when
parsing fails. `Option`-`none` and `Result`-`Err` are sibling "absent" shapes;
the operator treats them the same way.

Used on a `Result`, the operator **swallows** the error — it discards the `Err`
and substitutes a value. That is a real decision, not a default one, so Tel
makes you write it: the operator *is* that explicit act, which keeps it
consistent with *an error is never dropped silently*. It is the opposite of
[error propagation](../08-control-flow/07-error-propagation.md), which forwards
the failure outward instead of replacing it here and now.

## Why

- **Terse on purpose.** Picking a default is so common in data transforms that
  a full `match` or `if` each time is noise; *error handling is explicit but
  terse* ([maxims](../02-philosophy/02-maxims.md)).
- **Lazy right side** means the default can be expensive without a cost when it
  is not needed.
- **Absence-only, not falsy** keeps the operator honest: it never throws away a
  legitimate value. The result type is the non-`Option` element type, so the
  fallback also *discharges* the optionality.

## How it looks

```tel
fn greeting(a_user: User) -> Text {
    "Hello, " & (a_user.display_name ?? a_user.login ?? "guest")
}
```

This is distinct from error *propagation* (a `?`-style operator that returns
the error to the caller) — see
[Error Propagation](../08-control-flow/07-error-propagation.md). Fallback
*replaces* a missing value here and now; propagation *forwards* a failure
outward.

## See also

- [Option and Nullability](../05-types/06-option-and-nullability.md)
- [Comparison and Logical Operators](03-comparison-and-logical.md)
- [Function Application](06-function-application.md#lazy-arguments) — lazy evaluation.
- [Error Propagation](../08-control-flow/07-error-propagation.md)
- [Fallback Operator (error-handling view)](../13-error-handling/06-fallback-operator.md)
  — how swallowing an `Err` stays consistent with never dropping an error.
