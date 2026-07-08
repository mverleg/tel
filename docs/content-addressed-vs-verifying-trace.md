# Content-Addressed Keys vs. Verifying Traces

Design rationale for why the query compiler stores results under **content keys** (a
Merkle DAG over answers) rather than under **logical keys with recorded dependency traces**
(the Salsa / rustc red-green style, "verifying traces" in *Build Systems à la Carte* terms).

Companion to [keys-and-invalidation.md](keys-and-invalidation.md) (the three identifiers, the
two cache layers, the invariants) and [execution-and-recovery.md](execution-and-recovery.md)
(concurrency, cancellation, recovery). That first doc records the choice in one line under
"Rejected alternative"; this doc is the long-form justification, including the performance
analysis across edit sizes and the concurrency argument that ultimately decides it.

## The two designs in one paragraph each

**Verifying trace (Salsa / rustc).** The store is keyed by *logical id* (`(kind, args)`).
Each record holds the last answer plus the list of direct dependencies it consulted, each
with the fingerprint it had at the time. To answer a query: if not dirty, return the record;
if dirty, walk the recorded deps, refresh each, and if every dep's fingerprint still matches
the recorded one, the answer is still valid (return it, no execution); otherwise re-execute,
which produces a new answer *and* a new recorded dep list. The record **is** the source of
truth for validity — there is no independent way to reconstruct "is this answer current"
without the trace.

**Content-addressed (this compiler).** The store is keyed by *content key*
= `hash(kind, stable args, direct deps' result fingerprints)` — a Merkle DAG over answers.
A key is valid forever by construction: if any input changed, the key is different, so the
store is append-only and never invalidated (keys-and-invalidation.md invariant 5). Validity
is a pure function of inputs, reconstructable from a root pull at any time. The store is the
source of truth; any per-process bookkeeping is a discardable optimization.

## The shared piece: both have a logical → latest-key link

It is tempting to think content-addressing means "no logical-keyed table." It does not. Both
designs keep a **session memo** mapping logical id → last content key, last result
fingerprint, forward + reverse dep edges, and a dirty bit
(keys-and-invalidation.md "Where Each Piece Lives"). In this codebase that memo is
`BindingLayer` (`DashMap<StepId, BindingRecord>`, with `BindingRecord` holding
`content_key`, `fingerprint`, `input_digest`, `dirty`).

The difference is not *whether* this link exists but *what role it plays*:

* **Content-addressed:** the memo is a **discardable per-process cache of a derivation.**
  Delete it and every content key can be re-derived by pulling from the root. Correctness
  lives in the content store; the memo only lets you skip the walk (this is the `TrustClean`
  fast path).
* **Verifying trace:** the memo **is the core data structure.** It cannot be discarded without
  losing the ability to decide validity, which is why it must be persisted transactionally and
  cannot be shared across processes.

### How the memo's fields are used, by dirty state

| Node state | What is read | Work |
|---|---|---|
| **Clean** (dirty bit clear / `Verified`) | the memoized `fingerprint` — it *is* the current answer's fingerprint, which is exactly what dependents fold into their keys | zero recursion, no store lookup (the `content_key` is only consulted if a dependent needs the actual *value*) |
| **Dirty** | re-derive this node's content key from its deps' current fingerprints (over the **recorded forward edges**), compare to the memoized `content_key` | key **same** → clear dirty, keep fingerprint, don't execute, stop propagating (early cutoff); key **changed** → store lookup (hit = these exact inputs seen before, possibly by another run/machine; miss = execute), update memo |

The dirty path is the only place the *store* participates, and it is where content-addressing
quietly out-earns verifying traces: a changed key can **hit** on work done by a previous run
or a concurrent process, whereas a verifying trace can only ever recompute.

## Performance by edit size

The key correctness fact underpinning the comparison is the **marking invariant**
(keys-and-invalidation.md:181): a node can only acquire a *new* dependency by executing
differently, which requires one of its *old* deps' fingerprints to have changed. Therefore
both designs clean a dirty node by walking its **recorded forward edges** — content-addressed
does *not* re-discover deps on the clean path; it reuses the stored edge set, and a stale edge
set is caught automatically (the thing that determines the dep set is itself a recorded dep, so
if the true deps changed, some recorded fingerprint changed, the key mismatches, and the node
falls through to re-execution which re-discovers correctly).

| Edit regime | Verifying trace | Content-addressed | Verdict |
|---|---|---|---|
| **Completely cold** (empty memo) | full pull, re-execute everything, deserialize answers up the graph | same | **equal** |
| **Completely hot / tiny change** | dirty bits serve the memo; clean cone untouched | same (`TrustClean`) | **equal** |
| **Half-hot** (large rebase, big dirty cone) | walk the dirty cone over recorded edges; per node, compare each dep's fingerprint to the recorded one | walk the *same* cone over the *same* recorded edges; per node, hash the deps' current fingerprints into a key and compare to the memoized key | **equal walk** (see below) |

The intuition that half-hot favors verifying traces is the interesting case, and it is
**mostly not correct.** The clean walk is the same shape and touches the same deps in both;
the only arithmetic difference is hashing a handful of fingerprints into a key vs. comparing
them individually — noise. Neither design materializes upstream *answers* on the clean path;
both run on recorded edges plus memoized fingerprints. Where a dep genuinely changed and the
node must re-execute, both re-run and re-discover deps identically.

