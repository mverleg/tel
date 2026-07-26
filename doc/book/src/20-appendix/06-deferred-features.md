# Deferred Features (Wanted, Not Yet Worth It)

This page lists features that are **not in Tel right now, but not because they
are undesirable** — only because they are not worth the implementation effort
for the time being. They may arrive later.

This is a distinct category from two others:

- **[Antifeatures](../02-philosophy/04-antifeatures.md)** — things Tel rejects
  *on principle*; they are not wanted at all.
- **[TIPs](../tips/README.md)** — design proposals under active discussion, or
  explored-and-rejected with the reasoning recorded.

A feature belongs here when the answer to "do we want this?" is *yes, in
principle*, but the answer to "is it worth building now?" is *no*. Each entry
records what the feature is, why it is deferred, and what to do in the meantime.

## REPL

**What.** An interactive Tel session — type a declaration or expression, have
it compiled and evaluated in a persistent session, see the result, and refer to
it on the next line.

**Why deferred.** A REPL that stays faithful to compiled Tel (no looser dialect)
needs **JIT and hot-swap**: each line is compiled incrementally and executed,
and re-binding or redeclaring something must update a live session. That
machinery is **hard** — enough that it is not worth building for the time being.
The cheaper "standalone interpreter with looser rules" alternative is rejected
because it would diverge from compiled behaviour (the jshell complaint), which
defeats the point.

**Design notes (for whenever it is picked up).** The hard part is preserving
session state across separately-compiled commands:

- **State lives in a session environment.** The driver holds a heap-allocated
  record of every live top-level binding; each command lowers to a function
  over that environment, reading prior bindings and appending new ones. The
  accumulated session is one ever-growing **scope**, so the ordinary
  type/borrow/move checker applies unchanged — a moved affine value stays dead,
  etc. The one wrinkle is must-use (`relevant`) values: the scope never closes,
  so the obligation must be deferred to session end or relaxed at the top level.
- **Redeclaring a type invalidates dependents.** If a type is redeclared with a
  different shape, existing values of the old type are not kept around as stale
  orphans (the jshell approach, which conflicts with *stale data is worse than
  no data*). Instead the REPL uses the incremental compiler's reverse-dependency
  graph to **drop every binding that depended on the old definition, loudly**.
  Structurally-identical redeclarations of a structural alias are a no-op;
  `newtype` (nominal) or structurally-different redeclarations always
  invalidate.
- **Commands are transactional.** Because Tel aborts rather than unwinds, a
  command runs in a child context whose new bindings are committed to the
  session only on clean completion; an aborting line is discarded and the prior
  session survives.

