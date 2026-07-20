# Swapping the query engine out of the sandbox into the real language

The [roadmap](roadmap.md) is the plan for *building* the query engine inside the
sandbox. It ends where this document begins: it lists "swappable codegen /
backend" and the real front-end queries under **"Deferred / out of scope for the
query engine"**, because the sandbox exists to mature the *engine*, not the
language. This document is the missing other half — the ordered plan for taking
the matured engine and making it drive the **real** Tel compiler, retiring the
toy Lisp it grew up on.

Read [roadmap.md](roadmap.md) first for engine status; read this for the
transition.

---

## 1. Where we are today

Three compiler-shaped crates exist in this workspace, and it is worth being
precise about what each one is, because the swap is really a *convergence* of
them:

| Crate | LOC (src) | What it is | Role in the swap |
|-------|-----------|------------|------------------|
| `sandbox/` | ~8.7k | The query engine, exercised by a toy Lisp (4 phases: parse → resolve → typecheck+mono → exec/codegen). Incremental, watched, persistent, cached. | **The engine we keep.** The Lisp is scaffolding. |
| `qcompiler/` | ~0.2k | The *first* attempt at a query-based compiler for the real language. `parse.rs`/`source.rs` are empty; it is design notes (`README`, `TODO`) plus a `db`/`engine`/`step` skeleton. | **Superseded.** Its open questions (cycles, panics, pending results, dedup) are already answered in the sandbox — see roadmap's "Where the sandbox is already ahead of qcompiler." It becomes the *design record*, not code we grow. |
| `compiler/` + `parser/` + `ast/` + `hir/` + `telir/` | real front-end | The existing, non-query Tel front end: tokenizer/parser, AST, scoping, a stubbed HIR, and the `telir` IR with its multi-language runtimes. Predates the engine. | **The queries we graft on.** Its phases become query kinds. |

So "swap it out of the sandbox to the real language" concretely means: **lift the
engine core out of `sandbox`, and re-point its query bodies from the toy Lisp's
phases at the real language's phases (the `compiler`/`parser`/`ast`/`hir`
pipeline), retiring `qcompiler` as the by-then-obsolete prototype.**

### Engine readiness (from the roadmap)

- **Phase 0 — correctness foundations:** done (cycle detection wired, content
  keys, schema hash).
- **Phase 1 — in-memory caching pipeline:** done (content store vs binding
  layer, cached resolve/mono, transitive validation, early cutoff).
- **Phase 2 — incremental & watch:** done, including the OS file watcher and the
  first slice of fast/IDE mode (span sidecar + in-session `path:line:col`
  upgrade for panics and type/resolve errors).
- **Phase 3 — persistence & scale:** LMDB content store done; query flavors
  mechanism done. **Not built:** memory/disk tiering + eviction (item 13) and
  external deps as sealed leaves (item 13b) — *both have decided designs*
  ([concurrency-and-eviction.md](concurrency-and-eviction.md),
  [external-deps.md](external-deps.md)) but no implementation. Pluggable source
  backends descoped.
- **Phase 4 — hardening & performance:** **not built** — context-leak
  prevention (item 16), lock-free during compile (17), boxed recursive awaits
  (18), threadpool parse (19), cleanups (20).

In one line: **the engine is feature-complete for a correct, incremental,
persistent, single-writer compiler, and is missing (a) concurrency + eviction,
(b) external deps, and (c) the Phase-4 hardening — before it is evolved in place
into the real compiler, which is what this document plans.**

---

## 2. What actually gets reused, and the central obstacle

The reusable core — the part with no opinion about what a "parse" or a "type" is
— is roughly:

- `context.rs` / `graph.rs` — the demand-driven pull, dependency + reverse-edge
  recording, two-pass invalidation, cycle detection.
- `store.rs` — the content store vs binding layer split, early-cutoff by
  fingerprint, single-flight via `async-lazy`.
- `keys.rs` — content keys, result fingerprints, `SCHEMA_VERSION`, xxh3 hashing.
- `disk.rs` / `portable.rs` — the LMDB tier and the Sym-free portable encoding.
- `monitor.rs` — the file-watcher backends and change batching.
- `trace.rs`, `flavors.rs` — step tracing and the flavor key dimension.

