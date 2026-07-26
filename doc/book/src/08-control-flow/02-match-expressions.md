# Match Expressions

`match` is Tel's primary branching construct for "this value is one of several
possibilities." It dispatches on which member of an [untagged
union](../10-data-modelling/02-union-types.md) a value is, or compares a value
against a list of patterns. Like `if`, it is an **expression** and produces a
value.

## What it is

A `match` lists *arms*. Each arm has a pattern and a body; the first arm whose
pattern matches the value is taken, and its body's value becomes the value of
the whole `match`.

```tel
let message = match outcome {
    Ok(score)        -> "scored " & score.to_text(),
    Err(Reject.Late) -> "too late",
    Err(other)       -> "rejected: " & other.to_text(),
}
```

Because a value's *type* is its tag in an untagged union, `match` dispatches on
type. Matching `Ok(score)` both selects the `Ok` member and binds its payload.

## Exhaustiveness

A `match` over a union **must cover every member**. If an arm is missing, the
script does not compile. This is the *invalid states unrepresentable* maxim
applied to control flow: there is no silent fall-through, no forgotten case
discovered only when a user hits it.

Exhaustiveness is **opt-out per union**, not per `match`. A union may declare
itself *non-exhaustive*, which means downstream `match`es over it must include a
catch-all arm. This exists so a library can add a member to its own union
without breaking every caller — adding a member to an *exhaustive* union is a
breaking change, which is sometimes what you want (your own error enum) and
sometimes not (a library's). See
[union types](../10-data-modelling/02-union-types.md) and
[`13-error-handling/02-result-types.md`](../13-error-handling/02-result-types.md).

A catch-all arm uses a wildcard or a plain binding:

```tel
match status {
    Status.Active   -> handle_active(),
    Status.Paused   -> handle_paused(),
    _               -> handle_other(),   # required only for non-exhaustive unions
}
```

## `return` from inside an arm

An arm body may `return` from the enclosing function. Tel deliberately **does
not copy Java's restriction** that forbids `return` inside a switch expression.
Java's rule buys a little local clarity but costs a common, readable pattern;
the priority *one good way over many clever ones* favours letting `match` arms
do what any other block can do.

```tel
fn classify(outcome: Result[Score, Reject]) -> Score {
    match outcome {
        Ok(score) -> score,
        Err(_)    -> return Score.zero(),   # early return straight out of the arm
    }
}
```

The same applies to `break` and `continue` when the `match` is inside a loop:
they target the nearest enclosing loop, not the `match`. See
[early return](06-early-return.md) and [loop and break](05-loop-and-break.md).

## Why `match` and not a chain of `if`

- The compiler proves every case is handled — an `if`/`else if` chain cannot be
  checked for completeness.
- It dispatches on type without manual `is`-checks and narrowing.
- Adding a member to an exhaustive union turns every `match` into a compile
  error that points at exactly the code that must be updated.

## Patterns

The pattern language is not fully pinned down. The settled intent:

- **Type binding** — `name: Type` matches a union member by its type and binds
  the whole value, typed, to `name`. It reuses Tel's `name: Type` form from
  [`let` bindings](../06-bindings-and-scope/01-let-bindings.md) and
  [parameters](../09-functions/02-parameters-and-arguments.md), so there is no
  new syntax to learn — `p: Person => greet(p)`. Use `_: Type` to match the
  type while binding nothing.
- **Destructure patterns** — `Type { field, … }` (records) match a member and
  reach inside it. A positional `Type(a, b)` form is **tentative** (see the
  open question in
  [Pattern Matching In Depth](../10-data-modelling/06-pattern-matching-in-depth.md)).
- **Wildcard** — `_` matches anything and binds nothing.
- **Binding** — a bare name matches anything and binds it (untyped catch-all).

Nested/destructuring patterns and guard clauses (`Ok(n) if n > 0 -> ...`) are
described in [Pattern Matching In Depth](../10-data-modelling/06-pattern-matching-in-depth.md#nested-patterns)
(see also its [Guards](../10-data-modelling/06-pattern-matching-in-depth.md#guards)
section).

TODO(open): literal patterns (matching against `0`, `"text"`) are the remaining
undescribed case. Decide whether bare literals are match patterns and how they
interact with refined-type narrowing.

TODO: review — this is a new large section; the exact arm syntax (`->` vs `=>`
vs `:`) and brace rules are placeholders pending the syntax chapter.
