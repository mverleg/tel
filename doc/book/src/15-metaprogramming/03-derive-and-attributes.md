# Derive and Attributes

<!-- TODO: review -->

Of all the [metaprogramming](01-macros.md) ideas, **`derive`** is the one
that earns a place in Tel. It auto-generates the mechanical, error-prone
code — equality, hashing, debug-printing — that almost every data type needs
and almost no one wants to write by hand.

This is a *narrow* feature. It is not a macro system: `derive` cannot run
arbitrary code or emit arbitrary types. It expands a fixed, language-known set
of templates against a type's declared shape — and it stays that way: a
derive is a **built-in declaration form** (spelled `impl auto`), not an
annotation-processing hook, and it will never become user-programmable (see
[antifeatures](../02-philosophy/04-antifeatures.md)).

## What `derive` is

A derive asks the compiler to generate a standard implementation of a trait
for a type. It is spelled as an `impl` with the `auto` modifier and no body:

```tel
struct EuroAmt {
    cents: Int64,
}

impl auto Eq, Hash, Debug for EuroAmt
```

The shape is the point: `impl Eq for EuroAmt { ... }` says *a human wrote
this implementation*; `impl auto Eq for EuroAmt` says *the compiler's fixed
template is this implementation*. A derive **is** an impl — the ownership
rules for implementations (see
[equality and hashing](../10-data-modelling/07-equality-and-hashing.md))
apply unchanged — and the syntax makes that literal rather than analogous.
Because the auto form has no body, it accepts a comma-separated trait list
where a hand-written `impl` takes one trait. By convention a derive sits
directly after the type it belongs to. In prose, "derive" and "derived"
remain the names for the feature and its output; `impl auto` is only the
spelling.

The generated code is ordinary Tel that the compiler and IDE see normally —
there is no hidden expansion the reader cannot follow.

Two properties of this spelling are deliberate:

- **No new reserved word.** `impl` is already a keyword, and `auto` is a
  *contextual* keyword — reserved only in the modifier slot after `impl`, so
  `auto` stays usable as an ordinary identifier everywhere else. A leading
  `derive` keyword would have needed either global reservation or a fragile
  contextual rule at statement position; `impl` has already committed the
  parser to a declaration. The keyword-plus-modifier pattern matches
  `let uniq`.
