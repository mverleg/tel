# Fast mode vs detail mode — design

Status: **direction decided, not implemented.** Decisions: no mode key/args,
no parallel pipelines; detail data as separately-fingerprinted **outputs of
multi-output steps** + driver demand policy. Roadmap: Phase 2, step 11 (see
[roadmap.md](roadmap.md)). Related to [flavors.md](flavors.md) — mode was
floated as the first concrete flavor, but the inventory below concludes it
likely needs **no key dimension at all** (detail = sidecar queries + demand
policy); flavors then wait for opt-level. ("Detail mode" was earlier called
"IDE mode"; same thing — the IDE is one consumer of it, error rendering and
doc extraction are others.)

## Goal

Two views of the same compilation:

- **Fast mode** — minimal metadata. No source spans, no reference tables, no
  rendered messages, no doc comments. Smallest/fastest path to "does it
  compile, and what does it produce". The batch-compiler default.
- **Detail mode** — full metadata: source spans (line/col), resolved reference
  maps (go-to-def), original identifiers, doc comments, rendered diagnostics.
  Slower and larger; needed for good error messages and editor features.

Driving policy: **compile and test in fast mode; on any error, automatically
upgrade the failing part to detail mode** and render the diagnostic from that.
The happy path never pays for metadata.

Where the sandbox is today: the AST has essentially *no* metadata already
(`PreExpr`/`Expr` carry no spans; only `Panic`/`Unreachable` get a
`source_location: String`, attached at resolve — `resolve.rs:231`). So current
behaviour ≈ fast mode; this plan is about *adding* the detail layer without
losing what the caching design already bought.

## Inventory — what does "detail" actually contain?

Before choosing machinery: is the difference only span metadata? No. The
candidates split into three categories, and the categorization drives the
whole design:

**Category A — extra recorded data (sidecars).** Same computation, more
written down: spans/line numbers; original identifier text surviving
interning, mangling, and monomorphization (errors should say `foo`, not
`FuncId(383)` or `foo$i64`); doc comments and trivia (the tokenizer discards
comments today; docgen and a future formatter need them); resolve's
reference map (go-to-def / find-refs); typecheck's per-node type map (hover,
inlay hints); completion scope tables.

**Category B — behavioral differences.** The computation itself changes:
error recovery (bail-at-first-error vs recover-and-continue with partial
ASTs); expensive diagnostics (suggestion search, "did you mean");
lints/warnings.

**Category C — impostors.** Debug-vs-release line tables and opt level —
that's the *opt-level* flavor, orthogonal to mode (see runtime story).

Each category B item dissolves on inspection:

- **Error recovery: make it always-on.** On valid input a recovering parser
  does the same work as a bail-early one; recovery code runs only *after* an
  error — already the slow path. rustc and rust-analyzer always recover.
  Always-on recovery makes the same-errors invariant hold by construction;
  a detail-only recovering parser would *break* it (detail would report
  errors fast never saw).
- **Suggestion machinery** lives in the diagnostic renderer — already a sink
  on the error path.
- **Lints** are separately demandable queries, a driver policy. (If `-Werror`
  ever exists, lints become semantic and must run everywhere anyway —
  another reason not to couple them to mode.)

**Consequence — "mode" may not be a key dimension at all.** If every real
difference is a category A sidecar or a dissolved category B, then no query
*computes differently* under any mode, and no cache key needs a mode
dimension. "Fast vs detail" reduces to a **demand policy in the driver**:
which sidecar/sink queries get requested, and when. Batch policy: none until
an error. IDE policy: working set. Debug-build policy: span sidecars for
emit. Same graph, same keys, no flavor — flavors.md's machinery then waits
for its first genuine customer (opt-level, with the backend).

Watch-item that would resurrect the mode bit: if preserving pretty names
through monomorphization turns out to need threading state through a hot
path rather than a side table, that is the first genuine
compute-differently case. Until one materializes, a mode flavor is
speculative machinery — the sections below describe it for when/if it is
needed, but the default plan is **sidecar queries + demand policy, no mode
key**.

## Question 1 — one pipeline or parallel branches?

### Approach 1a: parallel query kinds (`ParseFast` / `ParseDetail`, …)

