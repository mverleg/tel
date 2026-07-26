# Imports

<!-- TODO: review -->

An **import** makes the names of one [module](01-modules.md) available inside
another. Tel's import system is deliberately small: it resolves names and
nothing more.

The keyword is **`import`**. The earlier alternatives `use` and `depends` were
considered and dropped: `import` pairs naturally with the **`export`** side that
declares a module's outgoing surface (see
[crate export block](03-visibility.md#crate-export-block)), so the two
directions read as mirror images rather than two unrelated words.

## What an import does

An import brings a module into scope so its public members can be named:

```tel
import pricing.fx

fn quote(amt: EuroAmt) -> UsdAmt {
    pricing.fx.convert(amt, "USD")
}
```

A member is always reached through its own module path. Member `convert` of
module `pricing.fx` is `pricing.fx.convert` — never re-homed under some other
name.

## No import aliases

Tel has **no import aliases** — there is no `import x as y`. A fully-qualified
name is globally unique within a registry, so two imports can never produce the
same FQN and there is never a clash to alias away. A name therefore means
exactly what it says wherever it appears.

```tel
import market.fx
import bank.fx

fn spread(a: market.fx.Rate, b: bank.fx.Rate) -> Bp { ... }
```

TODO(open): **conflict with philosophy.** [`03-features.md`](../02-philosophy/03-features.md)
currently describes import aliasing (`import b.c as a`) as the supported way to
resolve a name conflict. The newest input explicitly reverses this: *no
aliases, use fully-qualified names if ambiguous.* This file follows the newer
input; `03-features.md` needs updating to match (a philosophy-chapter edit,
outside this chapter's scope).

TODO(open): if a fully-qualified name is the only disambiguator, decide how
*deep* a qualified name must be — the full path from the crate root, or just
enough segments to be unambiguous. Lean: enough to be unambiguous, since the
full path can be long.

## Importing published modules

A module published as a [crate](04-packages.md) named `X` is imported under
the path `X.path` — the crate name is the root segment of every path into
it. There is no separate "crate name vs module name" indirection: the
published name *is* the import root.

```tel
import datetime.calendar   # `datetime` is a published crate

fn next_workday(d: Date, cal: datetime.calendar.Holidays) -> Date { ... }
```

### The fully-qualified shape: `namespace . crate . module . member`

A name is `namespace . crate-path . module-path . member`: a single flat
[namespace](10-workspaces.md#namespaces), then the crate's (possibly dotted,
purely-lexical) name, then the (possibly nested) module path, then the member. In
`acme.user.auth.google.token.secret`, `acme` is the namespace,
`user.auth.google` the crate, `token` the module, `secret` the member. A
crate whose name is itself dotted is no different — the leading crate
segments are [purely lexical](04-packages.md#crates-have-no-parents), not
themselves crates.

Because the **namespace is the leading segment, every fully-qualified name is
globally unique** — two namespaces may each hold a `google` crate
(`acme.google` and `widgets.google`) with no clash, which is what backs the
[no-aliases](#no-import-aliases) rule.

In source, an `import` normally writes just the crate path (`import
user.auth.google`); the namespace is taken from the matching **declared
dependency** (each dependency records its namespace in the manifest), so the
terse form stays the common case. The namespace-qualified form is the canonical
identity used by the registry and is the disambiguator available if a project
ever depends on same-named crates from two namespaces.

TODO(open): the exact surface form of a namespace-qualified import when
disambiguation *is* needed (a leading segment, a manifest alias on the
dependency, or a per-dependency local name). Lean: resolve it on the dependency
declaration, not with an import alias, to preserve [no-aliases](#no-import-aliases).

### How a dotted root stays unambiguous

Within one namespace, a dotted name raises an obvious worry: if crate
`acme.user.auth.google` exists, what stops a *different* crate `acme.user.auth`
from also shipping a module `google`? Two rules keep it single-valued:

- **Crates in a namespace are leaves** — their names form a *prefix-antichain*:
  **a name is either a leaf (a real crate) or a grouping prefix, never both.**
  So `acme.user.auth` and `acme.user.auth.google` cannot both be published
  crates. The [registry](09-package-registry.md) enforces this at publish time;
  because a namespace is owner-scoped, the check is local to one owner.
- **Import roots resolve by longest declared-dependency match.** At an import
  site the root is matched against the crate names the project *depends on*,
  longest first — deterministic because the dependency set is small and explicit.

So a crate can never "grow into" or sit under another crate's name: a dotted
prefix is *pure grouping*, only full leaf names are real crates, and every path
points at exactly one place.

## Import order does not matter

The order in which a file lists its imports never affects whether a successful
compilation produces a different program. Tel does not allow import-time side
effects (top-level bindings are `const` — see [Modules](01-modules.md)), so
shuffling import lines cannot change behaviour. A formatter is free to sort
imports however it likes; reviews never have to argue about order.

## Local vs published imports

A path that starts with a [published crate](04-packages.md) name is reaching
*external code*; a path that does not is reaching *own code*. The distinction
is real (different compile-time options apply, different
[versioning](06-versioning.md) rules apply — see
[Modules: own code vs external code](01-modules.md#own-code-vs-external-code))
but the **syntax of the import is the same** either way.

TODO(open): inputs suggest making local and external imports *visually*
distinguishable in source (a leading marker, a separate keyword, or a
convention). Lean: leave them syntactically identical — a reader can tell from
the root segment which is which, and a second form is one more thing to learn.
Philosophy does not yet cover this.

## Hyphens and underscores

Crate and module names map to import path segments. Inputs ask that **hyphen
and underscore be treated as the same character** in identifiers used in
import paths, so that a crate published as `csv-tools` and one published as
`csv_tools` cannot both exist, and neither variant fails to import the other.

TODO(open): confirm the equivalence rule (hyphen ↔ underscore) and decide
whether it applies only to crate/module names or to all identifiers. Lean:
only to crate and module names, where the rule blocks confusing duplicates;
applying it to ordinary identifiers would silently merge `user_id` and
`user-id` and surprise readers. Philosophy gap.

## Renaming: only at the export boundary

Renaming is forbidden almost everywhere, with **one** sanctioned exception. The
prohibition that matters for readability is on the *consumer* side: an
`import` can never rename or re-home a name (no `import x as y`), so within any
file a name means exactly what it says and its origin is recoverable without
leaving the file. Crate-internal code likewise uses each item's real module
path.

The single place a name *may* be re-homed is the crate's
[`export` block](03-visibility.md#crate-export-block): it maps internal items
to the public paths the crate presents, so the **public API is decoupled from
the code layout** and internal refactors do not break consumers. That mapping is
explicit, lives in one reviewable place, and is the producer's own decision —
none of the scattered-aliasing hazards the consumer-side rule guards against.
The public path and the item's defining path may therefore differ, but only
because the export block said so.

## No textual snippet inclusion

Tel does **not** offer a C-style `#include` or any other "splice the contents
of another file into this one" mechanism. Composition between files is always
through *imports* — a named module is brought into scope and its members are
reached through it, with the usual visibility rules. There is no facility for
copying another file's tokens verbatim into the current file's parse stream,
and the language should not grow one.

`TODO(open): users will sometimes still want to share a chunk of code between
two scripts without publishing a crate — for example, two embedded scripts
that legitimately need the same 30-line helper. The supported answer is a
local module they both import. Confirm that there is *no* sanctioned snippet-
inclusion form, and that the import system is ergonomic enough that nobody
reaches for one. If a snippet-style mechanism ever becomes necessary, it must
be hygienic — the included file's identifiers do not leak into the importing
scope, and a reader can still tell where every name in a file comes from.`

### Why

Snippet inclusion (the `#include`, the `eval(read_file(...))`) loses two
things Tel relies on:

- **Local readability.** A reader of a file should be able to identify the
  origin of every name without leaving the file. Splicing in another file's
  names breaks that.
- **Tooling exactness.** Rename, go-to-definition, and find-references all
  assume that the set of names a file defines is bounded by its own contents
  plus its explicit imports. Snippet inclusion expands that set silently.

Imports give the same composition power with neither cost — the only thing
they do not give is the ability to *forget* that a name came from somewhere
else, which is exactly the property Tel does not want to grant.

## See also

- [Modules](01-modules.md) — what a module is and how its API is shaped.
- [Visibility](03-visibility.md) — which members an import can actually see.
- [Crates](04-packages.md) — where published module roots come from.
