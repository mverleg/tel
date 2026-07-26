# Recursion

<!-- TODO: review -->

A *recursive* function calls itself, either directly or through a chain of
other calls. Tel allows recursion and treats it as a normal use of the
function-call machinery — there is no special "recursive function" keyword in
the common case — but the language is careful about two things: making the
*compile-time resolution* of the name work, and naming the few places where
recursion needs an opt-in (mostly for performance or for tail-call
guarantees).

## What

```tel
fn factorial(n: Int64) -> Int64 {
    if n <= 1 { 1 } else { n * factorial(n - 1) }
}
```

A function may call itself by name. Both
**forward references and recursion are allowed without special syntax** — the
compiler resolves variable references in a separate pass after the AST is
built, so a function may refer to names that are defined later in the file
(or to its own name) without an extra declaration.

`TODO(open): forward declaration.` Forward declarations are not required.
Confirm that this holds across modules and for mutually
recursive top-level functions.

## Mutual recursion

```tel
fn is_even(n: Int64) -> Bool {
    if n == 0 { true } else { is_odd(n - 1) }
}
fn is_odd(n: Int64) -> Bool {
    if n == 0 { false } else { is_even(n - 1) }
}
```

Two functions that call each other are no different from a function that
calls itself — the same name-resolution pass sees both before either body is
checked.

## Local / inner recursion

Whether a *local* (block-scope) function may be recursive is open. A
function defined as a `let` binding cannot, in general, reference its own
name before the binding is initialised:

```tel
let count_down = fn(n: Int64) -> Int64 {
    if n <= 0 { 0 } else { count_down(n - 1) }   # may not see `count_down` yet
}
```

`TODO(open): named-recursive lambda form.` Some inputs lean toward a Grain /
ML-style explicit *recursive binding* (`let rec f = ...`) so the body sees
the name. Others would just say "use a top-level function." Lean: introduce a
small marker only if local recursion turns out to be common — otherwise push
recursive helpers to the file's top level, where they are constants and the
name is visible everywhere.

## Recursion versus iteration

Tel ships ordinary loops (`for`, `while` — see
[Control Flow](../08-control-flow/)) *and* recursion. The choice between them
is a style call, but the language pushes mildly toward iteration for plain
list-walking and toward recursion for genuinely tree-shaped problems.

There is even an open question whether `for` should exist at all, given `for_each` and
iterator combinators ([Higher-Order Functions](07-higher-order-functions.md)).
The answer it lands on is: keep `for` — familiarity wins, stack traces and
debugging are better, and a tight loop should not require a closure
allocation per iteration in slow interpreters.

## Tail-call optimisation (TCO)

A tail call is a call in *return position* — its result is immediately the
caller's result. Optimising tail calls into a jump eliminates stack growth
for tail-recursive code and lets functional-style state machines run without
risk of stack overflow.

`TODO(open): tail-call guarantees.` Split decision on whether Tel
*guarantees* TCO, *opts into* it per call site (so the programmer flags "this
call must be eliminated, fail to compile if it cannot"), or leaves it to the
implementation. Lean: a per-call opt-in marker, because:

- A *guaranteed* TCO across all hosts is hard (JS hosts in particular have
  given up on it).
- A *silent* TCO can mask a real stack-overflow bug if a refactor moves the
  call out of tail position.

A per-call marker — say, a postfix annotation or a keyword on the call —
gives both clarity at the call site and an error the moment the optimisation
would silently break. Concrete spelling deferred to the optimiser-hints
discussion in [Tooling](../18-tooling/).

## Bounded / unbounded recursion

Recursion can be thought of as a *resource* the language can sometimes
bound. Two related ideas:

- A function whose maximum recursion depth is **known at compile time** could
  be exempted from "may stack-overflow" warnings. Useful for traversals of
  bounded-depth ASTs.
- A function annotated *no virtual calls* could be checked for "no recursion
  at all" — useful in performance-sensitive code and as a guarantee that
  upstream changes don't introduce unbounded recursion.

`TODO(open): recursion-depth checking.` Both of these are speculative; they
also rely on no-virtual-call restrictions that are themselves open. Lean: do
not ship in 1.0. A function that cares writes the loop explicitly. Revisit
once trait-dispatch and inlining stories are pinned down — see
[Traits or Interfaces](../10-data-modelling/03-traits-or-interfaces.md).

## Stack overflow vs panic

A deeply recursive call that exceeds the host's stack is a *runtime error*,
not undefined behaviour. The behaviour matches the rest of the safety
story: fail loudly, at a defined boundary, with a clear message — see
[Antifeatures](../02-philosophy/04-antifeatures.md) and the error-handling
chapter.

## Why

- **Recursion is just calling a function** — no special declaration, no
  separate semantics. Mirrors *one good way*.
- **Forward references work** because name resolution is a separate pass
  ([Lexical Structure](../03-lexical-structure/)). Order of declarations in a
  file does not matter for correctness.
- **No silent TCO** — keep the optimisation opt-in so removing it (e.g. by
  reformatting that moves a call out of tail position) is a compile error,
  not a quietly slower program.

## See also

- [Function Declaration](01-function-declaration.md)
- [Closures and Lambdas](06-closures-and-lambdas.md) — capturing a recursive
  binding.
- [Higher-Order Functions](07-higher-order-functions.md) — iterator
  combinators as the iterative alternative.
- [Control Flow](../08-control-flow/) — loops.
- [Antifeatures](../02-philosophy/04-antifeatures.md) — no undefined
  behaviour on stack overflow.
