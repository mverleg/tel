# Why Content-Addressed

<!-- TODO: review -->

The design choice that shapes everything else is storing answers under **content
keys** — a Merkle DAG over answers — rather than under **logical keys with
recorded dependency traces**, which is what Salsa and rustc do ("verifying
traces" in the *Build Systems à la Carte* taxonomy). This topic is the long-form
justification, including the performance comparison across edit sizes and the
concurrency argument that ultimately decides it.

## The two designs

**Verifying trace.** The store is keyed by *logical id*. Each record holds the
last answer plus the list of direct dependencies it consulted, each with the
fingerprint it had at the time. To answer a query: if not dirty, return the
record; if dirty, walk the recorded dependencies, refresh each, and if every
fingerprint still matches the recorded one, the answer is still valid — return
it without executing. Otherwise re-execute, producing a new answer *and* a new
recorded dependency list. **The record is the source of truth for validity**;
there is no independent way to reconstruct "is this answer current" without it.

**Content-addressed.** The store is keyed by *content key* = `hash(kind, stable
args, direct dependencies' result fingerprints)`. A key is valid forever by
construction: if any input changed, the key is different. So the store is
append-only and never invalidated. **Validity is a pure function of inputs**,
reconstructable from a root pull at any time. The store is the source of truth;
any per-process bookkeeping is a discardable optimisation.

## Both designs keep a logical-keyed memo

It is tempting to think content-addressing means "no logical-keyed table". It
does not. Both designs keep a session memo mapping logical id to last content
key, last result fingerprint, both directions of dependency edge, and a dirty
bit.

The difference is not *whether* that link exists, but what role it plays:

- **Content-addressed** — the memo is a **discardable per-process cache of a
  derivation**. Delete it and every content key can be re-derived by pulling
  from the root. Correctness lives in the content store; the memo only lets you
  skip the walk.
- **Verifying trace** — the memo **is** the core data structure. It cannot be
  discarded without losing the ability to decide validity, which is why it must
  be persisted transactionally and cannot be shared across processes.

How the memo's fields are used, by node state:

| Node state | What is read | Work |
|---|---|---|
| **Clean** | the memoized fingerprint — it *is* the current answer's fingerprint, which is exactly what dependents fold into their keys | zero recursion, no store lookup; the content key is consulted only if a dependent needs the actual *value* |
| **Dirty** | re-derive this node's key from its dependencies' current fingerprints over the **recorded forward edges**, compare to the memoized key | key **same** → clear dirty, keep fingerprint, do not execute, stop propagating; key **changed** → store lookup (hit means these exact inputs were seen before, possibly by another run or machine; miss means execute), then update the memo |

The dirty path is the only place the store participates, and it is where
content-addressing quietly out-earns a trace: a changed key can **hit** on work
done by a previous run or a concurrent process, whereas a verifying trace can
only ever recompute.

## Performance by edit size

The fact underpinning the comparison is the
[marking invariant](04-invalidation.md#why-yesterdays-edges-are-enough): a node
can only acquire a *new* dependency by executing differently, which requires one
of its *old* dependencies' fingerprints to have changed. So both designs clean a
dirty node by walking its **recorded forward edges**. Content-addressing does
*not* re-discover dependencies on the clean path — it reuses the stored edge
set, and a stale edge set is caught automatically.

| Edit regime | Verifying trace | Content-addressed | Verdict |
|---|---|---|---|
| **Cold** (empty memo) | full pull, execute everything, deserialize answers up the graph | same | equal |
| **Hot** (tiny change) | dirty bits serve the memo; the clean cone is untouched | same | equal |
| **Half-hot** (large rebase, big dirty cone) | walk the dirty cone over recorded edges; per node compare each dependency's fingerprint to the recorded one | walk the *same* cone over the *same* edges; per node hash the current fingerprints into a key and compare to the memoized key | equal walk |

The intuition that half-hot favours verifying traces is the interesting case,
and it is **mostly not correct**. The clean walk has the same shape and touches
the same dependencies in both; the only arithmetic difference is hashing a
handful of fingerprints into a key versus comparing them individually — noise.
Neither design materialises upstream *answers* on the clean path. Where a
dependency genuinely changed and the node must execute, both re-run and
re-discover dependencies identically.

They diverge only in second-order ways, and neither clearly favours the trace:

1. **Content-addressing adds one store lookup per re-execute frontier node.** On
   a rebase this is opportunistically a *win*: if the rebased state reproduces
   an intermediate result seen before — a branch previously built, a shared CI
   cache — the lookup hits and execution is skipped entirely. Worst case, with
   all-novel content, it is a small constant loss of one failed lookup, and the
   dependencies were going to be refreshed for the execution anyway.
2. **Verifying traces keep answers hot in RAM**, whereas content-addressing may
   fetch and deserialize the answer from the store when a dependent needs the
   *value* rather than just the fingerprint. This is the one genuine
   single-process edge for the trace — but it is orthogonal to the walk, and it
   is an implementation choice rather than a property of content-addressing. An
   in-memory answer cache keyed by content key, with the store as the cold tier,
   closes it.

### The caveat that makes the intuition true

If you build the *pure* constructive variant that does **not** store forward
edges, and instead re-discovers dependencies by re-reading upstream *answers* on
every dirty node — deserializing an AST just to read a file's import list, even
when only trying to clean — then content-addressing pays answer materialisation
across the whole dirty cone, and verifying traces win half-hot decisively.

The fix is not to switch designs; it is to **store the forward edges in the
memo**, so the clean walk runs on edges plus fingerprints exactly like a trace.
That single choice collapses the half-hot gap.

## The deciding axis: concurrency and multiple processes

Within a single process the two are equivalent: the same per-query
single-flight, the same mutation/query phase split — which is itself Salsa's
revision discipline. They diverge at persistence and at any *second* process.

**Content store.**

- *Within a process*: lost races are benign. Racing tasks produce byte-identical
  keys and results, and content-addressed writes are idempotent. Locking affects
  throughput, never correctness.
- *Across processes* — a build daemon plus an ad-hoc command-line compile, or
  two daemons sharing one store — **zero protocol**. Racing writers on the same
  key write identical bytes, so no locking. An entry is a self-contained fact,
  so a concurrent writer cannot poison a reader. A cancelled or crashed wave
  leaves the store strictly richer. The entire discipline is per-entry atomic
  visibility plus a checksum caught on read.

**Logical-keyed trace store.**

- *Within a process*: equivalent.
- *Persistence plus any second process*: records are mutated in place and are
  meaningful only relative to one revision history. Two processes sharing the
  store race **destructively** — a record whose dependency list came from run A
  but whose answer came from run B is a correctness hazard that is not
  self-evidently wrong. (Contrast a torn content-store entry, which fails its
  checksum and is simply treated as a miss.) This forces single-writer
  discipline: rustc locks its incremental directory per compilation; Cargo
  forbids sharing one at all. A crash mid-update needs transactional whole-record
  writes, where the content store needed only temp-plus-rename per entry.

So the multi-machine property is **inseparable from the multi-process safety**
you want on a single machine anyway — a daemon and a terminal compile that feed
each other rather than blocking each other or running cache-less.

## Decision, and its honest costs

Stay content-addressed, and buy single-machine speed with a **persisted session
memo** as a per-process, discardable sidecar. This is the cheap, invariant-free
part of Salsa added *on top of* the store rather than swapped in. On startup:
load the memo, reconcile leaf digests, mark cones for changed leafs, serve the
rest at memo-hit speed. Because correctness never depends on it — stale, corrupt
or absent means discard and cold-pull — it needs no new invariants and no
migration care.

This is not a free or strictly simpler choice, and the costs deserve naming:

- **Two structures instead of one** — memo *and* content store — is strictly
  more machinery than a single trace store. This is the real reason not to.
- **Determinism is a permanent tax.** Portable keys require stable hashing
  everywhere, no interner indices leaking in, deterministic error types, and a
  schema version bumped on any format change. A trace's in-process fingerprints
  can be sloppier, since they are only ever compared within one process's
  history.
- **Append-only storage needs GC.** The store accumulates superseded garbage and
  needs a reclamation policy; a trace mutates in place, bounded by live query
  count.
- **The answer cache is extra work** — the piece added to match a trace's
  warm-answer residency.

The choice is nonetheless clear, because it is not either/or: content store plus
persisted memo delivers **both** a trace's clean-skip speed (from the memo) and
lock-free cross-process, cross-machine sharing (from the store). A verifying
trace delivers only the first.

The one scenario where the trade flips: a project that values minimal moving
parts over inter-process safety and cross-run hits. For that project a verifying
trace is legitimately the simpler system. Given that Tel expects a compile
daemon alongside ad-hoc command-line compiles, and portable keys throughout,
that scenario does not apply here.
