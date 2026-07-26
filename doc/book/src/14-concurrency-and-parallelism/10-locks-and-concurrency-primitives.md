# Locks and Concurrency Primitives

<!-- TODO: review -->

[Per-task heap isolation](07-memory-model-for-concurrency.md) makes the common
"two tasks racing on a plain field" impossible by construction. But shared
mutable state is sometimes the right tool — a hit counter incremented from
many tasks, a configuration that *can* be updated while readers run, a worker
pool of N handles. For those cases the standard library exposes a small,
fixed set of **concurrency primitives**: shared types that are safe to read or
update from any task at once.

This topic catalogues those primitives, the rules every user-defined type
inherits from them, and the trade-offs that drove the design.

## What the stdlib provides

Tel's standard library is the **only** legitimate source of types that are
safe to share mutably between tasks ([Sync](09-scoped-values.md), in the
terminology of the previous topic). A user-defined mutable struct is never
Sync; if a script needs to share mutation, it reaches for one of:

| Primitive | What it is | When to reach for it |
|---|---|---|
| **Channel** | Closeable queue — see [channels](06-channels-and-message-passing.md) | Hand-off, fan-out, worker queues, resource pools |
| **Mutex[T]** | Exclusive lock guarding a `T` | A small protected region around a non-trivial update |
| **AtomicInt** / **AtomicFlag** / **AtomicRef[T]** | Lock-free single-cell updates | Counters, flags, swap-the-pointer patterns |
| **RwLock[T]** | Many readers *or* one writer | Mostly-read shared configuration |
| **Once[T]** / **LazyInit[T]** | Initialize-once cell | Lazy singletons inside a script's lifetime |
| **WorkerPool** | A fixed pool of worker tasks | Bounded parallelism for CPU work |

This is roughly the candidate set (`concmap`, `queue`, `atomic`,
`lock`, `lazy_init`, "worker pool"); the table here trims to what actually
composes with the rest of the chapter and leaves room for the implementation
notes to discuss exact APIs.

The deliberate omissions are as informative as the inclusions:

- **No raw `Atomic[T]` for arbitrary `T`.** Atomics are confined to a few
  primitive-shaped cells. A general atomic over user types becomes a
  request for low-level memory ordering control, which Tel does not expose.
- **No concurrent hash map.** A shared mutable map invites contention bugs
  and ties the language to a specific implementation; the idiomatic
  alternative is to give one task ownership of a map and have others talk
  to it through a [channel](06-channels-and-message-passing.md).
- **No condition variables.** They are powerful but easy to misuse and
  needed mostly when you have already chosen the wrong primitive. Channels
  cover "wait for an event," `Once` covers "wait for an initialisation."
- **No reader/writer barriers, fences, or memory-order tunables in user
  code.** The stdlib types are sequentially consistent from the script's
  perspective; the host runtime picks the cheapest ordering that preserves
  that semantics.

TODO(open): Confirm the exact set above. `concmap` (a
concurrent map) is a candidate but Tel pushes back against it; if a clear small use case
emerges, reconsider. Reentrant mutex vs non-reentrant — see *Reentrancy*
below.

## Locks yield, they do not spin

A blocking operation on a Tel lock — `Mutex.lock()` waiting on a held
mutex, `RwLock.write()` waiting on outstanding readers — is a
[yield point](08-cancellation-and-timeouts.md). The waiting task is parked
by the scheduler; the worker thread is free to pick up other work.

This is the right default for an embedded scripting language. The
alternative — a spin lock that burns CPU until the holder releases — is
appropriate for low-level systems where every microsecond counts and you
own the scheduler, but it is the wrong default for guest scripts that
share a host's worker pool. Spin loops are simply not exposed to user
code.

Two consequences worth stating up front:

- **A lock is async-friendly.** Acquiring a lock that is held does not
  block a host thread; it parks the task. A host with one OS thread (a
  browser) handles this the same way it handles a channel `receive`.
- **Locks have backpressure built in.** A swamped lock holds queued tasks
  in their parked state instead of letting them spin; the host's
  scheduler sees what is waiting on what and schedules accordingly.

This is a firm point: *"waiting for locks should probably be
async. Not only is that probably more efficient than idling a thread, it
also prevents some deadlocks, where a lock is held across an await
point."* That last clause is the next section.

## Do not hold a lock across a yield point

The single most important rule for using locks in Tel:

> Acquire, do the small protected update, release. Do not block, do not do
> I/O, do not call into channels, do not call user-provided callbacks
> between `lock()` and the matching release.

Why:

- **Deadlocks.** A task that suspends inside a critical section can wake
  up after the rest of the system has decided it was abandoned, or it can
  block another task that holds a resource the first one needs to make
  progress. The classic case is two tasks each holding one lock and
  waiting on each other's; introducing an `await` between acquisition and
  release widens the window arbitrarily.
- **Throughput.** A long-held lock serialises every other task that wants
  it. On a fiber-capable host, parking a task that holds a lock means the
  scheduler may not even run the holder again for some time.
