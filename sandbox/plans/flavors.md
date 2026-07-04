# Query "flavors" — design draft (pros & cons)

Status: **draft / not implemented.** Roadmap: Phase 3, step 15 (see
[roadmap.md](roadmap.md)). Leaning **towards** adopting flavors; this doc lays
out what they are, the trade-offs, and a recommended shape. The first concrete
flavor is fast-vs-IDE mode ([fast-mode.md](fast-mode.md)).

## What a flavor is

A **flavor** is a dimension of a query's *cache key* that isn't the primary
subject (the file / function) but still affects the result — an ambient config
under which the query is answered. Today the sandbox keys are just `file_path`
(`ParseId`) and `FQ` (`ResolveId`/`ExecId`): **no flavor dimension at all**, so
only one configuration can be cached at a time.

Candidate flavors floated for Tel (from `qcompiler/README.md`):

| Candidate | Real nature | Verdict |
|---|---|---|
| fast vs IDE mode | ambient, result-affecting (metadata richness) | **genuine flavor** — adopt first |
| debug / opt level | ambient, result-affecting (lowering/codegen) | genuine flavor, but **backend only** — out of current scope |
| which source filesystem (disk vs web IDE) | changes the *bytes* | **not a flavor** — resolves to content; handle at the read/content layer |
| which cache | storage location of results | **not a flavor** — storage-layer concern, not a key dimension |
| generics / references (monomorph for types) | part of the query *subject* | **not a flavor** — model as query *parameters* (`monomorph F for (U,T)`) |

The important realization: **most proposed "flavors" belong elsewhere.** Only
mode (now) and opt-level (later, with the backend) are truly ambient
result-affecting flavors. Folding the others into "flavors" is a conceptual
trap:

- **Source backend → content addressing.** Two filesystems that yield identical
  bytes *should share* the cache — content-addressing already gives that for
  free. Keying by "which fs" would wrongly split identical content.
- **Generics → parameters.** Monomorphization args are what the query is *about*,
  not the environment it runs in. They belong in the query id, not an ambient
  flavor set.

## How to represent flavors

### Option A — flatten into every query key

Every `*Id` struct gains fields for all applicable flavors.

- **+** Explicit, type-safe, obvious in the graph.
- **−** Bloats every key; every query must thread every flavor even when
  irrelevant; easy to accidentally key a flavor-independent step by a flavor.

### Option B — ambient environment hashed into all keys

One `Flavor`/`Env` value carried in the context, mixed into every cache key.

- **+** One knob; queries don't each enumerate flavors; trivial to add a flavor.
- **−** **Cache fragmentation** — the killer. Parsing a file doesn't depend on
  opt-level, but if opt-level is in the ambient env it's hashed into the parse
  key, so you cache (and recompute) parse once per opt-level for no reason.
  Over-keying → redundant work + memory blow-up, multiplied across every
  independent flavor (debug×ide×target×…).

### Option C — per-query *declared* flavor subset (recommended)

Each query kind declares which flavors it actually depends on; its key includes
only those. Read/parse depend on nothing (content-addressed) or only `mode`;
codegen depends on `opt-level` + `target`; etc.

- **+** No fragmentation — flavor-independent steps are shared across flavors
  (e.g. one parse serves both debug and release).
- **+** Correct multi-config caching without combinatorial blow-up.
- **−** More machinery: each query must declare its flavor dependence, and the
  key builder must respect it. Worth it.

## Interaction with caching & invalidation

- Flavors sit **on top of** the content-addressed / two-layer model
  (`../../docs/cache-invalidation-problem.md`). A flavored key is
  `(subject, declared-flavor-subset) → digest`.
- **Invalidation is per position, across all flavors.** A file change dirties
  every flavor of the affected nodes; you must not leave a stale
  release-mode result alive because only the debug flavor was rechecked.
- **Content-addressing stays flavor-free** where the flavor doesn't apply — this
  is exactly what prevents fragmentation and is the reason to keep source
  backend out of the flavor set.

## Pros & cons of adopting flavors at all

**Pros**
- One uniform mechanism for mode (now), opt/target (later), instead of ad-hoc
  special-casing per config.
- Lets multiple configurations coexist in cache (fast+ide, debug+release)
  without collisions — the enabler for the fast→ide upgrade path.
- Future-proofs for cross-compilation / multiple targets.
- Makes "what does this result depend on" explicit and auditable.

**Cons**
- Complexity: key management, per-query flavor declaration, cross-flavor
  invalidation.
- Fragmentation risk if done as ambient env (Option B) — mitigated by Option C.
- Combinatorial memory if many independent flavors are cached at once — needs
  the deferred memory/disk tiering eventually; until then, evict recomputable
  flavored entries aggressively.
- Temptation to misuse flavors for inputs (fs) or subjects (generics), muddying
  the model — call this out in review.

## Recommendation

1. **Adopt flavors, via Option C** (per-query declared subset), not an ambient
   env.
2. **Start with a single flavor: `mode` (Fast | Ide).** Ship the mechanism with
   one real user; extend later.
3. **Keep source backend and cache selection out** — backend resolves to content
   at the read layer; cache selection is storage-layer.
4. **Model generics/references as query parameters**, not flavors, when
   monomorphization lands.
5. **Add opt-level / target flavors only with the backend** (out of current
   front-end scope).

## Open questions

- Representation of the declared subset: a per-kind const list, a trait, or a
  derive? Start simple (explicit key fields per kind).
- Do any *front-end* steps depend on opt-level? If truly none, opt-level never
  touches the front-end cache and only appears once codegen exists.
- How do flavors compose with the fast→ide upgrade — is `mode` special (drives
  control flow) versus other flavors that are pure key dimensions? (Mode is
  special only in that the *scheduler* picks it; as a key it's an ordinary
  flavor.)
