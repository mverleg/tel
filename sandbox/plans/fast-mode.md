# Fast mode vs detail mode — design

Status: **draft / not implemented.** Roadmap: Phase 2, step 11 (see
[roadmap.md](roadmap.md)). Closely related to [flavors.md](flavors.md) — "mode"
is the first concrete flavor. ("Detail mode" was earlier called "IDE mode";
same thing — the IDE is one consumer of it, error rendering and doc extraction
are others.)

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

### Approach 1c: one code path, mode as a query-key flavor (recommended)

`parse(F)` becomes `parse(F, mode)` per [flavors.md](flavors.md) Option C
(per-query declared flavor subset). Fast and detail results are separate
*cache entries* of the same logical query, produced by the **same code**.
Inside the shared code, the two modes differ only in what they *record*.
Sub-choices for how that recording branches:

| Variant | Shape | Pros / cons |
|---|---|---|
| runtime flag | `if mode.detail { record(span) }` | simplest; branch cost is noise at parse speed; recommended start |
| generic recorder | `Parser<R: Recorder>` with a no-op ZST for fast mode | true zero cost via monomorphization; more type plumbing; adopt only if profiling says the flag branches matter |
| wrapper query | detail query = fast core (cache hit) + separately computed sidecar | cleanest when the sidecar is derivable without redoing the core (spans: re-tokenize only); doesn't work where fast *drops* info mid-computation |

Likely a mix per step: wrapper where the sidecar is additive, flag inside the
shared function where it isn't.

**Invariant (load-bearing):** for any input, fast and detail modes accept and
reject exactly the same programs, with the same resolution decisions, types,
and values. Mode may only change metadata richness. Enforced by construction
(one code path) and by property tests (below).

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

- Fast-mode execution artifacts carry only the locator — `(FQ, NodeId)` plus
  the source content digest. Tiny, and stable under any edit that doesn't
  change the function's code.
- On `panic`/`unreachable`, the runtime error surfaces the locator; the
  driver then runs the **detail parse of that one file** (upgrade path, same
  as compile errors), reads the span sidecar, and renders file:line:col plus
  a snippet.
- **Digest pinning:** the sidecar must be computed from the *bytes the
  program was compiled from*. The locator's content digest guarantees this:
  look up / recompute the sidecar under that digest. If the file has since
  changed on disk and the original bytes aren't recoverable, degrade
  gracefully to today's coarse `file::function` location rather than lying
  with wrong line numbers.
- This replaces the current `source_location: String` on `Expr::Panic` (which
  is the coarse fallback, kept as exactly that).

## Which steps are mode-sensitive

| Step | Mode in key? | Notes |
|---|---|---|
| Read / content-hash | **No** | Same bytes → one entry shared by both modes. Do **not** put mode in the read key. |
| Parse | Yes (or wrapper) | Core AST identical across modes; detail adds span/doc sidecars. |
| Resolve | Yes (or wrapper) | Decisions identical; detail adds a reference-location map (go-to-def). |
| Typecheck / exec | Mostly no | Pass/fail, types, values identical; their *diagnostics* pull sidecars on demand. Keep mode out of these keys if errors carry only locators. |

Rule of thumb: mode goes in the key of steps that *record extra metadata*,
never in the key of mode-independent inputs — otherwise the flavor fragments
caches that should be shared ([flavors.md](flavors.md) Option B's failure
mode).

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

1. Mode appears only in the keys of **sidecar-producing** queries (detail
   parse, detail resolve).
2. Mid-pipeline queries (typecheck, exec) consume cores and locators only —
   **never sidecars**. Their keys carry no mode.
3. Sidecar consumers are **sinks** at the edge of the graph (diagnostic
   renderer, doc extraction, IDE features); nothing else depends on them, so
   detail results can't leak into compile keys.

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

## Interaction with caching & invalidation

Builds on the two-layer model in `../../docs/cache-invalidation-problem.md`:

- Fast and detail entries are distinct content-store entries (flavored keys);
  both valid forever by construction, both cacheable.
- **Invalidation is per position across flavors:** a file change dirties all
  flavors of the affected nodes; never leave a stale detail result alive
  because only the fast flavor was rechecked.
- Early cutoff applies within each mode — and, via 2b, whitespace edits now
  cut off *at parse* even when detail mode is in use.
- Sidecars are recomputable from bytes → evict aggressively (2d policy).

## Concrete sandbox changes (when implemented)

- `enum Mode { Fast, Detail }`, threaded per flavors.md Option C — in parse
  and resolve keys only, never the read key.
- Assign per-function preorder `NodeId`s in the core AST (parse or resolve —
  wherever function boundaries are first known; today that's resolve's
  `FuncData` split).
- Detail parse returns core + `ParseMeta { spans: NodeId → ByteSpan, docs,
  idents }`; fast parse returns core only.
- Replace `Expr::Panic { source_location: String }` with a locator
  `{ func: FQ, node: NodeId }` (+ digest reachable via the parse key); keep
  the string rendering as the degraded fallback.
- Split errors: `FastError { kind, locator }` vs a detail renderer that
  consumes sidecars.

## Tests

- **Property/fuzz:** compile arbitrary inputs in both modes; assert identical
  error sets, resolutions, and values.
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

- Wrapper query vs in-key flavor per step (see 1c) — decide per step during
  implementation; parse likely wrapper (sidecar = re-tokenize), resolve likely
  flavored (reference map needs info fast mode drops mid-walk).
- Do doc comments belong to the parse sidecar or a third `docs(F)` query?
  Separate query keeps `tel doc` (currently deferred) fully decoupled.
- After an upgrade, keep the fast entry cached alongside detail? Yes — they're
  different flavors; let eviction policy decide.
- Where exactly ids are assigned (parse vs resolve) interacts with the
  file-level parse answer being shared across identical files — ids must not
  embed anything path-derived (same rule that keeps `Panic` location-free at
  parse today, `parse.rs:250`).