- **Cancellation correctness.** A task cancelled while holding a lock at
  least releases it cleanly (the heap is dropped, the lock object's
  cleanup runs), but any invariant the task was in the middle of updating
  is *not* preserved. Keep the protected region narrow enough that
  "cancelled here" cannot leave the protected value in a bad state.

The convention is that a `Mutex[T]`'s lock-holding API returns a *guard*
that releases on scope exit — so the lifetime of the lock is the lifetime
of a small block, never the whole function. TODO(open): The exact API
(`mutex.lock |t| { ... }` closure, vs `let g = mutex.lock(); ... drop(g)`
guard) is undecided. Lean: the closure form, because it makes the
critical region a visible block and is hard to accidentally extend.

A related rule: **a lock-holding region should not call back into
user-provided code** (a passed-in closure, a virtual method, a callback
the host installed). Effective Java's Item 79 ("never call an alien
method from within a synchronized region") states the rule for Java; the
reasoning carries over: alien code can call back into the same lock and
deadlock, or it can break the invariant the lock was protecting. The
narrow-critical-region convention above naturally prevents this; the rule
is worth restating for the cases where the temptation is strong.

## Reentrancy

Java's intrinsic monitors are *reentrant*: the same thread can re-acquire
a lock it already holds, so a synchronised method can call another
synchronised method on the same object. This is convenient but masks
bugs — recursive lock acquisition is usually a sign the protected region
is too big or the call graph is wrong.

Tel's default is **non-reentrant locks**. A task that already holds a
mutex and calls something that tries to acquire it again deadlocks
itself, loudly. The narrow-critical-region rule above makes this rare in
practice; when it does happen the failure points at the design problem
instead of hiding it.

TODO(open): Whether a *reentrant* variant is offered as an explicit
opt-in for the cases where it is genuinely the simplest expression
(recursive parsing into a shared cache, for example). Lean: no — push the
caller toward refactoring into a private non-locking helper. Confirm.

## Atomics

`AtomicInt`, `AtomicFlag`, and `AtomicRef[T]` cover the lock-free
single-cell cases: a counter, a one-bit flag, a swap-the-pointer pattern.
Their operations are sequentially consistent from the script's view;
the host runtime is free to compile them with weaker hardware ordering
if it can prove the semantics is preserved.

There is a question of **whether `+=` on a Sync numeric type
should compile to an atomic operation by default**. Tel's answer is **no
implicit atomicity**: `+=` on a plain `Int64` is non-atomic and not Sync;
`+=` on an `AtomicInt` is atomic and is part of the atomic API. Hiding
the atomicity in an operator would make a sharp tool invisible and tie
the same syntax to two very different cost models. The script chooses
the type; the operator does what the type says.

Concretely:

```tel
let counter = AtomicInt(0)
tasks.spawn("inc", || counter.add(1))     # ok, atomic
tasks.spawn("inc", || counter.add(1))     # ok, atomic

let uniq tally = 0                          # plain Int64, task-local
# tasks.spawn("inc", || { tally += 1 })   # rejected: tally is not Sync
```

Atomics expose **CAS** (compare-and-swap) as their core primitive — every
other atomic operation either uses CAS or is a single-store/load. CAS is
the tool for "update X based on its current value" patterns that a plain
`fetch_add` cannot express.

TODO(open): The exact set of atomic types and operations (load, store,
add, exchange, compare-and-exchange, fetch-and-update closure). Lean:
small fixed set, no per-operation memory-ordering knob.

## RW locks: when reads dominate

A `RwLock[T]` is the right primitive when reads dominate and the protected
value is non-trivial enough that `AtomicRef[T]` is awkward (you'd be
swapping whole snapshots) but cheap enough that `Mutex[T]` would serialise
unnecessarily — a configuration map that is read every request and rewritten
on reload, say.

