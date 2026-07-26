# TIP-0009: Inline lambdas and non-local control flow

**Status:** **Accepted and migrated** into the chapter docs (2026-07-03) —
non-local control flow is settled in
[Early Return](../08-control-flow/06-early-return.md#lambdas-and-the-enclosing-function-question),
[Closures and Lambdas](../09-functions/06-closures-and-lambdas.md#lambda-return),
and [Return Values](../09-functions/03-return-values.md#returning-from-an-outer-scope).
Kept as the historical record.
**Created:** 2026-06-14
**Touches:** `09-functions/06-closures-and-lambdas.md` (the `inline lambdas`
and `Lambda return` open questions), `08-control-flow/06-early-return.md`
(the "return from the lambda vs the outer function" call),
`09-functions/03-return-values.md` (*Returning from an outer scope*),
`13-error-handling/01-philosophy.md` (callbacks "cannot `return` from, or
`break` in" the caller), `17-standard-library/14-observability-and-logging.md`
(the `log.sub(...) { … }` block needs these semantics),
`07-expressions/06-function-application.md` (trailing blocks and lazy
arguments — the same call shape this rides on).

## Summary

A plain lambda is a *value*: a `return` inside it exits the **lambda**, and it
cannot `break`/`continue` the caller's loops or assign to the caller's locals.
That is the right default. But it blocks the one thing that makes a
user-defined helper feel like *language syntax* rather than just *look* like it:
a block argument to a custom `unless`, `with_lock`, `repeat`, or `log.sub(...)`
that can leave the **enclosing** function the way a built-in `if`/`for` body
can.

An **inline lambda** is the answer: a block argument to a function marked
`inline`, expanded at the call site, so a *marked* `outer return` /
`outer break` / `outer continue` in the block can leave the **declaring
function** — the function whose source the block is written in — while a bare
`return` stays local to the block. This TIP collects what Tel already says,
surveys what other languages did and what they like or regret, and proposes the
Tel shape: **non-local control flow only through an explicit `inline` marker on
the function, only for non-escaping block parameters, spelled `outer` at the
jump and targeting the declaring function — never implicit, never for a lambda
that escapes.**

## What Tel says today

The idea is referenced in five places, all leaning the same way but none
deciding:

- **Closures and lambdas** carries the open marker directly:
  > `TODO(open): inline lambdas.` Whether a lambda passed to an *inline*
  > function can `return` / `break` / `continue` out of the **enclosing**
  > function … Lean: worth it for the DSL/control-flow story, but only as an
  > explicit `inline` marker on the function, never implicit.

  and fixes the *default*: "A `return` inside a lambda exits the *lambda* … it
  does not exit the function that built the lambda."
- **Early return** frames the ambiguity ("does it exit the lambda, or the
  function that contains the lambda?") and leans: "a `return` in a lambda exits
  the *lambda*; non-local return, if offered, needs explicit opt-in syntax.
  *Surprise is a cost — prefer the obvious.*"
- **Return values** names the mechanism: "An **inline function** can be defined
  so that a `return` written in a block-argument passed to it exits the
  *enclosing* function."
- **Error-handling philosophy** is *why this matters*: callbacks were rejected
  as the failure mechanism partly because "a callback cannot `return` from, or
  `break` in, or touch the locals of the calling function — not without special
  machinery (inline lambdas …)."
- **Observability** has a concrete customer: `log.sub("import") |log| { … }`
  mirrors lexical scope, but "a block makes it harder to … `return` / `break`
  out of the surrounding function — this is exactly the case for **inline
  lambdas**."

So the feature is wanted, the default (local return) is settled, and the only
open question is the *shape* of the opt-in.

## The problem precisely

A user-defined helper that takes a block:

```tel
fn with_lock(m: Mutex, body: || -> R) -> R { ... body() ... }

with_lock(m) {
    let row = table.get(id)?       # can `?`/return reach the OUTER function?
    if row.stale { return None }   # does this exit with_lock, or the caller?
    row.value
}
```

With an ordinary lambda, `return None` exits the *block* and becomes
`with_lock`'s result — almost never what the author meant, and a silent trap.
Three powers are at stake, and a plain lambda has none of them over the caller:

1. **Non-local `return`** — leave the enclosing function from inside the block.
2. **Non-local `break` / `continue`** — drive a loop in the enclosing function
   (what lets a custom iterator helper replace a built-in `for`).
3. **Transparent locals & effects** — read/assign the caller's bindings and
   share its effect row, so the helper is as *capable* as built-in syntax (it
   is not byte-identical — see the `outer` seam in the proposal below).

A value-lambda deliberately gets none of these. An inline block gets all three
because it is *spliced into* the caller rather than *called by* the helper.

## Prior art — what others did, and what they like or regret

Five distinct strategies exist. Tel can borrow from one without inheriting its
regret.

### 1. Inline + non-local return (Kotlin)

A lambda passed to an `inline fun` is spliced at the call site, so `return`
inside it exits the *enclosing* function. This is what makes `forEach`, `let`,
`run`, `with`, `apply`, `use`, `synchronized`, `repeat` feel built-in, with no
closure allocation. `crossinline` forbids non-local return when the lambda is
handed to a context that might run it later; `noinline` opts a single parameter
out; labelled `return@forEach` disambiguates which frame to leave.

- **Liked:** the control-flow-DSL story is the headline Kotlin win; zero
  allocation; helpers read exactly like syntax.
- **Regretted:** `inline` couples *codegen* (splice, code-size growth) with
  *semantics* (non-local return) — you cannot get one without the other.
  `crossinline`/`noinline` are real cognitive load, and labelled returns are
  extra surface area. Inlining is mildly viral and bloats binaries.

### 2. Two closure kinds (Ruby)

Blocks/`Proc` do non-local return (a `return` exits the home method); `lambda`
(`->`) returns locally. Same syntax, two semantics, chosen by *how the closure
was made*. Blocks gave Ruby its DSLs (Rails, RSpec).

- **Liked:** blocks make iteration and internal DSLs beautiful.
- **Regretted:** the proc-vs-lambda split is a canonical Ruby wart — `return`,
  `break`, and `next` all behave differently between the two, and a non-local
  return from a `Proc` whose method already returned raises `LocalJumpError`.
  Two meanings for one shape is exactly the *surprise* Tel forbids.

### 3. Capability / structured boundary (Scala 3, Common Lisp, Smalltalk)

Scala 3's `boundary { … break(x) … }` introduces a `Label` capability; `break`
unwinds to the nearest enclosing boundary that handed out that label. Common
Lisp's `block`/`return-from` and Smalltalk's `^` are the same idea — a *named
exit point*, not a property of the closure. Scala 3 added this **specifically
to replace** its earlier regret (below).

- **Liked:** explicit, lexically scoped, composes — the exit target is a value
  you can see and pass. Near-zero cost in the optimised path. No special closure
  kind; any block can carry a label capability.
- **Regretted:** little, by design — it is the considered replacement for the
  exception hack. Cost is a small amount of ceremony (the boundary must be
  introduced) and a non-local jump still exists at runtime.

### 4. Deliberately don't (Java, Swift, Rust, JavaScript)

Java lambdas are "functions, not blocks": `return` only ever exits the lambda,
and `forEach` *cannot* `break`. Swift splits escaping vs non-escaping closures
but still gives user code no non-local return, leaning on built-in `guard` /
`defer`. Rust closures cannot non-locally return; you reach for labelled
`break 'outer value`, `?`, and `try_fold`/`try_for_each` instead. JS `forEach`
famously can't `break` (use `for…of` or `some`/`every`).

- **Liked:** total control-flow transparency — you never wonder where a
  `return` goes; `grep` for `return` finds every function exit. Brian Goetz's
  "lambdas are functions" rationale is exactly *readability over writability*.
- **Regretted:** the "can't `break` out of `forEach`" papercut, which pushes
  people back to raw loops — a writability tax accepted for predictability.

### 5. Macros (Lisp, Rust `macro_rules!`)

The most flexible path skips closures entirely: a macro expands at the call
site, so the body is *already* in the caller's frame and every `return`/`break`
is naturally non-local. Tel has [metaprogramming](../15-metaprogramming/), so
this is a genuine alternative to an inline-lambda runtime feature for the
pure-syntactic-sugar cases.

- **Liked:** unlimited control-flow shapes, zero runtime mechanism.
- **Regretted:** hygiene and tooling cost; control flow hidden behind macro
  expansion is hard to read and step through — against *surprise is a cost*.
  Reserve for genuinely syntactic constructs, not as the default DSL tool.

### Summary of the field

| Approach | Non-local exit | Opt-in unit | Main regret |
| --- | --- | --- | --- |
| Kotlin `inline` | yes | `inline fun` | codegen/semantics coupled; `crossinline`/`noinline` |
| Ruby block/lambda | yes (blocks) | closure *kind* | two meanings, one syntax |
| Scala 3 `boundary` | yes | `Label` capability | small ceremony |
| Java/Swift/Rust/JS | no | — | can't `break` a callback |
| Macros | yes | macro | hidden control flow |

## Proposed direction for Tel

Take Kotlin's **explicit `inline`-on-the-function** opt-in, but borrow Scala 3's
discipline that non-local exit is a *visible, restricted* thing rather than a
side effect of how a closure was built. Concretely:

- **Default unchanged.** A lambda is a value; its `return` exits the lambda.
  This is already decided across the chapters and stays the rule.
- **`inline` marks the function, not the call.** Only a parameter of an
  `inline`-marked function can receive a block with non-local powers. The marker
  is on the *declaration*, so a reader of the call site still needs a cue —
  see the open question on call-site visibility.
- **Non-escaping only.** An inline block parameter **cannot escape** — it is not
  stored, returned, spawned into a task, or captured by a longer-lived closure.
  This is the soundness condition (you cannot non-locally `return` into a frame
  that has already gone). It mirrors Swift's non-escaping default and Kotlin's
  `crossinline` restriction, but as the *baseline* for inline params rather than
  an extra keyword. A block that must escape is an ordinary lambda with ordinary
  (local) semantics — no second closure kind, just "inline param or not."
- **Local by default; non-local is a word-marked jump.** Inside an inline block
  a bare `return`/`break`/`continue` is **local** — it targets the block,
  exactly as in any lambda. Reaching the caller's frame is the explicit form
  **`outer return`**, **`outer break`**, **`outer continue`** — a keyword
  modifier, *not* a sigil, matching the `move`/`borrow` capture keywords in
  [closures-and-lambdas](../09-functions/06-closures-and-lambdas.md) (and the
  "name the target at the jump" discipline of Lisp's `return-from`, Scala 3's
  `break(label)`, and Zig's labelled `break`). The two markers then split the
  job cleanly: **`inline` on the function grants the *permission*** (this block
  may reach upward), and **`outer` on the jump states the *intent*** — visibly,
  at the site where control actually leaves. `outer return` exits the
  **declaring function** — the function in whose source the block literal is
  written — *never* the inline helper that received the block (that is also
  Kotlin's non-local-return target). A block is written in exactly one function,
  so the target is unambiguous and lexical: no label, even through a chain of
  inline calls. The two levels then compose — a bare `return v` hands `v` to the
  helper as the block's value; `outer return v` makes the declaring function
  return `v`. All three of `return`/`break`/`continue` get the `outer` form —
  offering only some would be its own surprise. This is what `log.sub(...) { … }`
  and a custom early-exit iterator need.
- **The seam is honest.** So an inline helper is *not* byte-for-byte a built-in:
  a built-in `for` body breaks with a bare `break`, an inline iterator's block
  with `outer break`. That one-keyword difference is a *feature* — it tells a
  reader "control is leaving through a helper, not a language loop," which is
  *surprise is a cost* working as intended. Inline buys built-in *capability*,
  not built-in *invisibility*.
- **Reject the Ruby split** (two closure kinds) and **reject Scala 2's
  exception-based `return`-in-closure** (costly, swallowable by a broad
  `catch` — the very thing Scala 3 walked back). Tel's `inline` splice has
  neither cost nor a throwable to intercept.
- **Macros stay the escape hatch** for genuinely syntactic constructs, not the
  everyday DSL tool — inline functions keep control flow readable and typed.

This keeps every Tel maxim intact: *no implicit anything* (`inline` grants the
power, `outer` spends it — both written out), *surprise is a cost* (this inverts
Kotlin's polarity — the *surprising* non-local jump is the *marked* one, while a
bare `return` stays local), *one good way* (no second closure kind — escape-ness,
already tracked, decides), and *readability over writability* (a reader greps
`outer` to find every non-local exit).

## On acceptance — documentation to update

This TIP is a *proposal*. Until it is accepted, the chapters must keep framing
non-local return as a *lean*, not a settled rule — `outer return` is presented
"under that leaning," never as decided. When this TIP is accepted, apply these
edits together so the docs move in lockstep:

- **`08-control-flow/06-early-return.md`** — replace the "Lambdas and the
  enclosing-function question" TODO block (which still leans "non-local return,
  if offered, needs explicit opt-in syntax") with the resolved rule: a bare
  `return` is local to the lambda; `outer return` / `outer break` /
  `outer continue` leave the **declaring function**, enabled by an `inline`
  marker on the receiving function.
- **`09-functions/06-closures-and-lambdas.md`** — promote the `outer return`
  material in "Lambda `return`" from *leaning* to settled: resolve the
  `TODO(open): inline lambdas` marker and drop the "under that leaning" hedge on
  the `find_ready` example.
- **`09-functions/03-return-values.md`** — the *Returning from an outer scope*
  note firms up from proposal to rule.
- Sweep the remaining files in **Touches** (top of this TIP) for stale "open" /
  "if offered" phrasing about non-local return.

## Open questions

- **Call-site visibility — resolved.** Kotlin's regret is that a non-local
  `return` is invisible at the call (the marker lives on the declaration). Tel
  avoids it by inverting the polarity: a bare `return`/`break`/`continue` is
  always local, and the non-local jump is the explicit `outer return` /
  `outer break` / `outer continue` written *at the jump*. No call-site sigil or
  bracket is needed — the `outer` keyword is the cue.
- **Multi-level targeting — resolved for `return`.** Because `outer return`
  names the *declaring* function and a block is declared in exactly one place,
  there is no "which level" to choose: no label is ever needed for return, even
  through nested inline blocks (they share the one declaring function).
- **`outer break` / `outer continue` loop target — resolved.** `outer break`
  breaks the nearest loop enclosing the block **at the site where the block is
  written**. Because an inline call is *spliced* into the declaring function, the
  helper's own loop (e.g. the `for` inside an `each_row` iterator) is, after
  inlining, exactly that nearest enclosing loop — so "the loop where the block is
  defined" and "the loop the inline iterator represents" are the same loop. A bare
  `break` ends the block (the helper's per-item step); `outer break` stops that
  loop. (`TODO(open):` multi-level — reaching a loop *beyond* the immediately
  enclosing one — is deferred; pin it with the return/yield disambiguation below
  if a real case needs it.)
- `TODO(open):` **`return` vs `yield` disambiguation.** Named in
  [closures-and-lambdas](../09-functions/06-closures-and-lambdas.md); an inline
  block that drives a generator-style helper must distinguish "value for this
  iteration" from "leave the enclosing function." Resolve jointly with
  [for-loops-and-iteration](../08-control-flow/04-for-loops-and-iteration.md).
- `TODO(open):` **relationship to lazy arguments.** An inline block and a
  [lazy argument](../07-expressions/06-function-application.md#lazy-arguments)
  are both "code passed unevaluated, run in the caller's context." Are they one
  feature with two surfaces, or distinct? Pin down before either ships.
- `TODO(open):` **effect-row sharing.** If an inline block shares the caller's
  effects, an `inline` function's own effect signature must be transparent to
  the block's effects (it cannot claim `pure` while splicing effectful caller
  code). Specify how the inline marker interacts with effect inference.
- **Lambda receivers overlap — resolved (orthogonal).**
  [TIP-0010](0010-lambda-receivers-and-builder-dsls.md) settles this: receivers
  and inline blocks are **separate, composing features**, not one. A receiver
  supplies *context* (the block's `this`; bare names resolve through it, the same
  rule a method body uses), while `inline` supplies *control flow* (`outer
  return` / `outer break` / `outer continue`). The one coupling point is
  escape-ness — a receiver block that is *also* inline must obey the non-escaping
  condition above to keep its `outer` powers. Note the directions never collide
  on syntax: a **bare name** reaches into `this`, **`outer`** reaches up — and
  the leading `.` is left to method chaining, used by neither.
```