Two independent pipelines, each a full set of query kinds.

- **+** No mode plumbing; each pipeline is simple in isolation.
- **−** Doubles the query graph and the cache footprint.
- **−** **The pipelines will diverge.** The whole feature rests on one
  invariant: *the detail rerun reproduces exactly the error fast mode hit*
  (same failures, same resolution, same values — mode changes only metadata
  richness, never decisions). Two hand-maintained implementations cannot
  guarantee that; the day they disagree, the upgrade-retry either can't find
  the error it's supposed to explain or invents a different one.
- **Rejected.**

### Approach 1b: always compute detail, derive fast by stripping

- **+** Trivially consistent (one computation).
- **−** Defeats the point: fast mode exists to be *cheaper*, not a projection
  of the expensive result. Every happy-path build pays full metadata cost.
- **Rejected** (though see 2d — *lazy* derivation of metadata is a different,
  viable shape).

### Approach 1c: one code path, detail as separately demanded queries (recommended)

Per the inventory: the default shape is **no mode key at all**. The core
queries stay as they are; each piece of detail data is its own sidecar query
(`spans(F)`, `docs(F)`, `refmap(FQ)`, `typemap(FQ)`) that the driver demands
by policy. All of them are produced by the **same code** as the core path —
never a parallel implementation. If a genuine compute-differently case ever
materializes, it upgrades to `parse(F, mode)` per [flavors.md](flavors.md)
Option C; nothing below changes shape when that happens, because the flavored
variant would still share the code path.

Sub-choices for how the shared code records (or skips) detail data:

| Variant | Shape | Pros / cons |
|---|---|---|
| runtime flag | `if recording { record(span) }` | simplest; branch cost is noise at parse speed; recommended start (only needed for lazy outputs) |
| generic recorder | `Parser<R: Recorder>` with a no-op ZST | true zero cost via monomorphization; more type plumbing; adopt only if profiling says the flag branches matter |
| always record | no branch at all | right answer where recording is nearly free (spans: the tokenizer walks offsets anyway) — see the eager/lazy call in "Multiple outputs per step" |

The outputs land as separately-fingerprinted results of one step — see
"Multiple outputs per step (decided)".

**Data-model note:** one code path does *not* mean one output type with
optional fields. `Parsed { core, meta: Option<ParseMeta> }` is the wrong
shape twice over: consumers can't tell from the type whether details exist,
and — decisively — `None` vs `Some` would make the core artifact hash
differently across modes, breaking the fingerprint stability that keeps the
flavor from cascading (see "No transitive flavor"). The core type must
contain **no mode-dependent fields at all**; the sidecar is a separate value
under a separate key, and detail consumers take it as an explicit parameter.
Presence of detail is a property of *which query was demanded*, not of a
field.

**Invariant (load-bearing):** demanding detail queries never changes core
results — for any input, the same programs are accepted and rejected, with
the same errors, resolution decisions, types, and values, whether or not any
sidecar was computed. With sidecars as pure add-on queries and error
recovery always-on, this holds by construction; property tests (below) guard
it against regressions.

## Question 2 — source pointers

The tension: runtime errors (`panic`, `unreachable`, later stack traces) need
source locations even in fast mode, so locations can't simply be omitted — but
putting spans in the AST means **every whitespace edit changes every
fingerprint downstream**, destroying the early cutoff the whole caching design
is built on (function-level cutoff via resolved-AST fingerprints,
`context.rs:98`).

The resolution: separate **identity** from **layout**. The semantic artifacts
carry a cheap, layout-independent *locator* per node; the mapping from locator
to concrete span lives in a **sidecar** that only detail-mode consumers touch.

### Approach 2a: absolute spans on every AST node

- **+** Simple; spans always at hand; what most toy compilers do.
- **−** Whitespace/comment edit → every span after the edit shifts → parse
  fingerprint changes → resolve fingerprints change → typecheck/exec cascade.
  Undoes invariant-level work already landed (location-free parse answers,
  function-level cutoff). Memory cost on every node in every cached artifact.
- **Rejected.**

### Approach 2b: node-id locators + span sidecar (recommended)

