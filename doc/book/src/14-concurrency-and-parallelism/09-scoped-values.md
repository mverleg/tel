# Shared State and Locks

Two questions decide whether a value can take part in concurrency:

- **Send** — can the value be *moved* to another thread?
- **Sync** — can the value be *shared*, reached by reference from more than one
  thread at once?

Both are properties the compiler derives automatically; neither is written by
hand in ordinary code. This topic states the rules and their rationale; the
locking mechanisms for the mutable-shared case are sketched at the end.

<!-- TODO: review -->

## Send — moving a value to another thread

**What.** A type is **Send** when a value of it can be handed to another thread
— captured by a spawned task, pushed through a channel, and so on.

**The rule.** *Every type is Send unless it transitively contains a
thread-affine host resource.* Send is structural: a composite type is Send
exactly when all of its fields are.

The things that are **not Send** are leaf forms that point into one task's
private state, together with anything that transitively holds one:

- **Borrows** — `&T` and `&!T` are scoped pointers into the
  owner's task-local heap. Sending one to another task would yield a
  cross-heap pointer the receiver cannot validly dereference (see
  [Why borrows are `not Send`](../12-memory-and-runtime/04-references-and-aliasing.md#why-borrows-are-not-send)).
- **GUI / windowing handles** — most UI toolkits are main-thread-only.
- **Thread-local host state** and **thread-pinned FFI handles**.
- A **host capability** the embedder deliberately scoped to a single thread.
- A value **bound to a thread-local arena**, if the program uses one — it
  carries an arena-relative reference that is meaningless on another thread.

Borrows are language-level `not Send` (compiler-enforced from the type
structure); the other leaves are marked `not Send` in the **host-binding / FFI
layer**. Either way, plain *owned* user data is always `Send` — the script
never opts in.

**Why mutability is irrelevant for owned values.** A mutable *owned* value in
Tel is *affine* — unaliased, with exactly one reference to it. Moving such a
value to another thread is race-free by construction: the sender loses access,
the receiver gains exclusive access, and the two never touch it at the same
time. So an owned mutable type is `Send` for the same reason an immutable one
is; it becomes `not Send` only by containing one of the leaf forms above. A
`&!T` *borrow* is a different matter — borrows are `not Send` regardless of
what they point at, because the not Send-ness comes from the scope, not from the
borrowed type. Send tracks *"contains a borrow or a thread-pinned resource"*
and nothing else.

This fits the embedding philosophy: the language core is thread-agnostic, and
thread-affinity is purely a concern the host introduces.

## Sync — sharing a value across threads

**What.** A type is **Sync** when a value of it can be *shared* — reached by
reference from several threads concurrently — without a data race.

**The rule.** A type is Sync exactly when it is one of:

- an **immutable type** — deeply immutable, so concurrent readers cannot race;
- a **standard-library concurrency type** — channels, `Mutex`, atomics and the
  like, which are built to be shared safely.

A user-defined **mutable** type is never Sync. Mutable shared state exists in
Tel only through the standard library: you reach for a `Mutex`, an atomic, or a
channel rather than sharing a plain mutable value. This keeps the sharp tool —
shared mutation — visible and deliberate, while ordinary mutable data stays
unaliased and race-free by default.

## How it looks

```tel
# Immutable data — Send and Sync. Move it or share it freely.
let config = Config{ retries: 3, name: "ingest" }
spawn(|| run_with(config))      # ok
spawn(|| run_with(config))      # ok again — config is immutable, freely shared

# A freshly built mutable buffer — Send, not Sync.
let uniq buf = Buffer.with_capacity(4096)
spawn(|| fill(buf))             # ok: buf is moved onto that thread (Send)
# spawn(|| fill(buf))           # rejected: buf is affine and not Sync

# Shared mutable state goes through the stdlib.
let counter = Atomic(0)         # stdlib concurrency type — Sync
spawn(|| counter.add(1))
spawn(|| counter.add(1))        # ok: both threads share it

# A host GUI handle — not Send.
# spawn(|| window.redraw())     # rejected: window is thread-affine
```

## Relationship to copying

Whether a shared immutable value is physically copied or shared behind a
reference count — and, if refcounted, whether that count is atomic — is an
implementation choice, not part of the language (see
[memory management](../12-memory-and-runtime/03-memory-management.md)). A value
that never escapes its origin thread can use the cheaper non-atomic form; one
that crosses a thread boundary uses the atomic form. This is an optimisation,
invisible in the type system.

## Locks and shared-state mechanisms

The concrete stdlib Sync types — `Mutex`, atomics, `RwLock`, `Once`,
`WorkerPool` — and the rules that govern their use (locks yield, never
hold across a suspension, no implicit atomicity on operators) are covered
in [locks and concurrency primitives](10-locks-and-concurrency-primitives.md).
Two omissions worth flagging here, because they shape what *kind* of Sync
types Tel does not offer:

- **No condition variables, no memory fences, no per-operation ordering
  knobs.** The Sync surface stays small. A sequentially consistent
  primitive is easy to reason about; a `seq_cst` / `release` / `acquire`
  ladder is the kind of low-level control Tel keeps out of user code (see
  [antifeatures](../02-philosophy/04-antifeatures.md)).
- **No general concurrent hash map.** A shared mutable map is too easy to
  misuse; the idiomatic pattern is to give one task ownership and others
  talk to it through a [channel](06-channels-and-message-passing.md).

## Open questions

RESOLVED: borrows are first-class in Tel
(see [References and Aliasing](../12-memory-and-runtime/04-references-and-aliasing.md)
and [TIP-0001](../tips/0001-mutability-and-borrowing.md), Accepted) but are
structurally `not Send`, so the rule still works without reconsideration: a
borrow simply cannot be captured into another task. The `not Send` leaf set now
includes borrows alongside thread-affine host resources, listed above.

RESOLVED: Send and Sync lean on the wider type-property model (deep
immutability, mutable owned values being affine), which is settled in
[TIP-0001](../tips/0001-mutability-and-borrowing.md) (Accepted) — mechanism (b),
type-level `!T` plus affine mutable values. No further reconciliation needed.
