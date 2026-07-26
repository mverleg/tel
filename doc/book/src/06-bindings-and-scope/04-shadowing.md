# Shadowing

Tel **allows shadowing, but only at an explicit binding site** — a `let`
binding, a function or lambda parameter, a `match`/destructuring pattern, or a
loop variable. A **bare assignment never shadows**: `name = value` either
reassigns a binding already in the current scope or declares a brand-new local
when the name is free, and it is an *error* when the name exists only in an
enclosing scope (see [Scoping Rules](05-scoping-rules.md)).

So the rule has two halves:

- **`let` (and other binding sites) may shadow** an outer name.
- **A bare assignment may not** — it is a reassignment or a fresh declaration,
  never a new binding that hides an outer one.

## What

```tel
let total = compute()
{
    let total = 0        # OK — `let` introduces a new, inner `total`
    total                # the inner one
}
total                    # the outer one, unchanged
```

```tel
let total = compute()
{
    total = 0            # ERROR — `total` exists only in an outer scope.
                         # Write `let total = 0` to shadow it, or
                         # `outer total = 0` to reassign the outer one.
}
```

Shadowing is permitted **across a scope boundary** (an inner block, a `match`
arm, a lambda body). Re-declaring a name **in the same scope** is still an
error — that reads like a typo, not an intent:

{{#spec SAME_SCOPE_REDECLARATION}}

```tel
let x = 1
let x = 2     # ERROR — same scope. Did you mean `x = 2` (reassign)?
```

## Why `let` is the shadow marker

`let` is otherwise optional in Tel (see [Let Bindings](01-let-bindings.md)): a
fresh local can be introduced by a bare `name = value`. Reserving `let` for the
cases that *introduce* a name — and forbidding a bare assignment from shadowing —
makes `let` a **signal that appears exactly where a visible name changes
meaning**, and nowhere else. A reader scanning a block never has to wonder
whether `name = value` quietly created a second `name`: it cannot. If a name is
being re-introduced, the `let` is right there.

This keeps the maxim *if it looks correct, it is correct* without the cost of the
earlier blanket ban. The previous rule forbade shadowing outright, which forced
unrelated renames and made common patterns awkward:

- unwrapping an optional and reusing the same name once the optional one is dead;
- a nested sub-logger that rebinds `log` for its block;
- a validated value that should hide the raw input it was derived from.

Those patterns are now expressible. What stays banned is the *confusable* form —
a bare assignment that silently hides an outer binding. (For the "hide the raw
input" case, note that [affine/linear types](../12-memory-and-runtime/08-substructural-types.md)
are the stronger tool: consuming the raw value makes it inaccessible under *any*
name, where shadowing only hides one name in one scope.)

## The invariant that keeps it safe

Because a bare assignment never reaches an enclosing scope, it can only ever
touch a binding **declared in the current scope** or introduce a **fresh** one.
An outer-scope name can therefore never be silently re-used by a bare
assignment — doing so is a compile error that points at `let`. Combined with
[immutability by default](02-mutability.md):

> An outer-scope name is never re-used implicitly. Re-using it is always
> written — `let` to shadow it, `outer` to reassign it.

## Where shadowing comes up

A name reused in **sibling** (non-overlapping) scopes was always fine and still
is — those scopes never see each other:

```tel
if cond { let x = 1; ... } else { let x = 2; ... }   # OK: the two `x` never coexist
```

Other explicit binding sites shadow under the same rule, because they
*introduce* names just as `let` does:

- **Parameters.** A lambda parameter may reuse an outer name; this is how a
  nested sub-logger works — `log.sub("import") |log| { ... }` rebinds `log` for
  the block (see [Observability and Logging](../17-standard-library/14-observability-and-logging.md)).
- **Patterns.** `match` arms and [destructuring](06-destructuring.md) bind fresh
  names that may shadow.
- **Loop variables.** `for x in xs { ... }` introduces a fresh `x` each
  iteration (see [Scoping Rules](05-scoping-rules.md)).

## See also

- [Let Bindings](01-let-bindings.md) — when `let` is optional and when it is required.
- [Scoping Rules](05-scoping-rules.md) — the write rule and reaching outer scopes.
- [TIP-0001](../tips/0001-mutability-and-borrowing.md) — the binding-surface decision.