- The core AST stays span-free. Each node gets a **NodeId**: its preorder
  index **within its enclosing function** (see below for why per-function).
  A full locator is `(FQ, NodeId)` — and, for runtime use, the content digest
  of the file it was compiled from (already in the parse key).
- Detail-mode parse additionally produces a **span sidecar**:
  `NodeId → ByteSpan` per function, plus token/trivia info (doc comments,
  original identifier text) as further sidecars.
- Whitespace or comment edits change the sidecar but leave the token stream —
  hence node count, order, ids, and the entire core AST — **bit-identical**:
  parse reruns (it's the cheap leaf query), the core fingerprint matches, and
  early cutoff stops the cascade right there. Only the sidecar entry (used by
  nobody on the happy path) is new.
- To be explicit: the sidecar **does** go stale on every edit — that is the
  point, not a flaw. It is content-addressed (keyed by the file digest), so
  the old entry is never wrong, merely no longer demanded; and it is never
  patched, only recomputed from the new bytes **lazily, on demand**. A
  whitespace edit on the happy path therefore computes no sidecar at all.
  When one is computed later (an error appears), old locators still match it:
  NodeId assignment depends only on node structure, so an unchanged core
  guarantees unchanged ids, and old `(FQ, NodeId)` + fresh sidecar yields the
  correct shifted span. 2b's win is converting "everything downstream is
  stale" (2a) into "one lazy leaf entry is stale".

**Why per-function ids, not per-file:** with file-wide preorder numbering,
adding a node to function A shifts every id in function B below it, so B's
resolved AST changes and B's downstream cache entries die — silently undoing
the function-level cutoff that content keys currently provide. Ids must be
relative to the cutoff unit. (Same reasoning that led rustc to item-relative
spans for incremental.)

### Approach 2c: relative spans stored inline (rustc-style)

Spans on nodes, but encoded relative to the enclosing function's start.

- **+** Spans available without a sidecar lookup; invalidation confined to the
  edited function.
- **−** A whitespace edit *inside* a function still changes that function's
  fingerprint and its downstream — 2b confines it to nothing. Rebasing
  arithmetic throughout. Solves less than 2b at more complexity.
- **Rejected** for now; could later complement 2b if sidecar lookups ever
  become a hot path.

### Approach 2d: no stored spans — recompute by reparse on demand

rust-analyzer-style lazy source maps: never cache spans; when a diagnostic
needs one, reparse that file with recording on and read the map.

- This is really **2b with an eviction policy of "always"**: the sidecar is a
  content-addressed cache entry either way; whether it's kept or recomputed
  from bytes is a storage decision, not a design fork.
- **Adopt as the default policy:** sidecars are recomputable from bytes at
  leaf-query cost, so evict them aggressively; the roadmap's memory/disk
  tiering can revisit.

### The runtime story

Two distinct cases — lazy lookup only works in the first:

**In-session execution (the sandbox today).** `execute` is a query in the
same session, so a `panic` happens with the engine warm:

- Fast-mode execution artifacts carry only the locator — `(FQ, NodeId)` plus
  the source content digest. Tiny, and stable under any edit that doesn't
  change the function's code.
- On `panic`/`unreachable`, the runtime error surfaces the locator; the
  driver then runs the **detail parse of that one file** (upgrade path, same
  as compile errors), reads the span sidecar, and renders file:line:col plus
  a snippet.

**Emitted artifacts (future AOT backend).** A binary on another machine
cannot reparse anything — whatever it will print must be joined in at build
time, by a **sink** step (so mid-pipeline cutoff is unaffected either way):

- *Embed at emit:* the final assemble step depends on the span sidecar and
  bakes `file:line` into the artifact (Rust's panic messages). Honest cost:
  a whitespace edit now really changes the output bytes, so emit reruns —
  but only emit; you cannot cache away a real output difference.
- *Locators in the binary, spans in a side map:* the binary carries only
  `(FQ, NodeId)`; a separate map artifact links locators to spans (JS source
  maps; stripped binaries + dSYM/PDB). Panics print the locator; tooling
  symbolicates. The code artifact becomes layout-independent — whitespace
  edits rebuild only the map — and fast builds may skip the map entirely.
  Preferred direction when the backend exists.

**Stack traces make span demand per-build, not per-error.** Debug builds
want line numbers on every frame, so every debug emit demands the sidecar
for every file — "sidecars are only computed on errors" holds for the
in-session compile/test loop, **not** for AOT debug builds. The architecture
is unchanged (spans still enter no mid-pipeline key); only the cost model
shifts: debug builds always pay leaf-cost sidecar + line-table work at the
sink. To keep that cheap, split emit DWARF-style:

- **codegen** depends on the core only; outputs instructions *plus* a
  `code offset → NodeId` map. Layout-independent — whitespace edits leave it
  cached.
- **line-table assembly** joins codegen's offset map with the span sidecar
  into a `.debug_line`-style artifact (embedded section or side file per the
  options above).

A newline edit in a debug build then reruns: parse (cheap, cuts off),
sidecar, line-table assembly — never resolve, typecheck, or codegen.
Debug-vs-release is the *opt-level* flavor's call (embed / strip-to-map /
omit); it is orthogonal to fast-vs-detail mode, which remains about
compile-time diagnostic metadata. Stack traces need the sidecar *artifact*,
not detail *mode*.
- **Digest pinning:** the sidecar must be computed from the *bytes the
  program was compiled from*. The locator's content digest guarantees this:
  look up / recompute the sidecar under that digest. If the file has since
  changed on disk and the original bytes aren't recoverable, degrade
  gracefully to today's coarse `file::function` location rather than lying
  with wrong line numbers.
- This replaces the current `source_location: String` on `Expr::Panic` (which
  is the coarse fallback, kept as exactly that).

## Which steps are mode-sensitive

| Step | Detail impact | Notes |
|---|---|---|
| Read / content-hash | none | One entry regardless of what is demanded. |
| Parse | sidecar outputs | Core AST unchanged; `spans` / `docs` are further outputs of the same step, separately fingerprinted. |
| Resolve | sidecar outputs | Decisions unchanged; `refmap` is a further output. |
| Typecheck / exec | sidecar + sinks | Pass/fail, types, values unchanged; `typemap(FQ)` is a sidecar; diagnostics carry locators and are rendered by sink queries. |

Rule of thumb: detail data lives in *sidecar* and *sink* queries; core query
keys never mention it. (If a mode flavor is ever introduced, the analogous
rule: flavor only in sidecar-producing keys, never in mode-independent
inputs — otherwise the flavor fragments caches that should be shared,
[flavors.md](flavors.md) Option B's failure mode.)

## The upgrade-retry path

1. Compile + test everything in fast mode.
2. No errors → done. Nothing metadata-related was ever computed.
3. Error → the fast error is a cheap token: `{ kind, locator }`. No rendered
   string, no span.
4. Driver re-demands detail queries for **only the failing subtree** — the
   file(s)/function(s) named by the locators, not the whole program.
5. Render diagnostics from sidecars (span, snippet, notes, doc-comment
   context) and report.

Why the retry is cheap: the read/content layer is shared (no repeated IO or
hashing); core results are cache hits; the only new work is sidecar
computation for the affected files — leaf-query cost. In effect "retry the
compile in detail mode" is not a second compile at all; it's demanding a few
extra queries against a warm session.

Two policies over one mechanism:

- **Batch compiler:** fast-first, upgrade on error (above).
- **Editor:** detail mode for the open working set, fast for the rest of the
  closure — the IDE doesn't wait for errors to want hover/go-to-def.

### No transitive flavor — the upgrade must not cascade

The danger to design against: if mode lived in parse's key *and* resolve's key
hashed parse's key, then demanding detail-parse would mint a new resolve key,
a new typecheck key, and so on — the "retry" would be a clean recompile of the
whole program under a second flavor. That is the naive-flavor failure mode
([flavors.md](flavors.md) Option B), and it must not happen.

It doesn't, given the sandbox's existing key discipline: downstream keys hash
the producer's **core fingerprint**, not its query key (`store.rs:66` —
resolve is keyed on `hash(fq, parse fingerprint, …)`). The core is
mode-invariant by the load-bearing invariant, so its fingerprint — and every
key derived from it — is identical whether it came from a fast or detail run.
Three rules keep it that way:

1. Sidecar data exists only in **sidecar queries** (`spans(F)`, `refmap(FQ)`,
   …), never as fields of core results.
2. Mid-pipeline queries (typecheck, exec) consume cores and locators only —
   **never sidecars**. Their keys mention no detail artifact.
3. Sidecar consumers are **sinks** at the edge of the graph (diagnostic
   renderer, doc extraction, line-table assembly, IDE features); nothing else
   depends on them, so detail results can't leak into compile keys.

The upgrade set is therefore exactly `{ detail-parse(F) for each file F named
by a fast error's locators } + the renderer` — flat, demand-driven, not
transitive through the dependency graph. Granularity is the existing cutoff
unit (file/function), which is strictly better than per-crate/module: a crate
would re-derive sidecars for every file in it when one function failed. (If
Tel later grows crates, they'd matter here only as an *eviction/batching*
boundary, not as the upgrade unit.)

Two practical corollaries:

- One diagnostic may point at several files (error site + the definition it
  conflicts with); the demand set is the union of the error's locators —
  still a handful of files, still bounded.
- With many errors, collect **all** fast errors first, then batch the detail
  demands once. Never loop restart-compile-per-error; the fast pass already
  found everything (same-errors invariant).

## Multiple outputs per step (decided)

Core + sidecar from one parse is naturally *one execution, two outputs*.
**Decision: multi-output steps are the mechanism.** One execution records
several **named outputs on one answer record**, each with its **own
fingerprint**; dependents declare which output they consume, and their keys
hash that output's fingerprint only. The store is halfway there already:
resolve keys hash "the parse fingerprint" (`store.rs:66`); this generalizes
to "parse *core* fp" vs "parse *spans* fp". Precedents: Bazel multi-output
actions, Salsa projections-with-cutoff.

Rejected alternatives:

- **Separate sidecar queries** (re-run the tokenizer per sidecar demand) —
  works with zero engine changes, but duplicates leaf work systematically
  once debug builds demand spans every build, and doubles queue/cache
  traffic. Kept only as a conceptual fallback.
- **One fat result + projection steps** — compute everything, split via
  projection queries. Same dataflow *effect* as multi-output, but every
  projection is an extra step through the task queue, locks, and cache, and
  it forces the full detail result to always be computed (approach 1b
  through the back door). Multi-output gives the projection effect with
  per-output fingerprints instead of projection tasks.

**Eager vs lazy per output:** whether a run fills a given output always, or
only when that output was demanded (re-running with recording on first
demand), is decided per output. `spans` is **eager** — the tokenizer walks
byte offsets anyway, so recording them is nearly free, and a filled span
output never touches the core fingerprint. `refmap`/`typemap` decide when
their consumers exist. Eagerly produced sidecars remain aggressively
evictable (2d policy).

Further users of multi-output: codegen (instructions + `offset → NodeId`
map, see runtime story), resolve's already-bundled outputs (resolved AST +
registered function list), doc-comment tables.