The trade-off (Wikipedia's classic readers-writers problem)
is **writer fairness**: a naive RW lock can starve writers if readers
arrive faster than they leave. Tel's `RwLock` defaults to writer-preferred
scheduling — once a writer is waiting, no new readers acquire the lock —
which is the right call for the typical "occasional reload" case the type
is meant for.

TODO(open): Whether `RwLock` is offered at all, given that the stdlib
already has `Mutex` + `AtomicRef`. Lean yes; defer to use cases.

## Once / LazyInit

`Once[T]` and `LazyInit[T]` cover the "compute this exactly once, even
under concurrent first calls" pattern — a parsed configuration, a derived
table, a host capability that has to be wrapped before use. The first
caller does the work; concurrent callers park until it finishes; later
callers get the value back without locking.

This is a Sync wrapper around an inner value that is initially absent.
Once initialised, reads are lock-free.

## What about contention, fairness, and priority?

Harder lock topics remain — fair locks (FIFO acquisition),
locks with multiple tokens (counting semaphore), tasks with explicit
priorities so the "important" worker wins a contended lock.

Tel keeps the surface narrow:

- **Fairness.** The stdlib `Mutex` makes no fairness guarantee. A task
  may be starved by faster competitors; if FIFO matters, build it
  explicitly on a channel.
- **Counting semaphores.** Use a bounded channel pre-loaded with N
  tokens — see [channels as resource gates](06-channels-and-message-passing.md#channels-as-resource-gates).
- **Priorities.** No language-level task priorities. The host's scheduler
  may have its own; a script does not see it.

These are not gaps to fill; they are surface deliberately left to the
host or to libraries built on top of channels.

## How it looks

```tel
# A small counter shared between several worker tasks.
fn count_hits(reqs: Channel[Request], tasks: Tasks) -> Int64 {
    let hits = AtomicInt(0)
    for _ in 0..workers {
        tasks.spawn("worker", || {
            for req in reqs {
                if interesting(req) { hits.add(1) }
            }
        })
    }
    # ... join children (structured concurrency) ...
    hits.load()
}

# A short critical section around a non-trivial update.
fn record(events: Mutex[EventLog], e: Event) {
    events.lock |log| {
        log.push(e)                    # plain mutation of the inner value
    }                                  # lock released here
    # NOT: events.lock |log| { flush_to_disk(log) }  -- holds lock across I/O
}

# Initialise-once for a derived value.
let schema: LazyInit[Schema] = LazyInit(|| parse_schema(source))
let s = schema.get()                   # first caller parses; others wait
```

## Cache and lock interaction: the ForkJoin / RecursiveLoad story

A particularly nasty catalogue case worth recording because it shapes how
locks, caches and the scheduler interact:

A computation runs on a worker pool. The body needs a cached matrix; the
cache loader uses a linear-algebra library that itself schedules sub-tasks on
*another* worker pool. The submitting thread waits for a future from the
sub-pool. Because the submitter is itself a worker, it does not block — it
*steals* a task from its own pool. The stolen task happens to need the same
cache entry the original task was already computing. The cache detects the
recursive load and aborts.

Lessons baked into the Tel model:

- **A blocking-looking call on a worker may actually run other work.** A
  fiber-capable host may suspend the waiting task and let the worker pick up
  something else. A user-implemented cache that holds a per-key flag during
  load can deadlock with itself under task stealing. The stdlib answer:
  `Once[T]` / `LazyInit[T]` are *the* shared-init primitive (see above), and
  they are designed to behave correctly under concurrent first calls.
- **A cache implementation that wants to share an in-flight result across
  callers should hand back a *task handle* (or its result type), not a flag
  with a sleep loop.** That way a second caller observes the in-flight task
  via the ordinary task/result machinery rather than re-entering the
  loader.
- **A concurrent and a single-threaded implementation of the same logical
  cache should not be reachable through the same identifier.** The
  catalogue records a bug where two distinct cache implementations were
  keyed under the same name; callers could end up with the wrong one. The
  stdlib's typed primitive surface keeps these visibly separate — they are
  different *types*, not a single name with hidden behaviour.

## Bugs this prevents

A few concrete catalogue cases that drove the design:

- **"Cache implementations swapped under the same key."** Two
  implementations of the same cache (one concurrent, one synchronised) were
  registered under the same identifier; in theory a caller could end up with
  the wrong one. Tel keeps the choice visible in the *type*: a `Mutex[T]` is
  not interchangeable with an `AtomicRef[T]`, and a piece of code that
  *requires* lock-free behaviour declares it in its parameter type.
- **"`ConcurrentModificationException` because a `synchronized` was
  removed."** A merge restored old code that didn't have the surrounding
  `synchronized` block. The bug only surfaced under load. The Tel mutex
  story makes this less fragile: the lock owns the protected value (the
  closure-form `mutex.lock |t| { ... }` makes the critical region a
  visible block) so there is no separate "and remember to lock this" rule
  for the reader to apply.
- **"`add_all` to a `ConcurrentHashMap` of `null` values."** The recurring
  failure case for `null`-in-`ConcurrentHashMap`. Tel has no `null`; this
  bug class is structurally absent.

## Open questions

- TODO(open): The lock-acquisition API shape — closure-form
  `mutex.lock |t| { ... }` vs guard-form `let g = mutex.lock()`. The
  closure form makes the critical region a visible block and prevents
  accidental extension across `await`s; the guard form composes better
  with conditional acquisition. Lean: closure.
- TODO(open): Whether the language statically *rejects* an attempt to
  yield (block, send/receive on a channel, call a function marked
  `can-block`) inside a `Mutex.lock |t| { ... }` body. This would harden
  the "no lock across yield" rule into a checked one; the cost is
  another flavour of function colouring through `can-block` (see
  [function colouring](03-async-and-function-colouring.md)).
- TODO(open): Whether `Mutex[T]` actually *owns* the protected value
  (Rust-style), so the only way to reach the inner value is by holding
  the lock — vs `Mutex` as a separate object you discipline yourself
  to pair with the data. Lean: ownership form, because it makes the
  "you cannot touch this without the lock" guarantee structural.
- TODO(open): Reentrancy (see above).
- TODO(open): Fair-locks / counting-semaphore primitives — keep out
  of stdlib, or revisit. Lean: out.
- TODO(open): The exact set of atomic types and operations.
