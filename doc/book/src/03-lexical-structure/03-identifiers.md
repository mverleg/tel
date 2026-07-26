# Identifiers

An **identifier** names a binding, function, type, field, or parameter. Tel
identifiers follow the conservative shape that a reader from Python, Java, C#,
Rust, JS, or Kotlin already expects: a letter or underscore followed by letters,
digits, or underscores.

{{#spec IDENTIFIER_SHAPE}}

```tel
score          my_age_days     EuroAmt     User_v4     _unused
```

## What is settled

- Identifiers **are case-sensitive**: `score` and `Score` are distinct names,
  and casing carries meaning (e.g. `Self` vs `self`).
- **But a single compilation unit may not declare two identifiers in the same
  namespace that differ only in case.** Case-sensitivity is kept for meaning,
  yet two *same-namespace* names that differ *only* in casing are rejected at
  compile time: two types `HttpServer` and `HTTPServer` (or `Id` and `ID`), or
  two values `user_id` and `userid`-style near-twins. Telling them apart on
  sight is error-prone, and a diff reviewer should never have to squint — this
  is *readability over writability*, and it costs almost nothing because such
  near-collisions are nearly always a mistake.

  The rule is **scoped to one namespace** precisely so the universal idiom
  survives: a type and a value that differ only by the conventional initial cap
  — `let person = Person(...)` — are in *different* namespaces (a type position
  vs a value position tells them apart), and that pairing is not just allowed
  but encouraged. Case there is the *intended* signal of *type vs value*, the
  one distinction case is reserved for (see
  [style guide](../20-appendix/03-style-guide.md)). Because types are
  `UpperCamelCase` and values `snake_case`, a within-namespace case-twin is hard
  to write by accident anyway; the rule is a backstop for the acronym-casing
  cases above.

  `TODO(open): also reject identifiers differing only by confusables like I/l/1?
  scope of confusable set undecided.` The same collision-avoidance argument
  extends to visually-confusable characters — capital `I` vs lowercase `l` vs
  digit `1`, `O` vs `0`, and so on. Whether to forbid declarations that differ
  only by such confusables, and exactly which character set counts as
  "confusable", is not yet decided.
- Keywords (see [Keywords](04-keywords.md)) are reserved and cannot be used as
  identifiers.
- A name binds to exactly what it says. Tel deliberately keeps name resolution
  *local and literal*: there are **no import aliases that rename another
  module's members**. A member keeps the name it was defined with, under its own
  module — so you can rely on a name meaning what it reads as, without scanning
  the import list. (Importing a whole module under an alias to resolve a clash
  is a separate, allowed thing — see
  [Modules](../11-modules-and-packages/). A fully-qualified name is the fallback
  when two names would otherwise collide.)

## Why this shape

*Readability over writability.* A line of Tel is read far more often than it is
written, frequently by a different person or an AI reviewing a diff. Identifiers
that look like identifiers in every mainstream language keep the surface
unsurprising, and forbidding rename-on-import means a name can be understood
without first reconstructing what the importer aliased it to.

## Open questions

`TODO(open): allow non-ASCII identifiers? DSL value vs one-way-to-do-things.`
Should Tel source permit internationalized (non-ASCII) identifiers, or be
ASCII-only at the surface? Both sides have real weight:

- **Pro.** Tel is *embedded* and often hosts domain-specific scripts; letting
  domain experts name things in their own language can make a DSL read far more
  naturally to its intended authors. This is the embedding USP at work.
- **Con.** It cuts against *"one obvious way to do things"* and against
  portability: the same concept can now be spelled in multiple scripts, near-look-alike
  identifiers across scripts reopen the confusable-collision problem
  above, and a reviewer from another team may be unable to read or type the
  name.

String literals and comments are unaffected either way and may contain any
UTF-8 — this question is only about *identifier* characters. If non-ASCII
identifiers are allowed, the same-compilation-unit collision rules (case and
confusables) need extending to cover cross-script look-alikes. Decide, and pin
down the exact permitted character set.

TODO: review
