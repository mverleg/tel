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
(b) external deps, and (c) the Phase-4 hardening — plus the generalization work
this document is about.**

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

**The obstacle: the engine core is not query-kind-generic yet.** It is welded to
the toy language in exactly the places that matter:

- `keys.rs::QueryKind` is a **fixed enum** — `Parse | Resolve | Mono | Exec |
  Spans`.
- `store.rs::ContentStore` has **hardcoded fields** (`parse`, `resolve`, `mono`,
  span sidecar) with concrete answer types (`PreExpr`, `ResolveAnswer`,
  `MonoAnswer`, `SpanTable`), not a generic `kind → cache` map.
- `Error`, the `StepId`/`ExecId` families, and the portable encoding all name
  the toy phases directly.

This is the single most important thing to understand about the swap: it is
**not** a clean "extract an engine crate, register real queries against a stable
API" operation, because that stable, kind-agnostic API does not exist yet.
Producing it is the bulk of the work.

---

## 3. Two strategies

### Strategy A — Generalize in place, then extract (recommended)

1. Inside the sandbox, refactor `QueryKind`/`ContentStore`/keys/portable so the
   engine is parametric over an abstract set of query kinds and their answer
   types (a trait per kind: stable key, fingerprint, portable encode/decode),
   with the toy Lisp as its *first client* rather than its hardcoded innards.
2. Prove the generalized engine still passes every existing sandbox test
   (incremental, invalidation, cache, spans, codegen-agreement, generated
   project) — the toy language is now an ordinary consumer of the generic core.
3. Extract the generic core into its own crate (working name `telc-engine` /
   `qengine`), leaving the toy Lisp behind in `sandbox` as an example consumer
   and regression suite.
4. Add a *second* consumer: the real front end.

**Why recommended.** The toy language stays as a continuously-green,
fast-to-compile witness that the generic core is correct. Every generalization
step is validated by an existing test before the real language — big, slow,
still-changing — is ever in the loop. The risky refactor and the risky new
client are never entangled.

### Strategy B — Fork the skeleton, re-specialize by hand

Copy the engine modules next to the real front end and hand-edit the concrete
types (`PreExpr` → real AST, add a real `Typecheck`/`Monomorphize`/`Lower`
kind, etc.), keeping only the invalidation/caching/watch machinery.

Faster to first light, but it forks the engine: two divergent copies of the
subtle invalidation code, and the sandbox tests no longer guard the version that
ships. Acceptable only if generalization (A.1) proves genuinely intractable —
which we will not know until we try it, so **do Strategy A and fall back to B
only on evidence.**

---

## 4. Readiness gate — what must hold in the sandbox before we swap

Do **not** start grafting real queries until these are true, because each one is
far cheaper to get right against the toy language:

- [ ] **Item 16 (context-leak prevention).** The real compiler will have many
  query bodies written over time; the scope-safety contract (a step cannot
  smuggle the context out) must be *enforced by construction*, not by
  convention, before third parties write queries. This is the one Phase-4 item
  that is a true prerequisite, not just polish.
- [ ] **Generic query-kind core (Strategy A.1–A.2).** The engine is parametric
  over kinds/answers and the toy language rides on it with all tests green.
- [ ] **Concurrency + eviction (item 13).** Real projects are large; an
  unbounded in-memory cache and a single-writer run loop are fine for the toy
  suite but not for a real workspace. Design is decided
  ([concurrency-and-eviction.md](concurrency-and-eviction.md)); it must be
  built.
- [ ] **External deps as sealed leaves (item 13b).** Real projects have
  dependencies; the sealing/provenance model must exist before the real
  language's imports point at packages. Design decided
  ([external-deps.md](external-deps.md)).
- [ ] **Portable encoding is kind-extensible.** Adding a real query kind must
  not require touching `portable.rs` by hand for each one — it falls out of the
  per-kind trait from A.1.

Deferable past the swap (do in the real crate): lock-free compile (17), boxed
recursive awaits (18) — though 18 may become *urgent* under the real language's
deeper resolve chains and should be watched, threadpool parse (19), the
remaining fast-mode sidecars (`refmap`/`typemap`/`docs`, the always-on
error-recovering parser, the multi-output record).

---

## 5. Migration phases (once the gate is green)

**S1 — Extract the generic core crate.** Land Strategy A.3. Sandbox depends on
the new engine crate; all sandbox tests green; nothing about the real language
yet. This is a pure, reviewable refactor commit.

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
lower — one kind per commit, each with its own incremental/invalidation test on
real code, reusing the sandbox test *shapes*. Fast/IDE mode: reuse the span
sidecar and the error-path `path:line:col` upgrade already built; real errors
are richer, so the located-error wrapping (`Located<E>`) extends to the real
error types.

**S5 — Retire the toy path and `qcompiler`.** Once the real pipeline passes the
real example suite (`compiler/examples/*.tel`) through the engine: demote the toy
Lisp to an engine-crate example/test fixture (keep it — it is a fast regression
witness), and delete `qcompiler`'s dead skeleton, preserving its `README`/`TODO`
design notes as historical record (or fold the still-relevant bits into
`docs/`).

**S6 — Real backends as flavors.** Only now do target/opt flavors get real
customers (roadmap item 15's "remaining"): `telir`'s multiple language runtimes
are the swappable codegen that was out of scope for the engine. Opt-level
already exists as the first flavor; real targets slot into the same mechanism.

---

## 6. Risks

- **Generalization proves intractable (A.1).** Mitigation: it is the first thing
  we attempt, gated before any real-language work; failure surfaces early and
  the fallback (Strategy B) is known.
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

## 7. Open questions

- **Crate naming / placement** of the extracted engine (`telc-engine`?
  `qengine`? absorb into `telc-cache`?) and whether the toy Lisp lives on inside
  it or in a sibling `examples` crate.
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
