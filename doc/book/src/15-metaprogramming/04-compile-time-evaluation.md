# Compile-Time Evaluation

<!-- TODO: review -->

**Compile-time evaluation** means running ordinary Tel code *during
compilation* and baking its result into the program — for example, computing a
constant or a lookup table once, at build time, instead of on every run.

Unlike [macros](01-macros.md), this generates no code: it evaluates code that
is already written. It is the most conservative item in this chapter and the
one most clearly compatible with Tel's priorities.

## What it is

Tel evaluates **pure, constant expressions** at compile time. A function with
no [effects/capabilities](../16-ffi-and-interop/04-embedding-tel-in-a-host.md)
— no I/O, no clock, no randomness — applied to constant arguments has a
result the compiler can compute once and store.

```tel
# Illustrative — syntax not pinned down.
const LOOKUP = build_table(256)   # pure: evaluated at compile time

fn build_table(n: Int32) -> Array[Int32] { ... }   # no capabilities
```

Because every top-level binding in Tel is already `const` (see
[Modules](../11-modules-and-packages/01-modules.md)), and Tel functions are
explicit about the capabilities they require, the compiler can tell which
expressions are safe to fold ahead of time.

TODO(open): how much compile-time evaluation is exposed as a deliberate
*feature* (an explicit "evaluate this in a const context" form, as some
languages offer) versus left as an invisible compiler optimisation. The
question runs both ways: "run funcs in a const context and fail if they do
anything non-const?"
and treat const-folding as a plain optimisation elsewhere. Lean: keep it mostly
an optimisation; if an explicit const-context marker is added it must stay
small. [`03-features.md`](../02-philosophy/03-features.md) lists compile-time
evaluation of pure functions only as *tentative* — do not over-commit.

## Why it fits Tel

- **No new runtime surface.** It runs *existing* Tel with *existing* semantics;
  it does not add a macro language or a metaprogramming API. Nothing new for
  host implementations to reproduce — beyond evaluating Tel, which they already
  do.
- **Safe by construction.** Only capability-free, pure code is eligible, so
  compile-time evaluation cannot perform I/O or observe the clock. This keeps
  builds reproducible.
- **Faster runs without low-level tricks.** It moves work from run time to
  build time without exposing anything machine-level — consistent with *high
  abstraction over low-level control*.

## Caching compile-time work

For fast incremental compiles, the results of compile-time evaluation should be
**cacheable**. That only works if the evaluated code is genuinely
side-effect-free — which is exactly what restricting evaluation to pure,
capability-free functions guarantees. Three caching modes are worth
distinguishing: none, persistent, and this-compile-only.

TODO(open): caching policy for compile-time evaluation — which mode applies
where, and whether it is per-operation or decided more cleverly. This is partly
an implementation concern; keep the user-facing rule simple ("pure const
expressions may be folded and cached").

## Not in scope: PGO and optimisation hints

One rejected idea is making **profile-guided-optimisation conclusions part of
the language** — source-level constructs for *hot call*, *likely branch*,
*monomorphise these types* (with a dynamic fallback), or *force
monomorphisation*.

**Tel rejects these.** They are explicitly excluded by the
[antifeatures](../02-philosophy/04-antifeatures.md): *no low-level machine
access … no PGO hints.* The reasons:

- **High abstraction over low-level control.** PGO hints are the programmer
  reasoning about code generation and branch prediction — exactly the
  machine-level thinking Tel keeps out of user code.
- **One script, many hosts.** A "monomorphise this" hint assumes one
  compilation model. Tel runs on interpreters, JITs, and AOT compilers to many
  targets; a hint that helps one can be meaningless or harmful on another.
- **Stability.** Performance-tuning constructs would be frozen into a language
  meant to outlive the hardware assumptions behind them.

TODO(open): there is genuine demand for some of this ("sometimes makes sense …
specific user knowledge"). The philosophy is unambiguous that it does not
belong in Tel — but the philosophy does not say where such tuning *should*
live. The intended answer: optimisation is the host runtime's and compiler's
job, informed by its own profiling, never by source-level hints. Flag as a
philosophy gap if a stronger statement is wanted.

### The one related construct: "do not optimise away"

A separate request is a way to mark a block as **not to be optimised
away** — for security (zeroing a buffer holding a secret) and for benchmarking
(so the measured work is not eliminated). This is *not* a performance hint: it
constrains the compiler from removing observable-by-intent work, rather than
asking it to go faster.

TODO(open): decide whether Tel has a "do not optimise away" / "keep this"
marker. It is defensible (security-relevant, not machine-level control) and
distinct from the rejected PGO hints, but it is a niche feature for an embedded
scripting language where secret-zeroing is usually the host's concern. Lean:
probably out of Tel1 — the host owns memory and security-sensitive cleanup —
but record the request. If kept, it would be an [attribute](03-derive-and-attributes.md).

## See also

- [Macros](01-macros.md) — the broader, mostly-rejecting metaprogramming
  stance.
- [Derive and Attributes](03-derive-and-attributes.md) — the attribute syntax a
  "keep this" marker would reuse.
- [Antifeatures](../02-philosophy/04-antifeatures.md) — the formal exclusion of
  PGO hints and low-level control.