The toy-language-specific part is `parse.rs`, `resolve.rs`, `typecheck.rs`,
`types.rs`, `execute.rs`, `codegen.rs` — the query *bodies* and their AST types.

**The engine core is welded to the concrete query kinds, and we keep it that
way — on purpose.** The coupling shows up in exactly these places:

- `keys.rs::QueryKind` is a **fixed enum** — `Parse | Resolve | Mono | Exec |
  Spans`.
- `store.rs::ContentStore` has **hardcoded fields** (`parse`, `resolve`, `mono`,
  span sidecar) with concrete answer types (`PreExpr`, `ResolveAnswer`,
  `MonoAnswer`, `SpanTable`), not a generic `kind → cache` map.
- `Error`, the `StepId`/`ExecId` families, and the portable encoding all name
  the phases directly.

**Decision (2026-07-18): do *not* generalize the engine over an abstract set of
query kinds.** An abstraction earns its tax only when it has more than one
concrete instantiation, and the engine will only ever host **one** query-kind
set — the real language's phases. The toy Lisp was always scaffolding, not a
permanent second client. A generic `<K: QueryKind>` core would pay in threaded
generics / erased answer types / a harder `portable.rs` / worse compile errors,
wrapped around the *subtlest* code in the system (invalidation), for zero
runtime benefit and a plurality that will never exist.

Crucially, the coupling is **shallow**: a fixed enum plus a few concrete
`ContentStore` fields. The valuable part — the dependency graph, reverse-edge
invalidation, early cutoff, cycle detection, content keys, the disk tier — is
already kind-agnostic *in its logic* while naming concrete kinds. Adding a real
`Lower` kind is one enum variant + one struct field + one match arm, not a
plugin registry.

So the swap is **not** "extract a generic engine and register real queries
against a stable API." It is: **evolve the sandbox engine's concrete kinds into
the real language's kinds, in place.** The mechanism code stays concrete and
readable; only the query *bodies* and their answer types change.

**The light seam we *do* keep.** "Not generic over kinds" is not the same as
"the mechanism reaches into AST internals." The graph/cache/invalidation code
should touch answers only through a tiny surface each concrete answer type
implements — `content_key()`, `fingerprint()`, `to_portable()`/`from_portable()`
— rather than pattern-matching AST guts inside the engine. That is a handful of
trait impls on concrete types, not a parameterized engine: ~90% of the
architectural cleanliness for ~10% of the abstraction cost, and the invalidation
code stays concrete.

**The one thing this forecloses** (recorded so it is a choice, not an accident):
the engine cannot double as a reusable, salsa-like *framework* for third parties
to build their own incremental compilers. That is not a goal — the goal is Tel.
If a standalone-framework product ever becomes a goal, revisit; generalization
is the price of that, and it is not worth paying speculatively now.

---

## 3. Strategy — evolve the kinds in place (chosen)

Per the §2 decision, there is one strategy and it is deliberately the simple one:
**evolve the sandbox crate's concrete query kinds into the real language's
phases, one kind at a time, keeping the graph/cache/invalidation/watch/disk
machinery untouched** (the crate keeps its name until the toy language is gone;
the rename is deferred to S5, §8).

1. Keep `context.rs` / `graph.rs` / `store.rs` / `keys.rs` / `disk.rs` /
   `portable.rs` / `monitor.rs` / `trace.rs` / `flavors.rs` as they are (concrete
   mechanism). Introduce the *light seam* from §2 (small answer-interface trait
   impls) so the mechanism stops reaching into AST internals — a refactor that
   pays for itself immediately by making the body swaps below local.
2. Replace the toy query bodies with real ones, bottom-up (parse → resolve →
   typecheck → mono → lower), editing `QueryKind` and `ContentStore` concretely
   as each real kind lands (add a variant, add a field, add a portable arm).