- **Syntax, not an `@`-attribute.** Deleting an attribute never changes what
  a program does — attributes are advisory metadata (see
  [the cutoff](#keyword-or-attribute--the-cutoff) below). Deleting a derive
  removes an implementation, and code elsewhere stops compiling: it changes
  what the program *means*, so it gets real declaration syntax. The dedicated
  form also preserves a guarantee Rust's `#[derive]` cannot make: in Rust, a
  builtin derive and a proc-macro derive doing arbitrary work look identical
  at the use site; in Tel, anything spelled `impl auto` is — with
  certainty — one of the language's fixed templates.

Candidate derivable behaviours:

- **Equality** and **hashing** — structural, field-by-field.
- **Debug-printing** — a developer-facing string form.
- Possibly **builders** and `Immutables`-style boilerplate (these are
  "build into the language" candidates rather than user macros).

## Defaults are opt-in, never automatic

Tel does **not** auto-implement equality, hashing, or `toString` for every
type. The rule is firm: *don't provide defaults unless they are 99%
correct.* A derived `Eq` is correct for a plain value record; it is wrong for a
type with an identity, a cache field, or a normalisation rule. So `derive` is
always an explicit request — the author states "the structural default is
right here," and is responsible if it is not.

This matches *make the right thing easy and the wrong thing hard*: deriving is
one line, but it is a line you chose to write.

## Why this and not macros

`derive` is the metaprogramming Tel keeps because it stays inside the
guardrails the rest of the chapter sets:

- The set of derivable things is **fixed and language-defined** — every host
  implementation reproduces the same finite list, so stability holds.
- Expansion is **mechanical**, so clean compiles stay fast.
- The output is **predictable**, so an IDE and a reviewer can follow it.

A user-written proc-macro has none of these properties; see
[Macros](01-macros.md).

TODO(open): the exact derivable set for Tel1 is not pinned down. It must be
chosen carefully — every entry is frozen forever. `Eq` / `Hash` / `Debug` are
settled; builders are plausible; serialisation is explicitly
*not* a derive (data models are schema-first, with code generated as a separate
module — see [Macros](01-macros.md) and [Reflection](02-reflection.md)).

## Attributes

`derive` is a keyword, not an attribute. What Tel calls an **attribute** is
an `@`-spelled *advisory* marker attached to a declaration (`@allow`,
`@hot`) — metadata for tooling and the optimiser that never changes what the
program does. Tel deliberately rejects open-ended, user-defined annotation
processing (no Java-style `@Annotation` macro frameworks; see
[antifeatures](../02-philosophy/04-antifeatures.md)). The attributes Tel has
are a fixed, language-defined set.

One member is decided: **`@allow(rule, "reason")`** — lint
suppression at a declaration boundary, specified in the
[Linter](../18-tooling/07-linter.md#per-declaration-suppression-allow)
chapter. It is the archetype of what an attribute is for: purely advisory
metadata for tooling, with no effect on what the program does, parameterised
by an open set of rule names that could never be keywords.

TODO(open): which further attributes exist is unresolved. Other chapters
already imply attribute-like markers — e.g. a non-exhaustive marker on
unions ([union types](../10-data-modelling/02-union-types.md)), a
"do-not-optimise-away" marker (see
[Compile-Time Evaluation](04-compile-time-evaluation.md)). Decide whether these
share one attribute syntax and keep the full list small and language-defined.

### Keyword or attribute — the cutoff

The rule of thumb: a **keyword** is warranted when the construct changes what
the program *means* — it alters semantics, admissibility, or control flow, and
the compiler enforces it. An **attribute** carries advisory,
tooling-and-optimiser-facing metadata that leaves runtime behaviour untouched.
Two supporting arguments for keeping the attribute side big and the keyword
side small: (1) fewer reserved identifiers, so ordinary names stay usable;
(2) anything added after Tel1 freezes must arrive in attribute form anyway to
stay backwards-compatible ([priorities](../02-philosophy/01-priorities.md)),
so the convention is future-proof.

Applying the cutoff:

- derive (spelled `impl auto ... for ...`) — **declaration syntax**, decided:
  it generates implementations, so deleting it changes what the program means
  and what compiles. Costs no new reserved word — `auto` is contextual after
  the existing `impl` keyword. See [What `derive` is](#what-derive-is).
- `@allow` — attribute, decided (see above): zero semantic effect.
- `@hot` (performance-critical marker) — attribute: a hint the optimiser may
  use or ignore; the program means the same either way. Kept for now.
  `TODO(open): @hot is in tension with "no PGO hints in source" in
  [goals and non-goals](../01-overview/03-goals-and-non-goals.md). Current
  lean is that such hints are wanted after all; reconcile the goals text
  when this firms up.`
- `captures` (a closure declaring what it captures) — probably **not** an
  attribute, by this very rule: a checked capture list changes which closure
  bodies are admissible, so it is semantics, not advice. It belongs in
  closure syntax or a keyword.
  `TODO(open): earlier drafts listed @captures as an attribute candidate;
  decide its real spelling with
  [closures](../09-functions/06-closures-and-lambdas.md).`

### Declaration attributes vs type-usage attributes

One distinction is worth recording: an attribute on a **declaration**
(`@hot fn render(...)`) is different from an attribute on a
**type usage** (`fn lookup(id: @Validated Id[User]) -> ...`).

- **Declaration attributes** are consistent and per-declaration — one marker
  covers the declaration everywhere it is used. This is what `@allow` and
  `@hot` are.
- **Type-usage attributes** turn a single declaration into many: the same
  field type can be annotated differently in different signatures (`@Nonnull`
  on one parameter, plain elsewhere), and the attribute can in principle
  *transform* what is admissible at that site.

Tel leans **declaration attributes only**: the type either has a property or
it does not. The work that type-usage attributes do in Java
(`@Nonnull`-style) is done by Tel's [refined types](../05-types/) and the
contract system, both of which are first-class — a non-nullable wrapper is a
type, not an annotation.

TODO(open): confirm Tel rejects user-visible type-usage attributes outright.
The narrow exception worth weighing is something like `@uniq` on a parameter
type, which is already a separate
[mutability-model](../02-philosophy/04-antifeatures.md) question.

## IDE suggestion patterns

A related, lighter-weight idea: a way to declare, *in the
code*, a **suggested rewrite** that an IDE can offer — for example
"`filter(p)` → `keep(p)`" (where `filter` is a common guess that does not
exist), or migrating a deprecated call to its replacement.

The machinery is **structured AST matching**: a mostly-standard parser with
type-aware placeholders matches a tree shape, and a rewrite is suggested. This
is distinct from `derive` — it transforms *existing* code rather than
generating new declarations — and it is primarily a tooling feature, not a
language feature.

TODO(open): scope of IDE suggestion patterns. Open: whether they
are purely an IDE/discovery aid, or whether the same AST-matching machinery
could double as a constrained codegen step (a "structured macro" gated behind
an attribute, output not required to be type-checked — useful for code-folding
displays). This works for annotation-style transforms where the input
is valid Tel, but not for Rust-style macros. Lean: ship it as a tooling
feature first; treat the codegen use as a separate, later decision. Philosophy
does not cover IDE-directed rewrites — flag as a gap. See also
[Macros](01-macros.md#alternative-3-structured-ast-matching-transforms).

## Generators that live outside `derive`

A handful of "I want a `@MakeMapper` annotation"-style asks look like `derive`
candidates but are not. Examples:

- **Mappers between data shapes** (Java's MapStruct precedent) — given two
  data types plus a declarative field mapping, generate the conversion
  function.
- **Derived / cached properties** — `full_name` derives from `first_name +
  last_name`, with a caching policy.
- **ORM accessors, route tables, prepared statements** — generate typed
  accessors from a declarative description.

These do not belong in `derive` for two reasons. First, they take inputs
beyond the annotated type (the *other* shape, the mapping rules, the schema)
which `derive`'s "one attribute, fixed template, one type" shape cannot
express. Second, the set is open-ended — every freeze of a new derivable is
permanent, and Tel keeps that list short.

The intended path is [`std.tel_ast`](../17-standard-library/18-tel-as-data.md):
a normal Tel script reads the mapping or schema, builds a `Module`, and writes
a `.tel` file the next compile picks up. The generator lives in tooling or a
crate; the language stays small.

## See also

- [Macros](01-macros.md) — why heavyweight metaprogramming is out, and the
  "build popular macros into the language" strategy.
- [Reflection](02-reflection.md) — why schema-first codegen replaces reflective
  serialisation.
- [Compile-Time Evaluation](04-compile-time-evaluation.md) — running pure code
  at compile time.
- [`std.tel_ast`](../17-standard-library/18-tel-as-data.md) — where
  open-ended generators (mappers, ORMs, route tables) actually live.
- [Equality and Hashing](../10-data-modelling/07-equality-and-hashing.md) — the
  behaviour `derive(Eq, Hash)` generates.
