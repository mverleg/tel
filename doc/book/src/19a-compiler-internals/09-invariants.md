# Invariants

<!-- TODO: review -->

The whole design rests on ten rules. Each one is cheap to hold and expensive to
discover you have broken, because breaking most of them produces *stale output
with no error anywhere*. They are collected here so an implementation can be
audited against them.

1. **Everything hashed into a content key or a result fingerprint is
   deterministic.** Enforced by the `StableHash` trait
   ([Deterministic hashing](06-deterministic-hashing.md)). Interner indices, map
   iteration order, and spans in fast mode must never leak in.

2. **Result fingerprints are never storage or lookup keys.** Answers are stored
   only *under* content keys. The tempting future violation is deduplicating
   identical large answers by their fingerprint — that re-globalises the
   collision domain and breaks the 64-bit budget ([Hashing](05-hashing.md)). If
   answer dedup is ever wanted, give the blob store its own 128-bit digest.

3. **A dependent's key preimage pins dependency identity** — kind, arguments and
   position, with length prefixes — so fingerprints of *different* queries can
   never collide into the same slot.

4. **Memo updates are atomic per query**, key and fingerprint together, gated by
   the `Pending`/`Verified` node states. Otherwise a dependent can build its key
   from a stale fingerprint mid-run.

5. **The persistent cache is append-only**, plus garbage collection. Any urge to
   "invalidate" a persistent entry means a determinism bug is being papered
   over; fix the input capture instead.

6. **Only terminal deterministic answers are persisted.** A deterministic error
   *is* a terminal answer; a panic, a cancellation, or a transient failure is
   not, and nothing derived from an aborted subtree may reach the persistent
   cache. Entries are written atomically (temp plus rename, or a transactional
   store) and checksum-verified on read.

7. **Abort reverts `Pending`** — an extension of invariant 4. On cancellation or
   panic the claim returns to not-started, waiters are woken with "aborted", and
   abort-ness is sticky within the run.

8. **Dirty bits are cleared only atomically with a successful verification or
   recompute** — never at scheduling time. This is the one rule whose violation
   yields stale results with no hash collision involved.

9. **Each compile wave runs on a pinned input generation.** The first read of a
   leaf in a wave fixes its digest for the whole wave; file events arriving
   mid-wave queue for the next generation, optionally cancelling the current
   one.

10. **File-change events are hints, never truth.** Every trust boundary —
    startup, watcher overflow — re-derives dirtiness by comparing current leaf
    digests against recorded ones.

## Invariant 8, restated

Invariant 8 is the one worth internalising, because it is the difference between
"an interrupted compile is a non-event" and "an interrupted compile silently
corrupts every later build". Stated positively:

> A query is marked clean **only** as the atomic final step of a *successful*
> recompute or verification, and never while any of its inputs is dirty,
> unknown, or errored.

Hold this and any failure can only ever leave a query **dirty**, never falsely
clean. Dirty is always safe — it just means "re-check me". Falsely clean is the
dangerous state: it is what makes a later leaf-driven walk skip a stale node and
never reach the root, so the root is served from a key describing inputs that no
longer exist.

### How it is guaranteed: front-load the invalidation

Split the work into two passes with different failure characteristics:

1. **Mark — infallible.** Walk the reverse edges from the changed leaf and mark
   the *entire* affected cone dirty. Pure flag-setting: no user computation, no
   IO, nothing that can raise an error and almost nothing that can panic.
2. **Clean — fallible.** Recompute or verify dirty queries, clearing the dirty
   bit *only* on successful commit, with early cutoff on an unchanged key.

An error or panic mid-clean leaves every not-yet-cleaned node **still dirty**,
because pass 1 already marked the whole cone and pass 2 only ever *removes*
dirt. The next demand resumes on whatever is still dirty; no root-down walk is
needed to repair anything.

This is the clean form of "invalidate the remainder on error": by doing all
invalidation up front, unconditionally, there is **no fragile error path to get
wrong**. Fuse the two passes into one upward walk and you genuinely must
invalidate the remaining cone on every failure path — miss one and the nodes
above the failure stay falsely clean.

Two properties fall out:

- **Still incremental.** Pass 1 touches only reverse *edges* of the cone, which
  is cheap even when a widely-imported leaf changes; pass 2 is still cut short
  by unchanged keys. Nothing expensive runs over the whole tree and nothing
  walks down from the root.
- **Dirtiness is monotonic.** A second leaf change unions its cone into whatever
  is still dirty, so leftover dirt from a failed pass persists safely until
  something recomputes it.

### Panics specifically

Errors are values and cause no trouble — they are answers, cached like any other
([Execution and recovery](07-execution-and-recovery.md#failure-classification)).
Panics are not values: they unwind and can interrupt mid-mutation. Two cheap
defences, both upholding invariant 8:

1. Catch unwinding at the recompute boundary and convert a panic into "this node
   is dirty", never clean.
2. Make "commit the answer and clear the dirty bit" the single atomic last step
   of a successful recompute, so a panic *before* commit is indistinguishable
   from "never ran".

With the two-pass split, that is all that is required.