3. As each real kind comes up, the toy tests for that phase are replaced by the
   real-language conformance corpus for it (§5); the machinery tests
   (invalidation, early cutoff, watch, persistence) are repointed from toy
   fixtures to small real `.tel` fixtures — they test the mechanism and do not
   care which language the fixture is in.

**Why this over the alternatives.** Two alternatives were considered and
rejected:

- *Generalize then extract a generic engine* — rejected in §2 as premature
  abstraction for a one-instantiation system.
- *Fork the engine next to the real front end and hand-respecialize* — rejected
  because forking creates two divergent copies of the subtle invalidation code.
  Evolving in place keeps a single copy that is continuously exercised by
  whichever tests (toy, then real) are live at the time.

The toy language is not preserved as a permanent generic-core witness (there is
no generic core to witness); it is consumed as it is replaced. Its value was
getting the engine correct *before* the real front end existed — value already
banked in the shipped Phases 0–3.

---

## 4. Readiness gate — what must hold in the sandbox before we swap

Do **not** start grafting real queries until these are true, because each one is
far cheaper to get right against the toy language:

- [ ] **Item 16 (context-leak prevention).** The real compiler will have many
  query bodies written over time; the scope-safety contract (a step cannot
  smuggle the context out) must be *enforced by construction*, not by
  convention, before third parties write queries. This is the one Phase-4 item
  that is a true prerequisite, not just polish.
- [ ] **Light answer-seam (§2, §3.1).** The mechanism touches answers only
  through `content_key()`/`fingerprint()`/portable impls, not by matching AST
  internals — so each body swap in §3.2 is local. This replaces the
  (rejected) generic-core refactor: much smaller, and validated by the existing
  toy tests staying green.
- [ ] **Concurrency + eviction (item 13).** Real projects are large; an
  unbounded in-memory cache and a single-writer run loop are fine for the toy
  suite but not for a real workspace. Design is decided
  ([concurrency-and-eviction.md](concurrency-and-eviction.md)); it must be
  built.
- [ ] **External deps as sealed leaves (item 13b).** Real projects have
  dependencies; the sealing/provenance model must exist before the real
  language's imports point at packages. Design decided
  ([external-deps.md](external-deps.md)).
- [ ] **Portable encoding routes through the seam.** Each answer type carries
  its own `to_portable()`/`from_portable()` (the seam from §2), so adding a real
  kind is one concrete arm, not a `portable.rs` rewrite. This is expected work
  per kind, not a blocking prerequisite — listed here only so the seam lands
  before the first real kind does.

Deferable past the swap (do in the real crate): lock-free compile (17), boxed
recursive awaits (18) — though 18 may become *urgent* under the real language's
deeper resolve chains and should be watched, threadpool parse (19), the
remaining fast-mode sidecars (`refmap`/`typemap`/`docs`, the always-on
error-recovering parser, the multi-output record).

---

## 5. Testing workflow — test-first, Rust-agnostic conformance corpus

**Discipline: for each language feature, write the tests first — features,
feature interactions, edge cases — and only then implement it.** The migration
phases below (S3–S5) are gated on a *growing conformance corpus*, not just on the
Rust machinery tests. Two tiers of test exist and must stay separate, because
only one of them is Rust-agnostic:

- **Engine-machinery tests (stay in Rust).** Caching, invalidation, early
  cutoff, incrementality, watch, persistence, cycle detection — the existing
  `sandbox/tests/*.rs`. These test the *engine*, which is Rust and does not
  port, so they are legitimately coupled to the `Compiler` API. They are not the
  target of the "Rust-agnostic" goal and should not be contorted to meet it.
- **Language/feature conformance tests (Rust-agnostic — the focus here).** A
  corpus of programs plus **declarative expectations as data**, driven by a thin
  harness rather than hand-written Rust assertions. This is where "lots of tests
  covering features / interactions / edge cases, written before the impl" lives.

