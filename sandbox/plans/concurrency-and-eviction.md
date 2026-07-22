# Concurrency, dedup, and cache eviction — design

Status: **direction decided 2026-07-10; Phase A implemented 2026-07-22
(primitive + compaction, proven in isolation); Phases B–D not yet started.**
Covers roadmap
[item 13](roadmap.md) (memory/disk cache tiering + eviction) and the two
concurrency asks that turned out to share one mechanism with it: real
**parallel queries** with a represented **`Pending`** state, and **input
dedup** ("don't enqueue identical queries"). It also delivers roadmap
[item 16](roadmap.md) (prevent cache state leaking outside its scope) as a
*consequence* of the eviction design rather than a separate feature.

Related: [keys-and-invalidation.md](../../docs/keys-and-invalidation.md) (the
two-layer model this refines), [daemon.md](daemon.md) (the long-lived process
that makes eviction necessary at all), [roadmap.md](roadmap.md) Phase 3–4.

---

## The one observation that ties it together

Parallel queries, input dedup, `Pending` state, and eviction are not four
features — they are four properties of the **primitive under each content-store
table**. Today that primitive is inconsistent:

- `parse` → `async-lazy::Cache`: single-flight (so `Pending` already exists as
  `ALazy::is_initializing`), borrow-returning (`&Result<…>`), lock-free — but
  **append-only**, so it cannot evict in place.
- `resolve`/`mono`/`spans` → plain `DashMap`: evictable (they hand out clones)
  but with **no single-flight**, so concurrent demands for one key duplicate
  work.

Each half has exactly what the other lacks. The decision below keeps the good
half of `async-lazy` (borrows + single-flight) and adds eviction in the one
window where it is free.

## Decision 1 — never `Arc<V>` of an entry; owned clones or short-lived borrows

Two different `Arc`s must not be confused:

- **`Arc<Global>` — the whole-store handle. Kept, and required.** It is how
  parallel tasks share the store (`context.rs:484` already spawns import tasks
  that each own a `this.clone()`), and it is *not* a leak: you cannot use a
  store handle to pin an individual cache entry past a scope.
- **`Arc<V>` — a handle to one entry. Refused.** A handed-out per-entry `Arc`
  can be cloned and stashed anywhere, keeping its entry — and transitively
  everything it references — alive past the query. That defeats eviction (you
  cannot reclaim what someone still holds) and blurs ownership. This is the
  scoping hole, and roadmap item 16 ("don't smuggle cache state out of scope")
  is exactly the rule against it.

So consumers get an entry's value as an **owned clone** (across task/await
boundaries) or a **short-lived `&self`-scoped borrow** (within a single
synchronous poll) — never `Arc<V>`, never the raw context handle. Both satisfy
item 16: an owned clone is a *detached copy* that does not pin its cache entry
(so eviction stays free), and a borrow cannot outlive its scope. Owned results
are in fact the eviction-friendliest case, which is why the spawn path (below)
returns them.

The cost this accepts: a task that owns `Arc<Global>` cannot hold a cache
*borrow* across an `.await` (self-reference), so across awaits/tasks it works
with owned extractions. "Borrows during runs" therefore means *within a poll*;
the zero-clone borrow optimization does not cross await points. That is a fair
trade for real parallelism, and matches how the code already behaves.

## Decision 2 — evict by compaction between waves, under `&mut self`

`Arc` was only ever needed for *mid-run, in-place* eviction (dropping an entry
while borrows into it may be live). We give that up, because the borrow checker
already hands us a free, statically-safe eviction window: **`&mut self` cannot
be held while any `&self` borrow is outstanding.** So at the run boundary it is
*proven* there are no live references into the tables — the safety condition
for dropping entries.

Eviction is therefore **whole-table compaction at `&mut self`**, not per-entry
removal:

- `parse` (`async-lazy::Cache`): rebuild a shrunken cache from the retained
  entries — literally what its own docs prescribe ("to shrink, replace by a
  shrunken version"). This needs a small **`Cache::retain(&mut self, keep)`**
  (equivalently `compact`) added to the in-repo `async-lazy` crate; `&mut self`
  guarantees nothing is mid-init, so rebuilding the `AppendOnlyVec` + lookup
  map is sound. (Extending our own crate — not a new dependency.)
- `resolve`/`mono`/`spans` (`DashMap`): `retain(|k, v| keep(v))`.

Compaction runs `compact(&mut self)` between waves; see Decision 3 for *when*.

Because `Global` is `Arc`-shared (for the spawn path, Decision 5), `compact`
cannot simply take `&mut Global`. It does not need to: `run(&mut self)` **joins
every spawned task before returning** (it awaits all handles), so between waves
the `Compiler` is the sole owner and mutable access comes via `Arc::get_mut`
(or, for the parse `Cache` specifically, an interior-mutable pointer swap). The
DashMap tables need no `&mut` at all — `DashMap::retain` is `&self`. So only the
append-only parse cache's shrink-rebuild needs the `get_mut`/swap; everything
else compacts through interior mutability without disturbing the hot path's
lock-freedom.

Because the content store is content-addressed with a disk tier, **eviction is
always correctness-safe**: an evicted memory entry re-loads from LMDB
(memory → disk → compute fallthrough already exists) or recomputes to an equal
answer. Eviction only ever affects *warmth*, never correctness.

## Decision 3 — admission control: enqueue user queries, don't *start* over budget

The `&mut self` compaction window is created by backpressure. User queries are
**accepted onto a queue immediately** but a queued query is **not started while
the cache is over its byte budget**. The scheduler loop:

1. Dequeue the next user query.
2. If cache bytes > budget → `compact(&mut self)` first. This is safe now: no
   new wave has started, and the previous wave's borrows have all dropped
   (that is what "between waves" means), so `&mut self` is available.
3. Start the query as a wave, borrowing `&self` throughout.
4. Repeat.

Gating *start* (not *accept*) is what yields the quiescent `&mut self` moment
without a separate stop-the-world barrier: we simply stop opening new waves,
the current one drains, we compact, we resume. Clients see enqueue-and-wait
under memory pressure, never a dropped request.

## Decision 4 — single-flight for every kind = `Pending` + dedup

Route `resolve`/`mono` through the same claim/await primitive `parse` already
uses, keyed on **content key**. Then:

- **`Pending` is `ALazy::is_initializing`** — no `Pending(owner, wakers)` state
  hand-built in `BindingRecord`. In-flight tracking belongs in the *content
  store* (keyed by content key), not the binding layer (keyed by logical
  `StepId`): two different logical positions can resolve to the *same* content
  key and should share the one computation. Content-key single-flight gives
  strictly better dedup than a per-`StepId` state could.
- **Input dedup is free**: `get(key, init)` *is* request coalescing — identical
  concurrent demands await one computation; the mono worklist's "push a needed
  instance" becomes "demand it, await if already in flight." This is the "don't
  enqueue identical queries" ask, at content-key granularity.
- Cost: `resolve`/`mono` become **async** (they are a sync worklist today) so a
  demander can `await` an in-flight peer instead of recomputing.

## Decision 5 — the concurrency model

Parallelism is **`tokio::spawn` with each task owning an `Arc<Global>`** —
already the design (`context.rs:484` spawns import tasks; each does
`this.clone()`, so the future is `'static` without borrowing any stack frame,
and returns an **owned** `ResolveOutcome`). This is real multi-core parallelism
and it does *not* reintroduce the rejected `Arc`: `Arc<Global>` is the
whole-store handle (Decision 1), and owned results are detached copies that
don't pin cache entries.

There is **no sound scoped async spawn** to reach for instead, and that is not
an oversight: `std::thread::scope` is safe because its guard *blocks in `Drop`*
to guarantee join even on unwind; an async scope guard's `Drop` cannot force its
child futures to run to completion (a future can be dropped or `mem::forget`-ten
mid-flight), so borrowed children could outlive the parent — UB. Crates
(`async_scoped`, `moro`) wrap this in `unsafe` or block the executor. The
`Arc<Global>` + owned-results idiom is the sound answer and needs none of them.

`FuturesUnordered` / `join_all` remain fine for **IO-bound concurrency that
stays on one task** (e.g. fanning out leaf reads without a task per file), where
borrowing `&self` is convenient; they are complementary to spawn, not a
replacement.

Runs stay **serialized by `Compiler::run(&mut self)`**, which forks concurrent
work and *joins all of it before returning* — that join is what re-establishes
the sole-owner `&mut` window for compaction (Decision 2). Continuously
overlapping, unbarriered queries are off the table: without the join there is no
point at which compaction is safe, and no bound on in-flight memory.

## Decision 6 — eviction policy

We bound **memory bytes, not entry count**, so the budget is a byte budget and
per-entry **size** is the denominator, not just one signal.

- **Recency (last use) is the primary signal.** Especially apt here:
  content-addressing gives an edited file a brand-new key, so a *superseded*
  version's entries simply stop being touched and age out on their own —
  recency captures "stale version" for free.
- **Kind enters as weight, not as separate budgets.** One global byte budget
  (separate per-kind caps fragment the very thing we bound and need hand-tuning).
  Kind differentiates via (a) real per-entry size and (b) a recompute-cost
  multiplier: **`mono` stickiest** (expensive typecheck), `resolve` mid, `parse`
  cheap, **`spans` first-out** (memory-only, never on disk, trivially rebuilt
  from bytes, feeds no downstream key — the most disposable kind).
- **Deferred signals:** *frequency* (protects hot shared deps like a stdlib file
  every compile touches) is real but pollution-prone — a TinyLFU-style decaying
  sketch is the **v2** upgrade if the benches show hot-dep thrash. *Compute cost*
  matters mostly **disk-off**; with disk on, a miss is a cheap uniform LMDB read
  that collapses the cost differences, so cost stays a coarse per-kind weight.

The v1 target is **size-aware LRU on a global byte budget with a per-kind cost
weight** — the GDSF value density `(reaccess-likelihood × cost) / size`
approximated by recency.

## Decision 7 — barrier-free recency metadata (races are fine)

Recency/size metadata is stamped during the run under `&self` (concurrent
tasks). It is stamped **without a memory barrier**: correctness never depends on
it, so a lost or torn update is benign — at worst it mis-times a single eviction
(keep one entry we'd have dropped, or drop one we'd have kept), and a dropped
entry just re-loads from disk or recomputes. Performance wins.

Rust makes literal non-atomic data races UB, so we honor the *intent* — no
fence, one instruction, tolerate stale reads — with **`AtomicU64` +
`Ordering::Relaxed`**, not raw unsynchronized writes. Relaxed is barrier-free
and gives exactly the "racy but I don't care" semantics without UB. The global
clock that stamps `last_used` is likewise a `Relaxed` counter (monotonic-ish is
plenty; exact ordering is irrelevant to an approximate LRU).

**Where the metadata lives** (open implementation detail, leaning noted): a
side `DashMap<ContentKey, EntryMeta>` in `ContentStore` (`EntryMeta = {
last_used: AtomicU64, size: u32, kind }`), stamped `Relaxed` on every hit and
read only by `compact`. A uniform side table keeps `async-lazy`'s value type
clean (no per-entry metadata baked into `Cache`) and puts all policy state in
one place; the cost is one extra `Relaxed`-stamped lookup per hit. Alternative:
inline the tick in the `DashMap` value types and a small metadata slot in
`async-lazy` — faster stamp, more intrusive. Decide when building Phase A.

---

## What this supersedes

- The `BindingRecord.dirty` note in `src/store.rs` that `Pending(owner,
  wakers)` is "deliberately not represented" — it becomes represented, but as
  `ALazy::is_initializing` in the content store (Decision 4), not in the binding
  layer. Update that note rather than the layer.
- The TODO "`Pending` node state" bullet (concurrent query waves now exist).
- Roadmap item 13 (eviction) and item 16 (scope leak) fold into this doc.

## Phased plan

- **Phase A — primitive + compaction. ✅ done (2026-07-22).**
  `async-lazy::Cache::retain(&mut self, keep)` rebuilds the shrunken cache
  (append-only ⇒ replace-by-shrunken); `ContentStore` grew an `EntryMeta` side
  table (`{ last_used: AtomicU64, size, kind }`) stamped `Relaxed` on every hit,
  a `Relaxed` logical clock, and `compact(budget_bytes)` doing size-aware LRU
  with a per-kind cost weight (`keep_priority` = GDSF value density; parse cache
  shrinks via `retain`, the DashMaps via `retain`). Proven in isolation
  (`store::tests` + `async-lazy` `cache::tests`): eviction holds the byte
  budget, an evicted entry re-loads from disk, compaction drops the cold set,
  `spans` evicts first. The metadata-location question of Decision 7 was
  settled the leaning way (uniform side `DashMap`).
- **Phase B — admission control.** The scheduler queue + "don't start over
  budget → compact first" loop (Decision 3). Test: memory-pressured run enqueues
  and compacts instead of growing unbounded; clients block, never drop.
- **Phase C — single-flight for derived kinds.** Route `resolve`/`mono` through
  the `async-lazy` `Cache` primitive; make them async; keep the existing
  `tokio::spawn` + `Arc<Global>` + owned-results parallelism (Decision 5). Test:
  concurrent duplicate demands for one content key compute once (coalesced);
  panic in a claim reverts it (already `async-lazy`'s behavior) and leaves no
  poisoned `Pending`.
- **Phase D — policy tuning.** Per-kind cost weights, then TinyLFU admission if
  benches show hot-dep thrash. Wire the byte budget into daemon config.

## Invariants preserved

- Eviction never changes an answer — content-addressed + disk fallthrough make
  a memory miss a re-load or an equal recompute (warmth only).
- Append-only / first-write-wins is unaffected: a recomputed evicted entry is
  equal by determinism, so re-insertion upholds it.
- Scope safety (item 16): no `Arc<V>` of an entry ever escapes — consumers hold
  owned clones (detached copies that don't pin entries) or short-lived borrows,
  and only the whole-store `Arc<Global>` is shared. Compaction is reachable only
  after `run` has joined every spawned task, so nothing holds live cache state
  when the hot set is dropped.
</content>
</invoke>