**Meanwhile.** None needed for exploration (use a scratch file with
`tel run`). The one place a REPL-like facility was wanted was **mocking** —
replacing a top-level function at runtime reuses the same hot-swap machinery.
The mock story is itself still open; until it settles, use **dependency
injection** (pass the collaborator in, swap it in tests) — see
[Testing → Mocks](../14-testing/01-testing.md#mocks).

## Dimensional-analysis units (a dedicated `unit` construct)

**What.** Physical-unit types with a *dimension-aware operator algebra* — the
compiler knows `weight * velocity` yields `momentum`, rejects
`Temperature * Temperature`, tracks SI prefixes and derived units (`m/s^2`,
`kWh`), and a `Quantity[U]` stdlib type carries it all. The
[Physical Units](../05-types/13-units.md) chapter is a stub pointing here.

**Why deferred.** Construct-time unit *newtypes* are committed for Tel1 (rung
1b in [TIP-0004](../tips/0004-how-far-refinement-types-go.md)); what is deferred
is the heavier *dimensional-analysis algebra* on top. That machinery is "heavier
than usual library code" (see the units chapter), it pays off mainly in
scientific/financial scripts rather than the typical embedding hook, and several
sub-questions are unresolved (SI-prefix scaling without inviting implicit
conversion, display units, a curated unit catalogue). Crucially it is
**additive**: nothing about adding a `unit` keyword or a `Quantity[U]` stdlib
later breaks earlier code.

**Design notes (for whenever it is picked up).**

- **No `100 kg` suffix syntax.** An earlier sketch let `100 kg` desugar to a
  unit-multiplication (`kg.left_mul(100)`); this is **definitively abandoned**
  because it reintroduces magic whitespace (a number abutting an identifier),
  which Tel forbids. A quantity is written with ordinary syntax — a constructor
  `kg(100)`, a postfix conversion `100.kg`, or an explicit operator — exact
  spelling still open.
- **A `unit` declaration carries an algebra, not just a wrapper.** The point of
  a dedicated construct over a bare newtype is dimension-aware operators: some
  built-ins are *removed* (`Temperature * Temperature` should not type-check),
  new ones are *added* (`weight * velocity -> momentum`, yielding derived
  units), and same-dimension arithmetic stays (`weight + weight -> weight`). So
  a unit type is a refined type with a *customised operator surface*.
- **Open questions.** SI prefixes and scales (`kg` vs `mg` — same type with a
  scale factor, not separate types, and without inviting implicit conversion);
  value constraints composed with units (`weight >= 0`); display units (storing
  in one unit, rendering in another, including a shared scale across a
  collection); a built-in catalogue (percent vs fraction as distinct types,
  derived units like `euro/s`, `m/s^2`, `kWh`); and the *mechanism* question —
  language keyword vs a stdlib construct on refined types plus operator
  overloading. Lean: stdlib construct if those are expressive enough, a `unit`
  keyword only if they are not.

**Meanwhile.** The common case — *"this scalar is a distinct kind of thing"* —
is already covered by [newtype / refined types](../05-types/12-refined-types.md)
plus [operator overloading](../09-functions/09-overloading-and-dispatch.md): a
`Celsius` wrapper, a `Percent` distinct from a `Fraction`, a `Money` carrying
its `Currency`. Reach for the dedicated dimensional construct only once a
concrete embedding use case shows refined-types-plus-overloading is not enough.

## Branded types and generativity

**What.** Refined types where each *instance* carries a unique compile-time
*brand*, so e.g. an index handed out by one `GrowOnlyVec` cannot be used on
another, and bounds checks are provably unnecessary. It is a flavour of
[refined type](../05-types/12-refined-types.md), where this would integrate.

**Why deferred.** The mechanism is advanced and the **brand-preservation** story
is awkward (putting branded values in a collection generally loses the brand
unless the whole collection shares one). Its headline payoff — eliminating
bounds checks — is a performance win, and *high abstraction over low-level
control* means Tel does not chase that at the cost of a hard-to-learn feature.
The motivating use cases (portfolios, permutation groups) lean standalone rather
than embedding. It is additive: branded types can arrive later without changing
how unbranded code type-checks.

**Design notes (for whenever it is picked up).** This is the *generativity*
pattern: each call to a `with_brand`-style scope gets a fresh, incompatible
brand, and an index tied to that brand provably indexes only that structure.

```tel
# Sketch — syntax not settled. Each with_brand gets a fresh brand.
with_brand |orders| {
    let i: Index[orders] = orders.push(order)
    orders.at(i)          # no bounds check: i provably indexes `orders`
}
```

The hard part is **brand preservation**: putting branded values in a collection
generally loses the brand unless the whole collection shares one, so you
re-establish it with an assertion. It is most useful when a few explicitly-named
branded structures live on the stack. A borrow's lifetime can *approximate* a
brand inside a closure (Tel's lifetimes are structurally propagated and rarely
written), but it is not ergonomic enough to be the primary mechanism — a
first-class brand stays the clearer tool. Other fits: permutation-group
elements, portfolio positions, claims belonging to a customer.

**Meanwhile.** Use a phantom-typed [`Id[T]`](../19-use-cases/09-entity-identity-and-projections.md)
to keep one entity's keys from being used on another, and accept ordinary
bounds-checked indexing.

## Record-and-replay (time-travel) debugging

**What.** A debugger that can step *backwards*: the runtime journals every
capability call (plus the seeded RNG and deterministic clock), so a task's
state at any earlier step can be reconstructed and any debugger client driven
back to it. The [Debugger](../18-tooling/08-debugger.md) chapter ships the
forward-debug surface; this stepping-back layer is what is deferred.

**Why deferred.** Recording is expensive — every capability call must be
journaled — and a correct record-and-replay backend for each compile target is
a non-trivial commitment. The value is real but it is a power-user, occasional
need rather than an everyday one. The language should still **reserve the
runtime hook** now (so a host *can* drive replay), which keeps actually shipping
a recorder a backwards-compatible, host-by-host decision rather than a language
change.

**Design notes (for whenever it is picked up).** Per-task heap isolation makes
step-back tractable *per task* — the task's state at step N can be reconstructed
from the seeded RNG, the deterministic clock, and the recorded capability
inputs, *if* those are journaled. The runtime would expose a **record-and-replay
hook** the host enables for a session; replay then drives any debugger client
back to a chosen step. It stays **off by default** because journaling every
capability call is expensive and the host is the one paying. The open commitment:
ship the *hook* (so debuggers can drive it) without promising a *recorder* for
every backend.

**Meanwhile.** Per-task heap isolation already makes a single run reconstructable
from its inputs, so the cheaper
[non-interactive trace](../18-tooling/08-debugger.md#tracing-a-run-to-a-log)
(`tel trace`) gives most of the "what did this actually do" value without a
stepping recorder. Ordinary forward stepping, breakpoints, and auto-break on
panic cover interactive debugging.

## Computation dependency graph (stdlib)

**What.** A `std` facility for declaring a DAG of work items with dependencies,
run in topological order (optionally on a task pool), with optional caching of
intermediate results and a graph-visualisation hook. The
[Scheduling](../17-standard-library/17-scheduling-and-timed-ops.md) chapter
ships the committed timing helpers; this graph facility is what is deferred.

**Why deferred.** The surface is substantial and sits at the blurry line between
a *library* and a *workflow engine* — once durable state and richer scheduling
creep in, it stops being core stdlib. It is opt-in and recurring, but a small
script never needs it, so it is a poor use of early effort. Adding it to `std`
later is purely additive.

**Design notes (for whenever it is picked up).**

```tel
# Sketch — syntax loose.
let g = Compute()
let a = g.task("fetch_orders", || db.orders())
let b = g.task("fetch_users",  || db.users())
let c = g.task("join", needs = [a, b], |(orders, users)| join(orders, users))
let result = g.run(pool) ?
```

Properties: **topological scheduling** (independent tasks run in parallel on the
pool, dependents wait for inputs); **required vs fallible nodes** (a node marked
optional can fail without failing the graph); **caching** of intermediate
results across runs; and a **visualisation hook** emitting Mermaid/Graphviz for
docs and debugging. The boundary to hold: `std` would ship the declaration plus
topological run; durable state and distributed execution belong in a library —
which is exactly why the whole thing can start as a crate.

**Meanwhile.** Compose tasks directly with the
[concurrency utilities](../17-standard-library/12-concurrency-utilities.md)
(worker pools, `select`, structured concurrency); for the simple cases hand-wire
the dependencies. A third-party crate can ship the full graph runner before
`std` blesses one.

## Typed LLM capability

**What.** A typed `LLM` capability — messages, system prompts, tool / function
calls, streaming token deltas, structured-output schemas — handed in by the host
the way every other capability is. The
[Networking](../17-standard-library/11-networking.md) chapter points here.

**Why deferred.** The want is real and common (many 2026 scripts call an LLM like
any other remote service), but the surface is a *moving target*: every major
provider's request shape, tool-call format, and streaming semantics is still in
flux mid-2026, and Tel's stability commitment makes freezing any specific shape
now a poor bet — a frozen-wrong API is worse than none. The blocker is *durability
of the shape*, not effort, so this unblocks itself once a provider-agnostic shape
settles. Fully additive: it arrives as one more capability.

**Design notes (for whenever it is picked up).** The natural shape is a typed
capability mirroring the request/response surface every provider shares —
messages with roles, a system prompt, tool/function-call declarations and their
results, streaming token deltas, and a structured-output schema the model is
asked to fill. The reason to wait is precisely that this surface is *not* shared
yet: request shapes, tool-call formats, and streaming semantics differ per
provider and are still moving. Revisit once a provider-agnostic shape is durable
enough to freeze for decades.

**Meanwhile.** A bare `Http` capability already lets a script talk to any LLM
endpoint; the typed convenience lives in a crate on top of `Http` until a
freezable shape exists.

## Advanced package-registry features

**What.** The heavier end of the [package registry](../11-modules-and-packages/09-package-registry.md):
**binary mirrors** (serving prebuilt, platform-specific artifacts) and the
**stronger supply-chain layer** — signing, attestations, build provenance, a
Go-style transparency log, automated vulnerability auditing.

**Why deferred.** High-effort infrastructure with low payoff for the embedded-
scripting target, where the host owns deployment and most scripts lean on `std`
rather than a large third-party surface. Binary mirrors in particular multiply
the surface badly (one source version maps to many target binaries), and the
central index stays source-only regardless. The committed 1.0 baseline already
covers the essentials — immutable versions, content-addressed artifacts verified
against the lockfile, source-only distribution, per-dependency capability
declarations — and the rest layers on additively.

**Design notes (for whenever it is picked up).**

- **Binary mirrors.** A self-hosting org could run a mirror that *also* serves
  prebuilt binaries as a speed optimisation. Shelved because binaries are
  platform-dependent — one source version maps to *many* target binaries, which
  multiplies the surface — and the central index stays strictly source-only
  regardless. Revisit only after the base language and the source-only index are
  settled.
- **Stronger supply-chain protections.** Signing, attestations, build
  provenance, maintainer reputation, a Go-style transparency log
  (`sum.golang.org`) for cross-index tamper-evidence, automated vulnerability
  auditing (deps.rs-style). The committed baseline is enough for 1.0: immutable
  versions, content-addressed artifacts verified against the lockfile,
  source-only distribution, and per-dependency capability declarations — and
  most users lean on `std`, keeping third-party surface small. Revisit signing /
  attestation once the base language is settled.

**Meanwhile.** Rely on the committed baseline; an org that needs mirrors or
provenance today runs that tooling itself, outside the central index.

## Compiler-plugin model and crate-distributed lints

**What.** A model for distributing build-time extensions as crates — custom
lints, doc-generation hooks, compiler plugins. Flagged "deferred until after the
base language is ready" in
[Build System](../18-tooling/03-build-system.md),
[Crates](../11-modules-and-packages/04-packages.md), and
[Workspaces](../11-modules-and-packages/10-workspaces.md).

**Why deferred.** It cannot be designed before the base language and its
metaprogramming line are settled, and it brushes directly against the
*no heavy metaprogramming* antifeature — a plugin that can rewrite or inspect
code is exactly what that maxim guards. So this is **defer leaning toward
re-justify**: revisit once the base language is frozen, and be ready to conclude
that only a narrow, declarative extension surface (or nothing) survives. Additive
if it lands.

**Meanwhile.** The built-in [linter](../18-tooling/07-linter.md) and
`derive`-style attributes cover the in-language needs; anything genuinely
data-driven is a visible codegen step, not a hidden plugin.

## Cross-machine build-cache sharing

**What.** The compiler's incremental cache (see
[the compiler](../18-tooling/01-compiler.md#two-compile-modes)) shared
*between machines*: a teammate or CI runner compiles a dependency once, and
everyone else's build turns into cache hits from a shared store.

**Why deferred.** The content-key cache design is already shareable *by
construction* — a content key means the same thing on every machine, and
validity-by-construction means there is no invalidation protocol to
coordinate. What sharing additionally needs is plumbing, not design: a
persistent store, a machine-neutral serialization of cached answers, a
compiler-version namespace in the keys, a transport, and a trust story (a
shared cache is a "whoever writes an entry decides what your compiler
computed" channel). None of that is worth building now.

**Meanwhile.** Build nothing explicit, but hold one standing constraint: **no
machine-local identity may leak into cache keys or cached answers** — no
absolute paths, no process-local ids, no platform-dependent encodings.
Single-machine correctness already requires this (an entry must mean the same
thing across two runs), so honouring it costs nothing extra; it just must not
be traded away by a future architectural decision.

## Tagged DSL literals

**What.** Tag-function string literals — `sql"..."`, `html"..."` — where the tag
chooses how interpolation slots are interpreted (e.g. `sql` keeps `${id}` as a
bound parameter rather than substituting it). See
[Literals](../03-lexical-structure/05-literals.md) and
[String Operations](../07-expressions/05-string-operations.md).

**Why deferred.** They are load-bearing for the glue-between-systems use case,
but they overlap [metaprogramming](../02-philosophy/04-antifeatures.md), which Tel
treats with suspicion — so this is the **closest of these to a cancel**: it may
ultimately belong in antifeatures rather than here. Lower-priority than raw and
multi-line literals, which cover most of the need. Additive: a tag-literal form
can be introduced later without changing how plain and raw literals parse.

**Design notes (for whenever it is picked up).** The sketched form uses
backticks as the delimiter, with a leading tag naming the embedded language:

```tel
# pseudo-syntax — not pinned
let q   = sql `SELECT name FROM users WHERE id = ${id}`
let doc = json `{ "ok": true, "n": ${n} }`
let rx  = regex `\d{3}-\d{4}`
```

The tag is an ordinary function the lexer hands the literal to, so an IDE can
highlight/lint the contents as the embedded language. Nested backticks escape by
*repeating* the delimiter (Markdown-style), so any embedded language appears
without backslash gymnastics. The contents look like a string to the lexer, but
the *meaning* is the tag's choice — a `sql` tag can keep `${id}` as a bound
parameter instead of substituting it inline. The tension to resolve before
shipping: this overlaps the [metaprogramming](../02-philosophy/04-antifeatures.md)
Tel is wary of, and whether interpolation inside a tagged literal evaluates
eagerly or is captured for the tag is unsettled.

**Meanwhile.** Use ordinary [string interpolation](../07-expressions/05-string-operations.md)
with explicit escaping/parameter-binding helpers (e.g. an `esc(...)` or a
parameterised query builder), as the
[localization-library use case](../19-use-cases/10-localization-library.md#4-escape-or-localise-output-slots)
does for HTML slots.

## Documentation Generator (`tel doc`)

**What.** `tel doc` — a toolchain subcommand that turns a project's source
(declarations, doc comments, contracts, examples) into **browsable reference
documentation**: a static HTML site by default, with single-page Markdown and
structured JSON from the same pipeline. It runs on top of the
[friendly/incremental compiler](../18-tooling/01-compiler.md), so it renders the
*resolved* API — signatures, refined types, capability requirements, contracts,
trait conformances — not a re-parse of the surface text. The
[Documentation Generator](../18-tooling/10-documentation-generator.md) tooling
page is a stub pointing here.

**Why deferred.** The want is real — the maxim *the standard library should be
enough for small, complete programs*
([`../02-philosophy/02-maxims.md`](../02-philosophy/02-maxims.md)) implies every
public name carries reference docs findable without leaving the toolchain. But
the generator is a substantial pipeline (resolved-API projection, a doctest
runner, multi-format output, a versioned site with a version picker) whose payoff
only lands once there is a language and a stdlib to document. It is fully
**additive**: nothing about the language changes when `tel doc` arrives later, so
building it now buys little. Doc comments themselves remain a committed language
feature ([comments](../03-lexical-structure/07-comments.md)); only the *generator
tool* waits.

**Design notes (for whenever it is picked up).**

Doc comments are **Markdown-flavoured**, attached to the declaration immediately
below them; a fenced ` ```tel ` block is a doctest, other tags render but don't
run. Position carries the target (module / type / function) — no Rust-style `///`
vs `//!` split. `TODO(open): exact comment syntax — whichever is chosen, the
doc/non-doc distinction should not need a second character class.`

For each public declaration the page renders: the **resolved signature**
(generics with constraints, [refined types](../05-types/12-refined-types.md)
linked); **capability requirements** as a header badge; **contracts** read
straight from source; the **doc-comment prose**; **examples with output**;
**[declared example values](../17-standard-library/19-testing-utilities.md#declared-example-values-and-counter-examples)**
for types that carry them; **cross-references** resolved against the symbol
graph; **trait conformances**; and a **source link** at the matching version.

### Examples as tests

A fenced ` ```tel ` block in a doc comment is picked up by the
[test runner](../14-testing/01-testing.md): it is compiled in a fresh context,
run, and its claimed results asserted. If a function changes so the example no
longer holds, the build and `tel test` both fail — there is no "out of date but
still rendered" state. This is the mechanism behind *examples as tests*.
`TODO(open): doctest semantics — fixture scope, capability availability, the
assertion expression; coordinate with [testing](../14-testing/01-testing.md).`

### Business goals and system invariants

The two highest-level kinds of documentation route by how checkable they are.
**System invariants** split: *machine-checkable* ones become
[contracts](../02-philosophy/03-features.md),
[refined types](../05-types/12-refined-types.md), and
[record invariants](../10-data-modelling/01-records.md), read straight into the
docs so they cannot drift; *unprovable* ones become
[review invariants](../18-tooling/07-linter.md#review-invariants--unprovable-re-checked-on-change)
— prose anchored to the declaration, re-surfaced on change. **Business goals**
are necessarily human language and usually cross-cutting, so they stay as prose
next to the code they motivate: a module's or crate's `##` doc comment states
*what this unit is for*, rendered at the top of its page; system-wide goals live
as long-form project prose the generator embeds but does not invent. The
asymmetry is the point — invariants migrate *out* of prose into the type system
where possible; goals stay prose by nature. `TODO(open): whether a goal deserves
a first-class @goal(...) anchor or a plain module doc comment suffices — lean:
doc comment, promote only if review tooling needs it.`

**Inheritance.** A trait method's doc comment is the **default** for a conformer
that writes none (the conformer's page links back, no silent duplication); a
conformer that writes its own **overrides**, with an optional explicit
re-include. `TODO(open): syntax for the re-include (e.g. @inheritdoc).`

**Versioning.** Generated docs carry the crate
[version](../11-modules-and-packages/06-versioning.md) with a "switch version"
picker; old versions stay fetchable forever, matching the
[registry](../11-modules-and-packages/09-package-registry.md) — part of the
stability commitment.

### What the generator does *not* do

Not a **website builder** (reference docs, not marketing — project sites embed
the output, not vice versa); not an **architecture-diagram generator** (the
dependency graph is structured data for the
[editor](../18-tooling/09-editor-integration.md),
[`tel graph`](../11-modules-and-packages/08-dependency-graph-and-locking.md#visualising-the-graph-and-its-diffs),
and external tools — `tel doc` picks no diagram style); not a **wiki** (doc
comments live with the code, never edited from the rendered side); not a
**substitute for project prose** (long-form chapters live as Markdown the project
renders).

Open questions: **markup choice** (Markdown lean, ReST/Sphinx set aside);
**output formats** (HTML required, JSON lean-yes, man/LaTeX speculative);
**editor preview** (hover-render, coordinate with
[editor integration](../18-tooling/09-editor-integration.md));
**internationalisation** (multi-language stdlib docs punted — revisit once the
language has users).

**Meanwhile.** Doc comments are ordinary `##` comments today, so the
documentation lives with the code regardless. Examples in them can be run by the
[test runner](../14-testing/01-testing.md) once that exists, keeping them honest
without the generator; and a project that needs a rendered site hand-writes
Markdown or runs an external doc tool until `tel doc` ships.
</content>