**Build on the convention that already exists.** The real compiler already
annotates test programs with a header comment — `# tel-test: parse-only`,
`# tel-test: should-fail` (`compiler/src/examples.rs`), and the sandbox already
compiles an `examples/` corpus via a `build.rs`-generated harness. Grow that into
a full expectation vocabulary rather than inventing a parallel mechanism:
expected stdout, expected error *kind* plus `path:line:col` (the fast-mode
locators from roadmap item 11 make the location checkable), expected
success/failure, and which phase to stop at. Keep every expectation as **data**
(inline `# tel-test:` header or a sidecar `.expected` file) so the same program
can be replayed against any implementation.

**Why Rust-agnostic pays off here specifically.** A data-driven corpus runs
against *more than one backend*:

- the sandbox interpreter and the Python codegen backend already cross-check each
  other (`tests/codegen.rs`) — that is the pattern generalized;
- after the swap, the same real-language corpus runs across `telir`'s language
  runtimes (`rust`/`java`/`python`/`typescript`, `telir/run.sh`).

One corpus, many backends, is exactly the **cross-implementation conformance
suite the swap needs as its acceptance criterion** — the thing that proves the
real language behaves identically no matter which host it is embedded in (Tel's
whole USP). Rust-coupled tests could never carry that guarantee across the
non-Rust runtimes.

**Where it plugs into the migration.** Each real query kind added in S4 is
preceded by its slice of the corpus (test-first), and a phase is not "done" until
the real language passes the corpus written for it — the corpus is the
acceptance gate, superseding "it compiles." The toy Lisp (`.telsb`) and real Tel
(`.tel`) have different surface syntax, so the *programs* are per-language, but
the **expectation format, the harness shape, and the discipline are shared** —
and both feed the same "run it across every backend" check.

## 6. Migration phases (once the gate is green)

**S1 — Introduce the light seam.** Land the small answer-interface refactor from
§3.1 (mechanism touches answers only through
`content_key()`/`fingerprint()`/portable impls), with the toy Lisp still riding
on it. The crate keeps its `sandbox` name for now — the rename is deferred to S5
(§8). All existing sandbox tests stay green; nothing about the real language
yet. A pure, reviewable refactor commit — much smaller than a generic-core
extraction, which §2 rejected.

**S2 — Map the real front end onto query kinds.** Enumerate the real phases and
assign each a query kind and a stable key:

- `Parse` — real tokenizer + parser (`parser/`, `ast/`) → real AST. Leaf read
  stays fused with parse, as in the sandbox.
- `Resolve` / scoping — `compiler/src/scoping` becomes the resolve query; real
  imports (no `::`, per qcompiler history) drive the same-kind import edges the
  runtime cycle detector already covers.
- `Typecheck` — the real type system (traits/bounds: `Number`, `Integer`,
  `Currency`; generics over `struct Point<N: Number>` etc.). Richer than the toy
  numeric inference; expect this to be the largest single body.
- `Monomorphize` — real generics, keyed as query *parameters* (per
  [flavors.md](flavors.md): generics are parameters, not flavors).
- `Lower` to `hir` / `telir` — the real IR. This is the query the toy Lisp never
  had a real analogue for (its "backend" was interpret-or-emit-Python as a
  demo); wiring `telir` in is genuinely new surface.

Decide the query-kind *ordering* (roadmap Phase 0.1: kinds are ordered and may
only call down) for the real phase set, so cross-kind cycles stay impossible by
construction and only same-kind import cycles need runtime detection.

**S3 — Vertical slice: `parse` only, end to end.** Route one real `.tel` file
through the engine for the parse kind alone, behind the daemon and the watcher,
with the disk cache. Prove incremental + early cutoff on a real file (a
whitespace edit re-parses one file and recomputes nothing) before adding
resolve. This is the analogue of the sandbox's Scenario A/B tests, on real
input.

**S4 — Fill the pipeline upward.** Add resolve, then typecheck, then mono, then
lower — one kind per commit. Each kind is **preceded by its slice of the
conformance corpus (§5, test-first)** covering that feature, its interactions
with earlier features, and its edge cases; the commit lands the impl that turns
those tests green. Alongside the corpus, each kind carries its own
incremental/invalidation machinery test on real code, reusing the sandbox test
*shapes*. Fast/IDE mode: reuse the span sidecar and the error-path
`path:line:col` upgrade already built; real errors are richer, so the
located-error wrapping (`Located<E>`) extends to the real error types (and the
corpus asserts those locations as data).

