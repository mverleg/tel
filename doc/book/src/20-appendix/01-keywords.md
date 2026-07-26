# Keywords Reference

A consolidated list of Tel's reserved keywords. Each entry links to the
chapter that defines its meaning. Many spellings are not yet pinned down —
see the linked chapters for the open questions.

## Reserved at all times

These keywords are reserved everywhere and cannot be used as identifiers.

| Keyword | Meaning | See |
|---------|---------|-----|
| `fn` | function declaration / lambda | [Function Declaration](../09-functions/01-function-declaration.md) |
| `let` | immutable binding | [Let Bindings](../06-bindings-and-scope/01-let-bindings.md) |
| `uniq` | unique (exclusive, mutable) binding modifier | [Mutability](../06-bindings-and-scope/02-mutability.md) |
| `const` | top-level constant / compile-time constant | [Constants](../06-bindings-and-scope/03-constants.md) |
| `local` | binding strictly local to the current scope (blocks accidental outer-scope reference) | [Let Bindings](../06-bindings-and-scope/01-let-bindings.md) |
| `if` / `else` | conditional expression | [If Expressions](../08-control-flow/01-if-expressions.md) |
| `match` | exhaustive pattern matching | [Match Expressions](../08-control-flow/02-match-expressions.md) |
| `for` / `in` | iteration | [For Loops and Iteration](../08-control-flow/04-for-loops-and-iteration.md) |
| `while` | conditional loop | [While Loops](../08-control-flow/03-while-loops.md) |
| `loop` / `break` / `continue` | infinite loop and loop control | [Loop and Break](../08-control-flow/05-loop-and-break.md) |
| `return` | early function return | [Early Return](../08-control-flow/06-early-return.md) |
| `struct` | record declaration | [Records](../10-data-modelling/01-records.md) |
| `type` | type alias / union / refined-type declaration | [Type Aliases](../05-types/10-type-aliases.md), [Union Types](../10-data-modelling/02-union-types.md), [Refined Types](../05-types/12-refined-types.md) |
| `trait` | trait / interface declaration | [Traits or Interfaces](../10-data-modelling/03-traits-or-interfaces.md) |
| `impl` | trait implementation | [Traits or Interfaces](../10-data-modelling/03-traits-or-interfaces.md) |
| `where` | type-parameter constraint, predicate on a refined type | [Generics](../05-types/07-generics.md), [Refined Types](../05-types/12-refined-types.md) |
| `import` / `export` | bring names into scope; declare a crate's outgoing API (`export { … }`, no `from`) | [Imports](../11-modules-and-packages/02-imports.md), [Visibility](../11-modules-and-packages/03-visibility.md) |
| `private` | visibility marker — default is open (crate-visible); `private` restricts to a module and its children | [Visibility](../11-modules-and-packages/03-visibility.md) |
| `with` | copy-update for records | [Records](../10-data-modelling/01-records.md), [Mutability](../06-bindings-and-scope/02-mutability.md) |
| `test` | test declaration / test-only visibility | [Testing](../14-testing/01-testing.md) |
| `and` / `or` / `not` | logical operators (word form) | [Comparison and Logical Operators](../07-expressions/03-comparison-and-logical.md) |
| `is` | type-test (pattern match outside `match`) | [Match Expressions](../08-control-flow/02-match-expressions.md) |
| `true` / `false` | the two `Bool` values | [Primitive Types](../05-types/02-primitive-types.md) |
| `abort` / `todo` | loud-failure stand-ins | [Panics and Aborts](../13-error-handling/04-panics-and-aborts.md), [Result Types](../13-error-handling/02-result-types.md) |

`TODO(open): final keyword names.` Several spellings are not settled
(`fn` vs `fun`, the *outer-scope-write* keyword, the keyword that picks `Eq` and
`Hash` via `derive`). The visibility marker is settled as `private`
(default-open, see [Visibility](../11-modules-and-packages/03-visibility.md)) and
the `api`/`impl`/`executable` dependency classification is pending its final
keyword. When a chapter pins a spelling down, update this table.

## Reserved for future use

Tel **reserves generously**: a keyword it may want later
must not be usable as an identifier now, or its eventual introduction is a
breaking change. The same notes propose pre-reserving these
even-though-currently-unused keywords. Any of them is rejected as an
identifier:

- `async`, `await`, `yield` — explicitly rejected for the *language*
  ([antifeatures](../02-philosophy/04-antifeatures.md)), but reserved so a
  host can use them in capability-typed surface code without colliding
  with a user identifier.
- `lazy` — possible future opt-in for lazy evaluation; see the open
  question in [Function Application](../07-expressions/06-function-application.md).
- `derive` — the
  [`derive`-style attribute](../15-metaprogramming/03-derive-and-attributes.md)
  spelling.
- `super` / `self` — `self` is the receiver in method syntax; `super` is
  reserved to keep the door open without committing to a meaning.
- `default` — reserved for switch-statement catch-all spelling, even
  though `_` is the current candidate.
- `static` — reserved against `static`-style scope decisions; Tel
  currently has no `static`.

`TODO(open): firm reserved-keyword list.` This needs to be updated once
the full keyword set is pinned across the linked chapters. Until then,
parsers should treat the list above as the canonical set of identifiers
that must not be used as names.

## See also

- [Operators and Punctuation](../03-lexical-structure/06-operators-and-punctuation.md)
- [Operator Reference](02-operator-reference.md)
- [Identifiers](../03-lexical-structure/03-identifiers.md)