## Interaction with caching & invalidation

Builds on the two-layer model in `../../doc/book/src/19a-compiler-internals/03-keys-and-fingerprints.md`:

- Core and sidecar results are distinct content-store entries under distinct
  query kinds; both valid forever by construction, both cacheable.
- **Invalidation is per position across all artifacts of a file:** a file
  change dirties its sidecar positions along with its core positions; never
  leave a stale span table alive because only the core was rechecked.
- Early cutoff applies as before — and, via 2b, whitespace edits cut off
  *at parse* even when sidecars are in active use (IDE, debug builds).
- Sidecars are recomputable from bytes → evict aggressively (2d policy).

## Concrete sandbox changes (when implemented)

- **No `Mode` enum, ever.** Parse becomes a multi-output step: outputs
  `core` and `spans`, separately fingerprinted on one answer record;
  dependents name the output they consume. `docs`, `refmap`, `typemap`
  follow as outputs of their steps when consumers appear.
- Extend the answer record to hold **per-output fingerprints**, and
  dependency edges / derived keys to reference `(step, output)` rather than
  `step`.
- Assign per-function preorder `NodeId`s in the core AST (parse or resolve —
  wherever function boundaries are first known; today that's resolve's
  `FuncData` split).
- Make parse error-recovering (always-on): report all errors it can, produce
  a partial core AST. Required for the same-errors invariant to survive any
  later mode split, and for IDE use regardless.