**S5 — Retire the last toy remnants and `qcompiler`.** By here the toy query
bodies have already been replaced kind-by-kind (§3.2), so most of the Lisp is
gone; S5 removes whatever toy-only scaffolding is left (the interpret/emit-Python
demo backend, any remaining `.telsb` fixtures the machinery tests no longer need)
once the real pipeline passes the real example suite (`compiler/examples/*.tel`),
and deletes `qcompiler`'s dead skeleton, preserving its `README`/`TODO` design
notes as historical record (or folding the still-relevant bits into `docs/`). If
a tiny synthetic fixture is still the fastest way to exercise the machinery
tests, keep *that* — but it is a fixture, not the toy language.

**S6 — Real backends as flavors.** Only now do target/opt flavors get real
customers (roadmap item 15's "remaining"): `telir`'s multiple language runtimes
are the swappable codegen that was out of scope for the engine. Opt-level
already exists as the first flavor; real targets slot into the same mechanism.

---

## 7. Risks

- **In-place evolution destabilizes the shipped engine.** Swapping query bodies
  edits the crate that already works. Mitigation: the mechanism (graph/cache/
  invalidation) is not touched — only bodies and answer types are; the light
  seam (§3.1) keeps each swap local; the machinery tests stay green throughout
  as fixtures move from toy to real.
- **The real type system doesn't fit the mono/answer shapes** the toy language
  implied (traits, bounds, generic structs are strictly richer). Mitigation: S2
  is a paper mapping before any code; the vertical slice (S3) deliberately
  starts at parse, the phase least likely to surprise.
- **`telir` lowering is new surface with no sandbox analogue.** Mitigation:
  it is last (S4/S6), after the incremental machinery is proven on the phases
  that *do* have analogues.
- **Deep recursion on real programs** overflows the stack before item 18 is
  done. Mitigation: watch for it during S4; promote boxing (18) from
  "deferable" to "urgent" the moment a real example trips it.
- **Two live compilers during the transition.** The legacy non-query
  `compiler/` path must keep working until S5 completes. Mitigation: the engine
  path is additive; nothing in `compiler/` is deleted until the engine passes
  the same example suite.

---

## 8. Open questions

- **Crate rename — timing decided (2026-07-20): defer it.** Keep the `sandbox`
  name through the transition and let the rename land as one mechanical commit at
  S5, when the toy language is gone anyway — renaming up front is pure churn
  (`sandbox-daemon`, Cargo deps, IDE configs) for no functional benefit, and the
  owner is indifferent to timing ("whatever is easier"). Still loose but
  low-stakes: the *target* name (`telc`? fold into the existing `compiler`?). No
  separate engine crate is extracted, so this is a naming/placement call, not an
  architectural boundary.
- **Ordering of the real query kinds** — does `telir` lowering sit above or
  beside monomorphization, and where do the multi-language backends attach
  relative to the flavor mechanism.
- **How much of `qcompiler`'s design notes** (`keys-and-invalidation.md`,
  `execution-and-recovery.md`, `content-addressed-vs-verifying-trace.md` in
  `docs/`) is already superseded by what shipped in the sandbox vs still
  authoritative — worth a reconciliation pass before S5 deletes the crate.
- **Whether concurrency/eviction (13) and external deps (13b) truly gate the
  swap** or can land in parallel with S2–S3 against real code. Listed as gates
  here on the conservative assumption that getting them right is cheaper against
  the toy suite; revisit if S2 is ready first.
- **Conformance expectation format (§5).** Inline `# tel-test:` header vs a
  sidecar `.expected` file per program; how errors are matched (exact
  `path:line:col` vs error-kind-only, to keep tests robust to message wording);
  whether feature-*interaction* coverage is hand-written or partly generated
  (combinatorial), given the toy corpus already has a project generator; and
  where the corpus + its harness physically live so one runner drives both the
  engine's example language and the real language across every `telir` backend.