The two designs diverge only in second-order ways, and **neither clearly favors the trace:**

1. **Content-addressed adds one store lookup at each re-execute frontier node.** On a rebase
   this is opportunistically a *win*: if the rebased state reproduces intermediate results seen
   before (a branch previously built, a shared CI cache), the lookup hits and skips execution
   entirely. Worst case — all-novel content — it is a small constant loss (one failed lookup
   per re-exec; the deps were going to be refreshed for the execution anyway, so nothing else
   is wasted).
2. **Verifying traces keep answers hot in RAM; content-addressed may fetch + deserialize the
   answer from the store** when a dependent needs the *value* rather than just the fingerprint.
   This is the one genuine single-process edge for the trace — but it is orthogonal to the walk
   and is an *implementation choice*, not a property of content-addressing. An in-memory answer
   cache (LRU keyed by content key, with the store as the cold tier) closes it; warm answers
   are then equal.

### The caveat that makes the "traces win half-hot" intuition true

If you build the *pure-constructive* variant that does **not** store forward edges and instead
re-discovers deps by re-reading upstream *answers* on every dirty node (deserializing an AST
just to read a file's import list, even when only trying to clean), then content-addressed
pays answer-materialization across the whole dirty cone and verifying traces win half-hot
decisively. The fix is not to switch designs; it is to **store the forward edges in the memo**
(which keys-and-invalidation.md already specifies) so the clean walk runs on edges +
fingerprints, exactly like a trace. That single choice collapses the half-hot gap.

## Concurrency and inter-process safety — the deciding axis

Within a single process the two are equivalent: same per-query single-flight over the memo
states (`Unknown / Dirty / Pending / Verified`), same mutation/query phase split
(execution-and-recovery.md), which is itself Salsa's revision discipline. The divergence is at
persistence and any *second* process.

**Content store:**

* *Within a process*: lost races are benign — racing tasks produce byte-identical keys and
  results, and content-addressed writes are idempotent (execution-and-recovery.md:20). Locking
  affects throughput, never correctness.
* *Across processes* (a daemon plus an ad-hoc `telsb` batch run, or two daemons sharing one
  store): **zero protocol.** Racing writers on the same key write identical bytes, so no
  locking; an entry is a self-contained fact, so a concurrent writer cannot poison a reader; a
  cancelled or crashed wave leaves the store strictly richer (free checkpointing,
  execution-and-recovery.md:104). Interruption is safe by construction — per-entry atomic
  visibility (temp + rename) plus a checksum caught on read is the *entire* discipline.

**Logical-keyed trace store:**

* *Within a process*: equivalent.
* *Persistence + any second process*: records are mutated in place and are meaningful only
  relative to one revision history. Two processes sharing the store race **destructively** — a
  record whose dep list came from run A but whose answer came from run B is a correctness
  hazard that is not self-evidently wrong (contrast: a torn content-store entry fails its
  checksum and is treated as a miss). This forces single-writer discipline: rustc locks its
  incremental directory per compilation; Cargo simply forbids sharing one. A crash mid-update
  needs transactional whole-record writes to avoid the torn-record hazard, where the content
  store needed only temp + rename per entry.

So the multi-machine property is **inseparable from the multi-process safety** you want on a
single machine anyway (daemon + terminal batch compile that feed each other, rather than block
each other or run cache-less).

## Decision

Stay content-addressed, and buy single-machine speed with a **persisted session memo** as a
per-process, discardable sidecar — this is the cheap, invariant-free part of Salsa added *on
top of* the store rather than swapped in. On startup: load the memo, `reconcile()` leaf
digests (execution-and-recovery.md), mark cones for changed leafs, serve the rest at memo-hit
speed. Because correctness never depends on it (stale/corrupt/absent ⇒ discard, cold pull —
the existing belt-and-braces), it needs no new invariants and no migration care. Optionally add
an in-memory answer LRU keyed by content key to match a trace's warm-answer residency.

This is not a free or strictly-simpler choice, and the honest costs should be named:

* **Two structures instead of one** (memo *and* content store) — strictly more machinery than a
  single trace store. This is the real "reason not to."
* **Determinism is a permanent tax:** portable keys require `StableHash` on everything, no
  interner indices leaking in, deterministic `Err` types, `SCHEMA_VERSION` bumps on any format
  change. A trace's in-process fingerprints can be sloppier (compared only within one process's
  history).
* **Append-only storage + GC:** the store accumulates superseded garbage and needs a
  reclamation policy; a trace mutates in place, bounded by live query count.
* **The answer LRU is extra work,** the piece added to match a trace's warm-answer residency.

The choice is nonetheless clear because those costs are **already sunk** — `StableHasher`,
`SCHEMA_VERSION`, `StableCtx`, [deterministic-hashing.md](deterministic-hashing.md), and the
content store all exist. The marginal cost of staying is small; the marginal cost of switching
is discarding that work *and still* solving inter-process safety separately. And it is not
either/or: content store + persisted memo delivers **both** a trace's clean-skip speed (from
the memo) and lock-free cross-process / cross-run sharing (from the store); a verifying trace
delivers only the first.

The only scenario where the trade flips: a project that values minimal moving parts over
inter-process safety and cross-run hits, for which a verifying trace is legitimately the
simpler system. Given this compiler's design (daemon + ad-hoc `telsb`, a shared store, portable
keys already built in), that scenario does not apply.
