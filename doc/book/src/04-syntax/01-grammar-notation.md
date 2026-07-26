# Grammar Notation

This documentation describes Tel's syntax through **prose and worked examples**,
not a formal grammar. That is a deliberate choice: the book is meant to be clear,
not to be a specification. Where a small grammar fragment makes a rule
unambiguous, this chapter uses an informal BNF-style notation inline; it is
illustrative, not normative.

## What the grammar is required to be

Even though the documentation is informal, the *grammar itself* is constrained
by Tel's priorities, and those constraints are firm:

- **Fixed, bounded lookahead — concretely, LL(1).** The most-recent notes are
  explicit: *"The syntax should be left-recursive with lookahead of 1, to be
  easy and fast to parse."* The grammar must be parseable with one token of
  lookahead and no unbounded backtracking. Fast, predictable compilation with
  clear errors is a stated priority, and it is also what lets many
  independently-maintained host implementations parse Tel identically.
- **Name resolution is a separate pass.** Both forward references and
  recursion are allowed without special syntax (and the AST is immutable);
  therefore resolving variable references happens *after* the AST is built,
  not during parsing. This keeps the parser free of type and name lookups —
  the lexer and parser need only the source itself.
- **Conflict resolution by priority is acceptable.** An LL-style parser
  (LALRPOP / "LLARP"-class tooling) where grammar conflicts are resolved by a
  fixed priority order is fine. What is *not*
  acceptable is a grammar whose meaning depends on type information or on
  arbitrary backtracking.
- **No precedence climbing across mixed operators.** Tel has
  [no cross-operator precedence](04-precedence-and-associativity.md): mixing
  *different* binary operators requires explicit parentheses (the narrow
  exceptions — a repeated same operator, the additive `+`/`-` group, and
  boolean/comparison chains — associate left-to-right and need no ladder). So
  the expression grammar never needs a full precedence ladder, removing a whole
  class of grammar ambiguity.

Several smaller syntax decisions exist *specifically* to keep the grammar within
these limits, for example:

- `[]` rather than `<>` for generics, so the lexer never has to disambiguate a
  generic bracket from a comparison operator.
- A `:` between a name and its type, so the parser always knows whether it is
  reading a name or a type.
- A leaning toward **requiring a comma** between arguments, because it keeps
  argument lists parseable with fixed lookahead.
- A **leading `.` before union members in patterns** (`match x { .None => ... }`)
  so the parser can always tell a fresh binding from an existing variant
  without a name-resolution step — see
  [Match Expressions](../08-control-flow/02-match-expressions.md).
- A **`\n.` combined token** so the leading-`.` chaining rule does not require
  unbounded lookahead — see
  [Whitespace and Newlines](../03-lexical-structure/08-whitespace-and-newlines.md).

## A worked fragment

The design notes include a fragment that shows how method chaining, the
short `\`-lambda, and binary expressions layer — and, by its awkwardness, why
the terminator-less `\`-lambda is under review:

```text
Expr        = NlDot
NlDot       = NlDot "\n." ShortClosure | ShortClosure
ShortClosure = "\" ShortClosure | JustDot
JustDot     = JustDot "." BinaryExpr | BinaryExpr
BinaryExpr  = ...
```

Here `"\n."` is a *single combined token* — see
[Whitespace and Newlines](../03-lexical-structure/08-whitespace-and-newlines.md)
— a workaround forced by the lack of a lambda terminator.

`TODO(open): final grammar shape.` The fragment above is exploratory. It depends
on unresolved questions: whether the `\`-lambda survives (see
[Expressions vs Statements](02-expressions-vs-statements.md)), and the final
chaining and newline rules. The complete grammar is not settled — this chapter
describes pieces as the feature chapters pin them down. Whether the book ever
ships a single consolidated formal grammar (e.g. in the
[appendix](../20-appendix/)) is itself open.

TODO: review
