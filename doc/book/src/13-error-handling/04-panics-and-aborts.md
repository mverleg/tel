# Panics and Aborts

Some failures are not part of a function's contract — a broken invariant, an
`assert` that did not hold, an unreachable branch reached, a [`todo`](02-result-types.md)
placeholder called. They mean the program's assumptions are already wrong.
Tel's response to these is to **abort**: stop the current task at once.

## Abort, never catch

When Tel hits an unexpected failure, it **aborts the task**. It does not raise a
catchable exception, and no handler can *resume* the failed work. There is
exactly one model: **panic equals abort**.

Aborting does run a **cleanup-only unwind** on the way down — it settles the
task's live linear resources (see [Cleanup and the abort
path](#cleanup-and-the-abort-path)) and runs any `finally` blocks — but this
unwind can only *augment* the teardown, never *stop* it. A `finally` is **not** a
`catch`: it may release a resource as the task dies, but it cannot swallow the
abort or resume execution past the panic point, and it is itself **NoPanic**. So
there is no `try`/`catch`, no exception object travelling upward, no handler that
can recover the failed work. Ordinary Tel code cannot *intercept* an abort. The
only place a failure is *contained* rather than fatal is a task/fiber boundary —
see [recovery](05-recovery.md) — and even there the failed task is destroyed, not
resumed.

## Why abort and not unwinding

This is a deliberate, load-bearing choice, and it pays off in several places:

- **No "what if it throws here?"** With catchable exceptions, every function
  must be written to be correct even if any call inside it unwinds partway
  through. Abort removes that entire class of reasoning — *crash by default;
  recover at the boundary, not in the middle of the work*.
- **No defensive `unwrap`.** In an unwinding language, joining a task or taking
  a lock can itself fail because the other side panicked, so every such call
  needs handling. With abort, a panicked task is simply gone; there is no
  poisoned-lock, failed-join ceremony to thread through clean code.
- **Linear types still get cleaned up — without *recover*-unwinding.** A
  recovering unwind (Rust-style) forces every function to be correct even if a
  call inside it throws partway through (Rust's `from_fn` needs a drop guard for
  exactly this). Tel keeps that reasoning out: its cleanup unwind only *settles*
  live linear resources and runs `finally` blocks — all **NoPanic** — and then
  the task still dies. It never resumes into half-finished work, so there is no
  "what if it throws here?" mid-expression. Cleanup on abort is not optional,
  though: without it you could leak any linear resource by moving it into a task
  and panicking it (a **task bomb**), which a linear type system must not permit
  — see [substructural
  types](../12-memory-and-runtime/08-substructural-types.md#cleanup-on-abort-a-limited-unwind-but-no-recovery).
- **Safe to embed.** A host embeds Tel and stays in control. An abort ends a
  task cleanly and predictably; it never leaves the host's process in a
  half-unwound state. Tel should never abort the *host* — only the Tel task.

## Cleanup and the abort path

A **cleanup-only unwind runs on abort.** As a task tears down, Tel settles every
live **linear (relevant) resource** — running its
[`AutoUse`](../12-memory-and-runtime/08-substructural-types.md#autouse--relevance-without-ceremony)
action, or the `finally` that covers it — and only then discards the task. This
is **required, not optional**: without it a linear resource could be leaked by
task-bombing (panicking a task that owns it), and a linear type system must not
permit that. See [substructural types: cleanup on
abort](../12-memory-and-runtime/08-substructural-types.md#cleanup-on-abort-a-limited-unwind-but-no-recovery).

The cleanup unwind is deliberately limited:

- It only *augments* teardown; it **cannot recover** (see
  [above](#abort-never-catch)). Every action it runs — `AutoUse` actions,
  `finally` blocks — is **NoPanic**, so the unwind itself can never panic.
- **Pure in-heap values** (affine builders, plain data) need no per-value action:
  the task's isolated heap is reclaimed in bulk — the Erlang-style isolation in
  [recovery](05-recovery.md).
- A **strict** relevant resource with **no `AutoUse`** — one whose settle needs a
  choice (`commit` vs `rollback`) — has no action the unwind can run for it. Such
  a value may not simply be left live across a panic: either wrap it in a
  `finally` that settles it, or keep it inside a **no-panic region** so no abort
  can occur while it is live (see [requiring a no-panic
  region](#requiring-a-no-panic-region)).
- **External resources** modelled as linear resources are settled by their
  `AutoUse`/`finally` on this path. Anything *not* so modelled — a raw host
  handle behind a [capability](../02-philosophy/03-features.md) that no linear
  value owns — is still reclaimed by the host when it tears the task down, since
  Tel is garbage-collected and RAII does not apply to it.

## Provable panics: warn, do not reject

A tempting idea: once the compiler is smart enough to *prove* that a piece of
code always panics — a `todo` on every path, an assertion that can never hold, a
pre-condition the call site provably violates — it could refuse to compile the
program at all. Tel does **not** do this. A provable panic is a **warning (a
lint), never a hard compile error.**

The reason is the [stable surface](../02-philosophy/01-priorities.md) priority.
A program that compiles today must keep compiling years from now. If a *future*,
smarter compiler started rejecting a program it used to accept — purely because
it grew able to prove a panic the older compiler missed — that is a
backwards-incompatible change disguised as an improvement. Worse, it is
non-monotonic: adding compiler intelligence would break working code.

This is also a different category of check from type errors, and the difference
is what makes "reject it" the wrong default:

- A **type error** fires when the compiler *cannot prove the code valid*. It is
  conservative — sound by rejecting anything it is unsure about — and the rules
  are fixed, so what type-checks today type-checks forever.
- A **provable-panic** finding fires only when the compiler *can prove the code
  invalid*. Its power grows with the compiler, so the set of rejected programs
  is open-ended rather than fixed. Promoting it to an error hands the compiler a
  moving veto over previously-valid code.

So the proof is surfaced as a diagnostic the author can act on, not a gate. The
same reasoning applies to **provably-violated pre/post-conditions**
([design-by-contract](../02-philosophy/03-features.md),
[refined types](../05-types/12-refined-types.md)): when the compiler can show a
contract is always broken at a site, it warns; it does not refuse the build.

TODO(open): whether the warning is on by default, lint-gated, or escalatable to
an error per-project (a strict-mode knob — see
[features: strict mode](../02-philosophy/03-features.md)). Lean: on by default,
escalation opt-in, never on by default, so the stable-surface guarantee holds
for the baseline dialect.

TODO(open): **provable-panic lints must not fire inside tests.** A unit test
often *deliberately* exercises a panicking path or a contract it means to
violate — asserting that bad input aborts, or that a pre-condition rejects a
value (see [testing](../14-testing/01-testing.md)). A lint that flagged "this
provably panics" inside such a test would be noise at best and would block the
build at worst. Decide how test bodies opt out: exempt all `test` blocks, or
provide a per-assertion "this is expected to panic" marker the lint respects.

## Requiring a no-panic region

The [`panics` effect](../05-types/05-function-types.md#effects-belong-on-the-function-type)
is the mechanism for *demanding* that code cannot abort. Panic is a **default-on
ambient capability** (like allocation): ordinary code may panic, and that is
inferred, not annotated. To require the opposite — *this must not panic* — a
function opts out with `pure` / `total`, which the compiler verifies carry no
`panics` effect. That is how you say "no-panic" even though the ambient default
is "may-panic".

The design notes once floated the inverse: make panic **opt-in** — don't
auto-inject the panic capability, so code is no-panic by default and must ask
for the ability to abort. Tel **rejects** that. The settled decision (see
[effects on the function type](../05-types/05-function-types.md#ambient-capabilities-panic-allocation))
is panic-by-default, opt-*out*. Opt-in panic would force nearly every function —
anything that indexes an array, asserts, or calls `todo` — to declare a
capability, against *readability over writability*. The guarantee you actually
want ("this critical span cannot abort") is the rarer case, so it is the one
that is spelled.

Holding a [relevant ("must be used")](03-error-propagation.md) value across code
that *might* panic is safe — but for a more specific reason than "the heap is
dropped": on abort the [cleanup unwind](#cleanup-and-the-abort-path) **settles**
that value (its `AutoUse` action, or the `finally` covering it) before the task
dies. A must-use resource that provides an `AutoUse` is therefore safe to hold
across a `panics`-effect call with no annotation. The case that still needs care
is a **strict** relevant resource with no automatic settle action — there a
`finally` or a no-panic region is what makes it safe, because the unwind has
nothing to run on its own.

The other half of this — *killing a task* when a fallible step fails, and
needing a "stop switch (a channel?) so a task can stop itself at predefined
points" — is the **cooperative cancellation** model, settled in
[cancellation and timeouts](../14-concurrency-and-parallelism/08-cancellation-and-timeouts.md):
a task is not insta-killed mid-instruction; it observes a cancellation request
at a yield point and stops there. That is exactly the predefined-stop-point
"stop switch", generalised so each script does not reinvent it.

TODO(open): whether Tel offers a **block-level** no-panic guarantee (a `pure { … }`
region inside an otherwise-effectful function) in addition to the function-level
`pure` / `total`. The useful unit is sometimes a *span* — "nothing between here
and there can panic" — not always a whole function. A scoped form is more ergonomic
for the critical-section case but adds surface; decide alongside the effect
system in [function types](../05-types/05-function-types.md).

## When to abort vs. return an `Err`

- **Return an `Err`** for anything a caller could reasonably expect and handle:
  rejected input, a missed lookup, a full channel, malformed data. These are
  [expected failures](01-philosophy.md) and belong in a
  [`Result`](02-result-types.md).
- **Abort** for anything that means the code itself is wrong: a violated
  pre/post-condition, an "impossible" branch, an unfinished `todo`, an array
  index that cannot be in range. Aborting here is correct — continuing would run
  code on top of state already known to be invalid. *When in doubt, fail — fast
  and loud.*
- **Abort** is also legitimate for a condition the program genuinely cannot
  recover from, or chooses not to bother handling — not only for outright bugs.
  The judgement call is *binary vs library*:
  - A **binary** may abort where its author judges a failure unrecoverable and
    not worth the stability/recovery cost. That is the application's call,
    informed by the external systems it talks to and how much uptime it owes.
  - A **library** should prefer returning a `Result`, because it cannot know how
    much stability the end application expects. It aborts on a bug in *itself*,
    not on a condition the caller might reasonably want to handle — and never on
    bad *input*, which is the caller's expected error.

TODO: review — new section; the cleanup/linear-types question is the open call.
