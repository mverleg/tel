# TIP-0012: Task Cancellation, Abort, and Shutdown

**Status:** Draft
**Touches:** `14-concurrency-and-parallelism/04-structured-concurrency.md`, `14-concurrency-and-parallelism/08-cancellation-and-timeouts.md`, `12-memory-and-runtime/08-substructural-types.md`, `13-error-handling/04-panics-and-aborts.md`, `02-philosophy/04-antifeatures.md`

<!-- TODO: review -->

## Summary

Three intertwined questions about how a task *stops* and how the program *stops*:

1. **Cancellation** — may a task be stopped by the runtime at any
   [yield point](../14-concurrency-and-parallelism/08-cancellation-and-timeouts.md),
   or only through application logic (a stop signal the task chooses to
   observe)?
2. **System abort** — is there a user-callable hard stop (`System.exit()`-style)
   that ends the program (or a task) *without* running cleanup?
3. **Shutdown** — do tasks whose join handle was dropped ("detached" in the weak
   sense of [TIP-scoped joining](../14-concurrency-and-parallelism/04-structured-concurrency.md#joining-and-when-a-handle-may-be-dropped),
   *not* free-floating) block program shutdown?

These are one question wearing three hats: **on which termination paths does a
value's must-use obligation (its "discard") get discharged?**

## The lens: discard must run

The **paramount** consideration is a hard constraint, not a preference:

> Every value's must-use discharge — its `close`/`commit`/`rollback`/flush, i.e.
> its [relevant](../12-memory-and-runtime/08-substructural-types.md#relevant-and-the-discard-capability)
> "use" — **must run on every termination path except true catastrophe** (OOM or
> the host tearing the guest down), whether the use was placed automatically
> ([`AutoUse`](../12-memory-and-runtime/08-substructural-types.md)) or written
> explicitly. A **panic is not an excuse to skip it**: a [NoPanic cleanup
> unwind](../12-memory-and-runtime/08-substructural-types.md#cleanup-on-abort-a-limited-unwind-but-no-recovery)
> settles the task's live linear resources before it dies.

The **secondary** consideration is usability: cancellation, timeouts, and races
should be expressible without every library hand-rolling a stop protocol.

The paramount consideration does most of the deciding, and it decides via one
derived principle:

> **Cancellation is a *value* propagated by a normal return, not a hidden
> unwind.**

Why this follows: relevance already guarantees discard runs *on the normal
path*, and the compiler already proves no linear value escapes un-discharged on
*every* early-return edge (a `return`, a `?`, an `Err` propagation). If
cancellation surfaces as **a value you observe at a yield point and propagate by
ordinary early return**, then the cancellation path *is* a normal-return path,
and relevance's existing guarantee covers it for free — no new "cancel-safety"
discipline, no shielding machinery, no reliance on an unwinder to fire
destructors. Discard-must-run becomes a *static* property of the cancellation
path, checked exactly like every other path.

This also lands cancellation squarely inside Tel's existing philosophy: errors
are **values, propagated explicitly**, never exceptions that unwind through
arbitrary frames (see [antifeatures](../02-philosophy/04-antifeatures.md) and
[error handling](../13-error-handling/)). A cancellation that unwound
implicitly would be the one exception-shaped control-flow edge Tel otherwise
forbids. Making it a value keeps the whole story uniform.

The consequence for the three questions:

- The **only** paths on which discard does *not* run are **true catastrophe** —
  **OOM** or the **host tearing the guest down** — where the runtime genuinely
  cannot run cleanup. That is tolerated because there is no alternative, and it
  is sound for *memory and OS resources* (isolated heaps + OS reclamation) even
  though it cannot honour *application-level* cleanup. A **panic is not in this
  set**: it runs the NoPanic cleanup unwind (settling live linear resources)
  before the task dies, so its discard *is* honoured.
- There is **no user-callable** discard-skipping abort.
- Detached tasks are ordinary tree members and **block shutdown** until their
  discard has run.

## Recommended outcome (one-line summary)

- **Q1 — Cancellation is cooperative and value-shaped.** No preemption. A task
  observes a **cancellation token** at yield points; observation surfaces as an
  ordinary value (a blocking op returns `Cancelled`, propagated by early
  return), which runs all pending uses on the way out. The token and the
  timeout/race machinery live in the **stdlib**, threaded through the blocking
  primitives so cancellation is ergonomic without being implicit. A task that
  never observes the token cannot be stopped — and that is a **bug** (an
  uncloseable scope), surfaced like a deadlock, not force-killed.
- **Q2 — No user-facing system abort.** True catastrophe (OOM, host teardown)
  exists as a failure mode and must stay sound, but it is never an API a script
  calls; a panic is *not* catastrophe — it runs cleanup first (see below).
  Stopping *the whole guest* is the **host's** prerogative (it drops the guest
  instance), not a guest-callable primitive.
- **Q3 — Detached tasks block shutdown, like every task.** Dropping a handle
  does not detach the task from the tree; it stays auto-joined at scope exit.
  `main()` exit joins the whole tree; each long-running task must be *stoppable*
  (its token trips → normal return → cleanup runs). An unstoppable one hangs
  shutdown, which is a bug.

---

## Q1 — May the runtime cancel a task, and how?

### Options

**A. Preemptive cancellation.** The runtime stops a task mid-instruction.
*Rejected already* and re-rejected here: it is unportable (a browser cannot
preempt a running callback), and it can stop a task with a half-mutated
structure, violating the predictability the isolated-heap model buys. Out.

**B. Cooperative unwind (Kotlin/`CancellationException` style).** The runtime
delivers cancellation at a yield point as a *thrown, implicitly-propagating*
signal that unwinds the task's frames, firing `AutoUse`/destructors as it goes.
- *Honours discard?* Only if the unwinder reliably runs every use — including
  explicitly-written uses, not just `AutoUse` — and only if a cleanup that
  itself yields cannot be re-cancelled mid-flight (which forces a **shielding**
  concept: a non-cancellable region around cleanup).
- *Cost:* it is precisely the implicit, exception-shaped control-flow edge Tel
  rejects. Every function spanning a yield acquires a hidden exit path.
- *Verdict:* dispreferred. Tel *does* have a [NoPanic cleanup
  unwind](../12-memory-and-runtime/08-substructural-types.md#cleanup-on-abort-a-limited-unwind-but-no-recovery)
  on the **panic** path, but that is a last-resort settle for *bugs* — it never
  resumes and runs only NoPanic actions. Routing *deliberate* cancellation
  through an unwinder is a different thing: it gives every yield-spanning
  function a hidden exit and needs shielding. Option C keeps cancellation cleanup
  **static and shield-free**, so it is preferred.

**C. Cooperative token, value-shaped (recommended).** The runtime/stdlib
provides a **cancellation token** scoped to the task subtree. Blocking
primitives (channel send/recv, `sleep`, `join`) are token-aware: when the token
is tripped, they return `Cancelled` instead of blocking. The task propagates
that value by ordinary early return (`?`-style), which runs all in-scope uses.
- *Honours discard?* Yes, *structurally*: the cancellation path is a
  normal-return path, so relevance's existing static check proves every linear
  value is discharged on it. No unwinder, no shielding needed — cleanup is just
  the ordinary code that runs on an early return.
- *Usability?* Recovered by putting the token in the stdlib and threading it
  through the standard blocking points, so `run_with_timeout` and `race_first`
  trip the token and cooperating operations stop promptly — without each library
  inventing its own stop channel.
- *Consistency?* It is the concurrent mirror of Tel's error model: a
  `Cancelled` value propagated explicitly, not a hidden throw.

The token is **stdlib, not language surface**, and tokens **nest**: each task
subtree derives its token from its parent's, so tripping a token trips every
token derived from it. The cancellation tree mirrors the [task
tree](../14-concurrency-and-parallelism/04-structured-concurrency.md#the-task-tree)
— cancelling a node cancels its descendants because their tokens are children of
its own. `run_with_timeout` and `race_first` create a derived child token and
trip it (on the deadline, on the first winner).

TODO(open): **Is the token ambient to the task, or an injected capability?**
Observing cancellation is effect-shaped — the same shape as `panics`, which is
an [ambient capability the compiler
injects](../05-types/05-function-types.md#ambient-capabilities-panic-allocation),
resolved at the call site. So a `Canceller` could ride that ambient set (no
threading, mirrors the task tree) rather than being passed like `Clock`. That is
ergonomic, but sits against Tel's "nothing is ambient" capability discipline (no
global `now()`). Blocking stdlib primitives observe the token either way; the
open part is how an *explicit* `cancel_check()` in compute-bound code reaches it.
Decide jointly with the effects question in
[`05-types/05-function-types.md`](../05-types/05-function-types.md#ambient-capabilities-panic-allocation).

### Why the paramount consideration selects C over B

Both B and C can *technically* honour discard-must-run. The tie-break is
**how**: B relies on an unwinder to run destructors and needs a shielded region
so cleanup is not itself cancelled; C makes the cancellation path a normal
return, so the *existing* relevance check already guarantees discharge and there
is nothing new to trust. "With or without `AutoUse`" is the giveaway — under C,
an explicitly-written `commit()` on the early-return path is checked and run
exactly like an `AutoUse` one, because it *is* just an early return. Under B,
explicit uses depend on the unwinder reaching them.

### The residual: unstoppable work

An operation that never reaches a token-aware yield point — a tight compute loop,
or a synchronous FFI call — cannot be stopped cooperatively. Consistent with
[structured concurrency](../14-concurrency-and-parallelism/04-structured-concurrency.md),
this is a **bug** (a scope that cannot close), surfaced like a deadlock via the
watchdog / missing-`join` mechanism — **not** a case for a runtime force-kill.
A deliberate per-task brutal-kill, if ever offered, is an explicit *supervision*
choice (Erlang-style, after a graceful-shutdown timeout), never a silent
default. Like host-teardown it is a **catastrophe-class** action — it force-stops
without cooperative cleanup, so that task's discard does **not** run — and it is
governed by Q2, not Q1. (This is distinct from a *panic*, which is not an
external force-stop and *does* run the NoPanic cleanup unwind.)

### Application-logic stop is still first-class

Nothing here forces a task to use the standard token. A worker that watches its
own kill-switch [channel](../14-concurrency-and-parallelism/06-channels-and-message-passing.md)
and returns when it closes is a perfectly good, fully-cooperative stop — and
since it returns *normally*, discard runs by construction. Option C is really
"the stdlib ships a standard, ergonomic instance of this pattern and wires it
into timeouts/races," not "a new kind of control flow."

### Does value-shaped cancellation colour functions?

The fair worry: if a cancellable operation returns `Cancelled`, every function
that transitively calls one gains `Cancelled` in its error type and propagates it
with `?` — isn't that [function
colouring](../14-concurrency-and-parallelism/03-async-and-function-colouring.md)
creeping back through the error channel?

**No new colour** — for three reasons, with one bounded cost:

- **It rides the error channel Tel already has.** `Cancelled` is an ordinary
  error value, not a second function-type axis. It propagates through `?` exactly
  like any `Err`, and a `Result`-returning function is callable from anywhere —
  there is no "only callable from a cancellable function" split, which is the
  defining harm of async colouring. The virality is the *existing* `Result`
  virality (every error exit written down), which Tel already treats as a
  feature.
- **The token is ambient, not a threaded parameter.** It rides the task subtree
  and is read by the stdlib blocking primitives, so functions do **not** grow a
  `token` argument — the difference from Go's `context.Context`, whose explicit
  threading *is* the mild colour. (Observation is explicit; the token is still
  not a parameter — see the ambient-vs-capability tension noted under Q1 option
  C.)
- **No generic-surface doubling.** Because cancellation lives inside the ordinary
  error type `E`, a combinator already generic over `E` carries it for free;
  there is no `Fn` vs `cancellable Fn` pair the way async needs `Fn` vs
  `async Fn`. The higher-order case folds into ordinary error-generic code.

The **one genuine cost:** an operation that is conceptually *infallible but
blocking* — `sleep`, `join`, a plain `recv` — becomes fallible, because it must
be able to return `Cancelled`. That widens the set of `Result`-returning
functions slightly. It is bounded to functions that actually reach a cancellable
blocking point, and its ergonomic weight hinges on **error-union inference**: if
`E` is an inferred/open union, `Cancelled` flows through `?` without being
spelled per signature and the cost is nearly invisible; if `E` must be declared
closed, every such function has to name `Cancelled` and the threading becomes
real. This is a strong argument for inferred/open error unions on the propagation
path.

TODO(open): confirm the [error model](../13-error-handling/03-error-propagation.md)
infers/opens the `Cancelled` variant through `?` rather than forcing every
cancellable function to declare it — that inference is what keeps value-shaped
cancellation from becoming a de-facto colour.

---

## Q2 — Is system abort supported?

### True catastrophe (kept, sound, not an API)

Two terminations are outside the language's control: **OOM** and the **host
tearing the guest down**. On these the runtime drops heaps wholesale and runs
*no* uses. This is the **only** sanctioned discard-skipping path, and it is
acceptable only because:

- there is no alternative (you cannot reliably run cleanup after OOM or a host
  kill), and
- it is sound for **memory and OS resources** — isolated heaps vanish with no
  cross-task repair, and the OS reclaims raw handles (fds, sockets).

It is **not** sound for *application-level* semantics: a buffered write is lost,
an uncommitted transaction rolls back. That is the accepted cost of a true
crash, and it is why catastrophe is a *failure mode*, never a *shutdown
mechanism* a program reaches for.

A **panic is not catastrophe.** An `assert`, a bad index, a `todo`, a panic
reaching the root — all abort the task, but the runtime *can* still run cleanup,
so it does: the [NoPanic cleanup
unwind](../12-memory-and-runtime/08-substructural-types.md#cleanup-on-abort-a-limited-unwind-but-no-recovery)
settles the task's live linear resources (its `AutoUse` actions / covering
`finally`s) before the heap is reclaimed. So a `commit`/`close`/flush *is*
honoured on a panic — which is what stops a **task bomb** (panicking a task to
dodge a must-use) from silently skipping cleanup. Only the two genuine
catastrophes above skip it.

### User-callable hard exit (rejected)

A script-callable `System.exit()`-style abort is rejected on three independent
grounds, any one sufficient:

1. **It violates the paramount consideration by design** — it skips every
   pending use. A language whose type system promises "discard must run" cannot
   also hand out a primitive whose whole purpose is to not run it.
2. **Surprise control flow** — it bypasses every scope's cleanup from an
   arbitrary point, exactly the [antifeature](../02-philosophy/04-antifeatures.md)
   Tel excludes.
3. **Embedding** — Tel is a *guest*. A guest that calls `exit()` tears down the
   **host** (the game engine, broker, IDE it lives in), reaching outside its
   sandbox to kill its parent. Stopping the guest is the host's decision, made
   by dropping the guest instance — served entirely from the host side, needing
   no guest API.

So "stop everything now" is not expressible from inside Tel. The closest
*deliberate* mechanism is a graceful shutdown that trips the root token and
joins (see Q3); the closest *involuntary* one is a panic reaching the root —
which still runs cleanup on the way up (below) before aborting the guest — the
terminal case of ordinary panic propagation, not a new primitive.

### A panic reaching the root

The root is **not special-cased**. A panic reaching it takes the same path a
clean `main()` exit does (Q3): it **runs the cleanup unwind** for its own live
linear resources, and the root scope still **blocks joining every detached
(tree-owned) task** — tripping the tree's cancellation token so cooperating
tasks stop, clean up, and join — before the guest exits. Only *then* does the
default handler run: it **prints a stack trace and sets a failure exit status**.

This composes with `?`: a `?`-propagated error out of `main` (or any frame) is
an **ordinary early return**, so it takes the identical cleanup-and-join-detached
path — it is not an abort. The difference is only the report at the very end: a
panic prints the trace and sets the failure status; a normal `?` return exits
through `main`'s own result (which may still set a non-zero status if the program
chooses, but runs no panic handler).

TODO(open): spell the exact host-facing contract for a root panic — how the trace
and exit status are surfaced to the host (returned error vs host callback) — in
[the FFI story](../16-ffi-and-interop/).

---

## Q3 — Do detached tasks block shutdown?

### They are not really detached

"Detached" here is the weak sense from
[structured concurrency](../14-concurrency-and-parallelism/04-structured-concurrency.md#joining-and-when-a-handle-may-be-dropped):
the *handle was dropped*, which is only permitted when the return type is
`Discard`. The task is **still a child of its scope** and is **auto-joined at
scope exit**. The forbidden free-floating detach (a task that outlives its
parent, a daemon) does not exist.

### So: yes, and it is mandatory

Because the task stays tree-owned, `main()` exit — which is just the root scope
closing — joins it along with everything else. And discard-must-run *requires*
this: a detached task may hold linear values internally whose uses must run, so
it must reach normal completion, so its scope must join it, so it blocks
shutdown until it is done. Detached and non-detached tasks are identical here;
dropping the handle only gave up *individually collecting the result*, never the
join itself.

The join is graceful: at shutdown the root trips the standard cancellation token
(Q1 option C), cooperating long-running tasks observe it, return normally, and
run their cleanup; then the join completes. A task that ignores the token hangs
the join — a bug, per Q1, not a licence to abort it.

### `main()` exit, precisely

- Join every task. Each long-running task must be **stoppable** (its token trips
  → normal return → discard runs).
- An unstoppable long-running task is a **bug** (uncloseable scope), surfaced
  like a deadlock. It is not routinely force-killed.
- Only a **true catastrophe** (OOM / host-teardown, Q2) ends such a task with its
  discard skipped; a panic still runs cleanup on the way out.

---

## Interactions and edge cases

- **Cleanup that itself yields.** A use may block (flush, `COMMIT`) — Tel's
  [function colouring](../14-concurrency-and-parallelism/03-async-and-function-colouring.md)
  allows it. Under option C the cleanup runs on the *normal early-return* path,
  so it is ordinary code. Cancellation is only ever observed **explicitly** — a
  use does *not* implicitly re-check the task token — so an already-tripped token
  cannot silently abort a cleanup mid-flight: there is nothing to re-fire. If a
  cleanup wants to stay interruptible (a flush that should itself time out), it
  observes a *fresh, explicitly-created* cancellation context; it never inherits
  the tripped one implicitly. So "explicit observation" *is* the shield — no
  automatic runtime token-swap and no separate shielding construct are needed.
  TODO(open): what a *second*, explicitly-observed cancellation during cleanup
  should do — the one remaining escalation candidate; lean: it is the cleanup
  author's choice (observe and stop, or ignore and finish), never an implicit
  abort, so the "no discard skip" invariant is never at silent risk.
- **Panic cleanup vs cancellation cleanup.** The two differ only in *how they
  are reached*: cancellation cleanup runs as ordinary code on a *normal early
  return*; **panic cleanup** runs on the NoPanic unwind. There is **no
  'blocking vs non-blocking' distinction** to settle — Tel has [no async
  colouring](../14-concurrency-and-parallelism/03-async-and-function-colouring.md),
  so a cleanup action is just code; if it does I/O the host parks the fiber,
  exactly as anywhere else. The only constraints on a panic-unwind action are
  that it is **NoPanic** (it cannot itself panic → no double-panic; a blocking op
  that *fails* must **absorb** the error, since there is no caller to hand a
  `Result` to) and that an `AutoUse` action returns `Discard` (nothing relevant
  to re-settle on the unwind). An explicitly-called consuming method on the
  normal path keeps its freedom to block, return `Result`, and be re-cancelled.
- **Timeouts** are token trips on a clock deadline (see
  [cancellation and timeouts](../14-concurrency-and-parallelism/08-cancellation-and-timeouts.md));
  they inherit everything above, including the injected `Clock` for
  deterministic testing.
- **`race_first` / `await_all`** trip the token on losers/siblings; those tasks
  stop the same cooperative, discard-running way.
- **Monitors** (a supervisor observing a task's death) are unaffected: a
  cancelled task still delivers an outcome (`Cancelled`) to whoever joins or
  monitors it.

## Conflicts with current docs to resolve on acceptance

- `08-cancellation-and-timeouts.md` currently says a cancelled task's heap is
  "dropped wholesale — the same clean teardown as a panicking task." Both
  teardowns now *do* run cleanup first — cancellation via a normal return, a
  panic via the NoPanic unwind — and only then reclaim the heap, so the wording
  is no longer *wrong*; but it must distinguish the two **mechanisms** and stop
  implying a panicking task skips its uses.
- The same chapter presents `h.cancel()` as if the runtime force-stops at a
  yield point. Re-cast it as *tripping the task's cancellation token*, with the
  task observing it as a value — not an injected unwind.
- `13-error-handling/04-panics-and-aborts.md`, `12-memory-and-runtime/03-memory-management.md`
  and `12-memory-and-runtime/08-substructural-types.md` were **already updated**
  (commit `1e33f29`) to the cleanup-on-panic model this TIP now matches: a NoPanic
  cleanup unwind settles linear resources on the abort path, with only
  OOM/host-teardown skipping. This TIP is consistent with them; no further change
  is needed there.
- `04-structured-concurrency.md` carries `TODO(open)` markers pointing here for
  the cancel-primitive, user-exit, and unstoppable-task questions; on acceptance,
  fold the resolved Q1/Q3 answers back and drop the markers.

## Open questions and follow-up tasks

Collected from the design discussion. Detailed context is at each linked spot.

**Decisions still open**

1. **Ambient vs injected cancel token** — is cancellation a compiler-injected
   ambient effect (a `Canceller`, like `panics`) or an explicit capability
   (like `Clock`)? See Q1 option C and
   [`05-types/05-function-types.md`](../05-types/05-function-types.md#ambient-capabilities-panic-allocation).
2. **Cleanup against a tripped token** — does a value's use run against a fresh
   cancellation context automatically, or via an explicit shield? And what does
   a *second* cancel during cleanup do — the one escalation-to-abort candidate?
   See "Interactions and edge cases."
3. **Blocking panic-cleanup** — may a NoPanic cleanup action yield/block (a
   flushing `close`) on the unwind, or must it be non-blocking? See
   "Interactions and edge cases."
4. **Root-panic host contract** — exact behaviour and host-facing report when a
   panic reaches the root with no handler. See Q2 and
   [the FFI story](../16-ffi-and-interop/).
5. **Per-task brutal-kill** — whether Tel offers a supervision-level force-kill
   at all (catastrophe-class, discard skipped), or leaves an unstoppable task as
   a pure bug. See Q1's residual.

**Follow-up doc tasks (on or before acceptance)**

6. **Rewrite `08-cancellation-and-timeouts.md`** — recast `h.cancel()` as a
   token trip; distinguish cancellation cleanup (normal return) from panic
   cleanup (NoPanic unwind); drop any "wholesale drop like a panic" framing.
7. **Rewrite "Borrowing in a scoped task" in `04-structured-concurrency.md`** —
   no cross-task borrows; immutable crosses by semantic-copy / physical-share;
   mutable by move or channel; real borrows are intra-task only. (From the
   no-cross-thread-pointer decision earlier in the discussion; not yet drafted.)
8. **Fold Q1/Q3 answers into `04-structured-concurrency.md`** and drop its
   `TODO(open)` cancel-primitive / user-exit / unstoppable-task markers.

**Adjacent, pre-existing**

9. **Monitors API shape** — a variant of `join`, a `monitor(handle)` call, or a
   supervisor abstraction (existing `TODO(open)` in
   `04-structured-concurrency.md`).
10. **The `with`-shaped resource construct** — an exit action that runs on
    normal exit, panic, *and* cancellation (existing `TODO(open)` in
    `08-cancellation-and-timeouts.md`); it is the concrete surface tying both
    cleanup paths together.

## Prior art

- **Go `context.Context`** — the closest match: a cancellation token threaded
  explicitly through call chains; a cancelled `ctx` makes blocking ops return an
  error you propagate. Tel's option C is this, minus the manual threading (the
  token rides the task tree and the stdlib blocking primitives), plus static
  discharge (relevance proves cleanup runs on the cancelled return).
- **Trio / anyio** — cancel *scopes* with checkpoints; cancellation delivered at
  `await` points. Structured-concurrency-native, but Python delivers it as an
  exception (unwind). Tel keeps the scope/checkpoint shape, drops the exception.
- **Kotlin coroutines** — cooperative `CancellationException` + `isActive`
  checks; needs `NonCancellable` shields around cleanup. This is option B, and
  the shield requirement is exactly the complexity option C avoids.
- **Swift** — `Task.isCancelled` / `checkCancellation()`, cooperative and
  value/flag-shaped rather than a throw-by-default. Close to option C in spirit.
- **Erlang/OTP** — graceful shutdown then **brutal kill** after a timeout. Tel
  takes the graceful half; the brutal-kill half is an explicit supervision
  escalation (an abort), never a default, per Q1's residual.
- **Rust async** — cancelling a future is a synchronous `drop`, which makes
  "cancellation safety" a subtle per-future property and can leave invariants
  half-updated with no cleanup. Tel's isolated heaps + value-shaped cancellation
  dissolve most of this; the residual is host-side state, owned by the host.
