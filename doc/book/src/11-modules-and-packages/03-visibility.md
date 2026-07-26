# Visibility

<!-- TODO: review -->

**Visibility** controls which of a module's members other modules can see. In
Tel this is a small, opt-in feature: a single script never needs it, and even a
multi-file project only reaches for it when it deliberately wants an internal
boundary.

## What visibility is

Tel has **three levels of visibility**, decided in
[TIP-0003](../tips/0003-module-levels-and-dependency-direction.md#visibility-three-levels-mandatory-crate-export).
They map onto the two boundaries that actually matter — the [module](01-modules.md)
(a file/directory) and the [crate](04-packages.md) (the publishable unit) —
and they are arranged so that the *embedded-scripting* common case needs no
marker at all:

1. **Crate-visible — the default.** A top-level item with no marker is visible
   throughout its **own crate**, but not to other crates. A single-file
   script is one crate, so *everything is usable everywhere in it* with zero
   ceremony — which is exactly what scripting wants.
2. **Module-private — `private`.** Opt in with a marker, and the item is visible
   only inside its **defining module and that module's children**
   ([modules nest meaningfully](01-modules.md#super-modules-see-sub-modules), so
   a child can still see a parent's private items). This is the deliberate act
   of drawing an internal seam inside a larger crate.
3. **Externally public — export-gated.** An item is visible to *other crates*
   only when it is **(a) listed in the crate's [`export` block](#crate-export-block)
   and (b) not `private`**. External surface is never implicit.

```tel
# Crate-visible (default): usable anywhere in this crate, not outside it.
fn convert(amt: EuroAmt, to: Currency) -> Result[Money, FxError] { ... }

# Module-private: visible only in this module and its children.
private fn round_half_even(x: Decimal) -> Decimal { ... }
```

Notably, there is **no separate marker for "visible across the whole crate but
not externally"** — that is just the *default* state, and the focus on embedded
scripting is why it is the default rather than an opt-in. An item becomes
external only by being named in the crate export block.

The marker is **`private`**, spelled in full. Tel abbreviates only its
ultra-frequent keywords (`fn`, `let`, `pub`) and spells the rest out (`match`,
`struct`, `trait`, `import`, `export`); a visibility opt-out is rare, so
readability wins over brevity. `local` is **not** available — it is already a
[keyword](../20-appendix/01-keywords.md) for scope-local bindings — and with
visibility defaulting to open there is no `pub` sibling that a `priv` would need
to match. `private` is also the most universally understood visibility word
(Java/C#/Kotlin/Swift/TypeScript). Subject to final confirmation once the wider
grammar is pinned.

The model also avoids a Rust footgun: a function may be public even
when its return type is private (warning only), the function then cannot
actually be called from outside (the return type is not nameable), and
re-exports can hide the inconsistency. Tel collapses the combinatorial mess:
an **exported** function whose signature mentions a type that is *not* reachable
at the same level — a `private` type, or a merely crate-visible type that the
crate does not export — is a **hard error**, not a warning. Exported surface
may only rest on exported (or [opaque](01-modules.md#module-level-apis)) types.

The same reasoning extends inward to inference: type inference is likewise
capped at the visibility boundary, so a hidden type cannot reach external code
*via an inferred position* either. See
[inference only produces nameable types](../05-types/08-type-inference.md#inference-only-produces-nameable-types)
and [Public members need explicit types](#public-members-need-explicit-types).

TODO(open): confirm the hard-error stance, and decide how it interacts with
[opaque types](01-modules.md#module-level-apis) — an opaque return is the
intentional way to expose "an unnameable type by handle." Lean: opaque is the
escape hatch; a non-opaque private type in a public signature stays an error.
The inference companion rule (infer only visible types) is decided for now,
pending the same opaque-type exception once unnameable types land.

## Does visibility even make sense for an embedded language?

This is a genuine open question. In a standalone language, visibility controls
what downstream code may depend on. But Tel is a *guest*: the **host** already
decides what a script can see and do, and most scripts are a single file with
no importers at all.

TODO(open): **does visibility carry its weight in an embedded language?** The
host chooses what to expose across the host/script boundary, so the visibility
marker is *not* what gates the host boundary. The remaining justification is
purely *intra-Tel* encapsulation: in a larger Tel project with several modules,
the private marker draws the seam between a module's API and its internals.
Lean (matching [`03-features.md`](../02-philosophy/03-features.md)): keep
visibility, but only as an opt-in project-scale feature — a small script, with
everything public by default, ignores it entirely. Philosophy should state this
explicitly.

Where it does carry weight, the payoff is **local reasoning**: an opaque type
and a private member let a module change its internals without any importer
being able to depend on them, so a reader reasons about the module from its
public seam alone. That is the encapsulation half of the same principle that
[no global mutable state](../06-bindings-and-scope/07-no-global-mutable-state.md#why)
serves from the mutability side.

## Public members need explicit types

Tel's type inference is intended to be limited and one-directional, but where
it *is* available it stops at the module boundary:

- **Exported functions must have fully explicit signatures** — every parameter
  type and the return type spelled out. A published signature is an API
  contract; it must not silently change because inference picked a different
  type after an unrelated edit.
- **Non-exported members may lean on inference** where it is unambiguous —
  whether they are crate-visible or `private`, the author has full context.

But inference inside a module is not a back door for leaking a hidden type past
this boundary. **Inference itself only ever produces a type that is visible where
its result is used** — a non-exported helper cannot infer a `private` (or merely
crate-visible) type and let it escape outward through call sites. Without this,
"exported signatures must be explicit" could be side-stepped by inference; with
it, every inferred type is one the author could have named. See
[inference only produces nameable types](../05-types/08-type-inference.md#inference-only-produces-nameable-types).

```tel
# Exported — fully annotated, no inference in the signature.
fn score(o: Order, clock: Clock) -> Result[Score, Reject] { ... }

# Internal — inference may fill in the obvious parts.
private fn weight(o) = o.total.amount * 0.001
```

### Why require types only on exported functions

- **Stability.** Type inference is an easy way to break backwards
  compatibility: implementing a new trait, or adding a type, can make inference
  pick a different (still valid) type, changing an inferred public signature.
  Pinning public signatures removes that hazard. See
  [Versioning](06-versioning.md).
- **Readability for the reader who matters most.** An exported function is read
  by people who did not write it; an explicit signature is documentation.
- **Writability where it is cheap.** Inside a module the author has full
  context, so inference on private members costs nothing in clarity.

This mirrors the priority ranking: explicit where it protects stability and the
external reader, inferred where it only saves the author keystrokes.

## Crate export block

A crate's external surface is **explicit and mandatory**: a
[crate](04-packages.md) declares an `export` block, and only names listed
there (and not marked `private`) are visible to other crates. There is no
implicit "everything public leaks out" — the crate-visible default
([above](#what-visibility-is)) stops at the crate boundary, and crossing it is
a deliberate, single, reviewable act.

The block is the natural diff surface and pairs with the
[generated API-summary file](06-versioning.md): "what does this crate offer?"
is one place to read and one place to review.

**`export { … }` lists in-scope names — there is no `from` clause.** Bringing a
name into scope is [`import`](02-imports.md)'s job; `export` only chooses which
already-visible names cross the boundary. Bare names keep it terse, and a
**re-export is just import-then-export**:

```tel
# Illustrative — syntax not finally pinned down, but this is the recommended shape.
import regex                               # bring the path into scope
export { convert, Money, regex.Regex }     # own items + a re-exported external path
```

This is chosen over Rust's scattered `pub` / `pub use` (no single place to read
the API), over a bespoke block syntax (unfamiliar), and over an ES-module
`from` clause (redundant — the path already says where a name comes from).

### The public API is decoupled from the code layout

The export block does not merely *filter* the module tree — it **defines the
public API tree**. Every entry is **`<public-path> = <in-scope-internal-path>`**,
and *either side may be dotted/nested*, so an internal `a.b` can be exposed as a
public `c.d`. A bare entry keeps the name (`name` ≡ `name = name`):

```tel
export {
    convert                              # public `convert`  = internal `convert`
    Money     = pricing.money.Amount     # shallow public name, deep internal item
    c.d       = a.b                      # deep -> deep: expose internal a.b as c.d
}

# a nested public block is sugar for a shared prefix — these two are identical:
export { c.d = a.b,  c.e = a.f }
export { c { d = a.b,  e = a.f } }
```

The payoff is **backwards compatibility through refactors**: moving or renaming
code internally changes only the *right-hand sides* here, leaving the public
left-hand paths — and every consumer — untouched. This is the
[one place renaming is allowed](02-imports.md#renaming-only-at-the-export-boundary):
the crate's own facade, declared in one reviewable spot, never the consumer side.

A **distributed form** — each module carrying its own `export` block that the
crate root composes — is also allowed, but it is **optional**, not required for
nesting; the crate-root block can express the whole public shape on its own.

This earns its keep precisely because the *external* surface is the small set,
not the default — so an explicit list of exports is short, while the
crate-visible default (the large set) carries no ceremony. That resolves the
earlier tension between "public by default" and "enumerate the exports": the two
live at *different boundaries* — the default applies *inside* the crate, the
export block applies *at* the crate edge.

### Export granularity: items, not fields

The export block lists **top-level items** — a type name, a function name — not
the individual *members* of a type. The two visibility notions are therefore
**not fully independent**: once a `type` is exported, the external visibility of
each of its fields and methods is governed by the *same*
[crate-default / `private`](#what-visibility-is) rule applied inside the type.
A field stays `private` even on an exported type; a non-`private` field of an
exported type *is* externally visible because its containing type crossed the
boundary.

So the model deliberately **cannot express "this field is visible across the
crate but hidden from external code while its type is exported."** Field-level
external visibility always follows the enclosing type's export plus the field's
own `private` marker. This is less than full expressivity, but it covers the
sensible cases — expose a type, keep some of its internals `private` — without a
second, field-level export mechanism. To hide more, keep the field `private`
(or front the type with an [opaque type](01-modules.md#module-level-apis)).

### Module export blocks feed the crate one

A **module may also carry an `export` block**, declaring the surface it offers
*upward*. The crate export block then **references those module exports** to
assemble the crate's outward API — a crate curates its public surface from
what its modules choose to offer, rather than reaching into module internals.
The module block is optional (a module with none offers its crate-visible
items to its parent as usual); the crate block is required to publish anything.

TODO(open): only fine syntax remains — whether a crate block can *glob* a
module's exports (`export { auth.* }`) rather than listing names, and final
token spellings. The `export { … }`/no-`from` shape above is the decided form.
Tracked in [TIP-0003](../tips/0003-module-levels-and-dependency-direction.md).

TODO(open): **multiple APIs per crate.** Inputs raise the niche case of a
crate exposing more than one API (e.g. a stable API and an unstable one). Lean:
no — one export surface per crate keeps the model simple, and splitting into
two crates covers the use case. Re-visit if real cases come up.

### Trait impls are not exported items

The export block governs **named items** — types and functions. A **trait
implementation** is not one of them and is never listed in an `export` block.
An `impl T for D` is visible wherever both `T` and `D` are, automatically:
its reach is fixed by the
[orphan rule](../10-data-modelling/03-traits-or-interfaces.md#coherence-the-orphan-rule-and-specialisation)
— coherence is [**crate-scoped**](04-packages.md), one resolved impl per
(trait, applied-type) in any program — rather than by the visibility markers
above. There is therefore no "private impl": you cannot expose a type and a
trait while hiding the fact that the type implements it, and you do not need to
re-export an impl that a downstream crate already sees through the trait and
type.

## Strict mode

A possible **strict mode** would promote visibility from opt-in to required —
forcing explicit visibility markers on items in larger projects while small
scripts stay loose. This is tentative and carries a real risk (splitting Tel into two
dialects); see the open question in
[`03-features.md`](../02-philosophy/03-features.md).

## See also

- [Modules](01-modules.md) — module-level APIs and opaque types.
- [Imports](02-imports.md) — how visible members are named by importers.
- [Versioning](06-versioning.md) — why public signatures must be stable.
