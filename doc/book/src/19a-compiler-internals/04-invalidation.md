# Invalidation

<!-- TODO: review -->

Recomputation is driven from two directions, and they have different jobs:

- **From the root (pull)** — re-derive content keys top-down; cache hits stop
  the recursion. This is *correct on its own*, with no invalidation state
  whatsoever.
- **From the leafs (push)** — on a file change, walk the reverse edges and mark
  a dirty cone, so untouched subgraphs are skipped without even re-deriving
  their keys. This is *pure optimisation* and never affects correctness.

Keeping that division sharp is what makes the system debuggable: if push mode
ever produces a wrong answer, the bug is in the optimisation, and disabling it
is always a valid fallback.

## Pull: from the root

To answer the root query:

1. Recurse into dependencies to obtain their result fingerprints. At a leaf,
   digest the file bytes.
2. Form this query's content key from the kind, its stable arguments, and those
   fingerprints.
3. Look the key up in the persistent cache. **Hit** → done, record the
   fingerprint in the memo. **Miss** → execute the step, store the answer and
   its fingerprint under the key.

Early cutoff falls out automatically. If a leaf changed but some intermediate
step produced byte-identical output — a whitespace edit yielding the same AST —
that step's fingerprint is unchanged, so every key above it is unchanged, and
every lookup above it is a pure hit.

The cost: even a fully-cached run walks the entire graph doing hash-chain
lookups, because you cannot know the root's key without first reaching the
leafs. That is acceptable for a batch compile and far too slow per keystroke —
which is why push mode exists.

## Push: from the leafs

With a file watcher and the reverse edges:

1. A leaf changes → mark its logical id and, transitively via reverse edges,
   its dependents as **dirty**. Marking is bit-flipping only; do no hashing
   here. The walk **stops at nodes that are already dirty**, which makes the
   common editor case free: a file that changes again before any compile ran is
   already dirty, so re-marking is `O(1)`.
2. Queries **not** marked serve their memoized key and fingerprint with **zero
   recursion**. This is the entire payoff — untouched subgraphs cost nothing.
3. To clean a marked query: refresh its dependencies (recursively, same rules),
   rebuild its content key from their fingerprints, and compare against the
   memo.
   - **Key unchanged** → un-dirty it *without executing*, and stop
     propagating. Early cutoff again, now saving even the cache lookup for
     everything above.
   - **Key changed** → look it up in the persistent cache, execute on miss,
     update the memo. Dependents stay dirty until cleaned the same way.

This is rustc's red-green algorithm transplanted onto content-addressed keys:
green means the memoized key is still derivable, red means it changed. The
difference is that the backing store is a pure content-addressed map, so it
needs no revision counters and can be shared across machines.

### The marking invariant

Stopping the walk at already-dirty nodes is sound **only if** *dirty implies
all transitive dependents are dirty*. A marking walk that dies halfway would
break that — some nodes marked, their dependents not — and the retry would stop
early at the half-marked frontier, leaving green nodes wrongly trusted.

Two rules keep marking safe without making every panic fatal:

- **Flip post-order.** Mark a node only after all of *its* dependents are
  marked: recurse up the reverse edges first, flip on the way back, so the
  changed leaf flips last. The invariant then holds at every intermediate
  state, and an interrupted walk is resumable — re-walking from the
  still-unflipped leaf revisits exactly the unmarked remainder.
- **Consume the change event only after the walk completes.** A panic mid-walk
  leaves the event queued, and the next mutation phase retries it.

Belt and braces: any unexpected panic during marking may simply discard the
entire session memo — never the persistent cache. The next compile does one
full pull from the root, which is always correct.

### Marking never cleans eagerly

An eager variant is tempting: recompute upward from the changed leaf, stopping
where fingerprints stop changing. It is worse on both counts.

It degenerates into pulls anyway — with dynamic dependencies, recomputing a
node needs the *current* fingerprints of all its dependencies, which is a pull
rooted at that node. And it executes **zombies**: the old edges are its only
guide, so it faithfully re-runs queries the new code no longer reaches. Delete
a function and "type of `f`" re-runs against a definition that no longer
exists, producing an error that must then be distinguished from a live
diagnostic.

Lazy cleaning executes only `dirty cone ∩ live cone` — what any correct scheme
must execute — while marking stays bit flips. Over-marking of dead subgraphs is
inherent (yesterday's edges are all you have) but nearly free: marked nodes
nobody pulls are never executed, they just sit dirty until GC. The eager
*behaviour* remains available as pure scheduling policy: mark, then immediately
re-pull the root.

### Why yesterday's edges are enough

Reverse edges from the *last* execution are sound for marking, because a node
can only acquire a new dependency by executing differently — and by
determinism, executing differently requires one of its old dependencies'
fingerprints to have changed, which is a path the old edges already cover.
Marking is therefore conservative: it may mark too much, never too little.

## Dependencies come from recorded edges

A worry that comes up when authoring a query: if a query discovers its
dependencies *by executing* — calling `ctx.parse`, `ctx.resolve` inline — how
can the engine ever *skip* that execution? Building the content key needs the
dependencies' fingerprints, and "the dependencies" seem knowable only by
running the very body you hoped to skip.

The tension is false. A query is authored as **one flat body that pulls
dependencies inline**, and each pull records a forward edge. That remembered
edge set — not a re-run of the body — is what the next run uses:

- **Cold** (no memo record): there is nothing to skip. Run the body, discover
  dependencies, record edges and fingerprints. You pay for the body, but only
  when there was no cached answer to serve anyway.
- **Warm and clean** (push mode): served from the memo with zero recursion. The
  recorded key and fingerprint are returned without touching dependencies at
  all. Merely *reading* a clean node never triggers re-derivation.
- **Warm and dirty**: rebuild the content key from the **recorded** forward
  edges' current fingerprints, refreshing each recursively. The body runs only
  if that key changed.

The recorded edge set is **self-correcting**. Whatever determines the
dependency set is itself a recorded dependency — so if the true dependencies
changed, some recorded fingerprint changed, the key mismatches, and the
re-executed body re-discovers the correct set.

So "you'd have paid for the body just to find out you didn't need it" is true
*only* on the cold path, where there was no hit to miss.

## Worked example

Dependency chain: `compile main → type of f → resolve util.tel → parse util.tel
→ util.tel bytes`.

**Comment-only edit to `util.tel`.** The watcher marks the leaf's cone dirty.
Parse's content key (a byte digest) changed, so parse re-runs — but in fast mode
the AST carries no spans, so the new AST is identical, so parse's fingerprint is
unchanged. Resolve's content key is therefore unchanged; resolve is un-dirtied
*without executing* and propagation stops. Total cost: one parse.

**Edit to `f`'s body, signature unchanged.** Parse and resolve re-run with new
outputs. "Type of `f`" re-runs but produces the same type, so the same
fingerprint, so `compile main`'s key is unchanged — cutoff. Type-checking of
other items that only used `f`'s signature either never got dirty, or cleans
through unchanged keys.

**A colleague pulls the same source.** Identical bytes give identical leaf
digests, which give identical content keys all the way up. Their compile is
pure cache hits from the shared store, with no invalidation protocol between
the two machines at all.