- Replace `Expr::Panic { source_location: String }` with a locator
  `{ func: FQ, node: NodeId }` (+ digest reachable via the parse key); keep
  the string rendering as the degraded fallback.
- Split errors: cheap `{ kind, locator }` values from core queries vs a
  renderer sink that consumes sidecars to produce spans, snippets, and
  suggestions.
- Driver policies: batch (demand sidecars only after a fast error), IDE
  (demand for working set), debug emit (demand `spans(F)` for line tables).

## Tests

- **Property/fuzz:** compile arbitrary inputs with and without every sidecar
  demanded; assert identical error sets, resolutions, and values (the
  invariant).
- **Recovery:** inputs with multiple seeded errors report all of them in one
  pass; valid inputs pay no recovery cost (parser does identical work).
- **Upgrade path:** seed an error; assert fast mode locates it and detail
  mode renders a span-accurate message for the *same* error, with zero extra
  file reads and recomputation confined to the failing subtree.
- **Cutoff under spans:** whitespace/comment-only edit with detail mode
  active → parse reruns, *nothing* downstream recomputes, sidecar updated.
- **NodeId stability:** edit function A; assert function B's ids, resolved
  fingerprint, and cache entries are untouched.
- **Runtime locator:** trigger `panic`; assert rendered line/col matches the
  compiled source, and the coarse fallback engages when bytes are gone.

## Prior art

The ingredients are each proven in production compilers; the driver-level
combination (fast compile, auto-upgrade the failing subtree) is the part
without a direct precedent.

- **rustc incremental** — hit exactly the whitespace problem: absolute spans
  in HIR made every edit invalidate everything downstream. Fix: item-relative
  span encoding plus special span treatment in its stable hashes (our 2c),
  and diagnostics cached/replayed as query results. Closest precedent for the
  *cache-invalidation* motive.
- **rust-analyzer** — span-free HIR; items carry positional `AstId`s; a lazy
  source-map query maps HIR back to syntax only when a diagnostic or IDE
  feature demands it. This is 2b + 2d almost verbatim, including per-item id
  numbering to confine invalidation.
- **Roslyn (C#)** — red-green trees: the shared green tree stores only node
  *widths*, no absolute positions, so it's reusable across edits; absolute
  spans materialize on demand in the red layer. Position-independent shared
  core = our fingerprint-stable core AST.
- **V8** — the preparser is a real two-mode parser: a fast skim that skips
  function bodies vs the full parse, with the hard-won invariant that both
  must report *identical errors*. Real-world evidence that the equal-semantics
  invariant is essential, subtle, and why we insist on one code path rather
  than parallel implementations.
- **clang / Go** — the counterpoint: make positions so cheap (a `u32` offset
  into a source manager / fileset) that "fast mode" keeps them anyway, and
  only line tables, snippets, and suggestions are computed lazily on the
  error path. That solves *batch speed* but not *fingerprint stability* —
  the u32s still shift on whitespace edits. Fine for clang, which has no
  content-keyed cutoff at that layer; fatal for Tel's content keys. Tel's
  motive is rustc's, not clang's — worth remembering when tempted to "just
  keep a u32 per node".

## Open questions

- Eager vs lazy per output beyond `spans` (eager, decided): `refmap` needs
  info the core walk has in hand anyway (likely eager), `typemap` may be
  bulky (likely lazy) — decide when their consumers exist.
- Does any step ever need to *compute differently* under detail (the
  watch-item: pretty names through monomorphization)? If yes, introduce the
  mode flavor per flavors.md Option C for that step only.
- Do doc comments belong to the parse sidecar or a third `docs(F)` query?
  Separate query keeps `tel doc` (currently deferred) fully decoupled.
- After an upgrade, keep the fast entry cached alongside detail? Yes — they're
  different flavors; let eviction policy decide.
- Where exactly ids are assigned (parse vs resolve) interacts with the
  file-level parse answer being shared across identical files — ids must not
  embed anything path-derived (same rule that keeps `Panic` location-free at
  parse today, `parse.rs:250`).
