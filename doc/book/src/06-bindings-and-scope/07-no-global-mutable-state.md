# No Global Mutable State

<!-- TODO: review -->

Tel has **no mutable global variables**. The top level of a file holds only
constants (see [Constants](03-constants.md)); there is nowhere to put state
that any function can quietly read or write. Cross-cutting values that would
otherwise be globals are passed explicitly, bundled where convenient into a
`Context` value.

## What

- Every top-level binding is an immutable constant. There is no top-level
  `uniq`.
- A function's inputs are its parameters (plus any capabilities the host
  injected). It cannot reach a hidden ambient variable.
- A long-lived mutable value (a cache, a counter, a logger sink) is owned by
  the host, or threaded through the program explicitly, never parked in a
  global.

This is consistent with the rest of the embedding story: I/O, the clock, and
randomness are already capabilities the host hands in
([antifeatures](../02-philosophy/04-antifeatures.md)). Ambient *mutable state*
would be the same anti-pattern wearing different clothes.

## The `Context`

Threading a value (a logger, a config, a tracing scope) through every call by
hand is noise — but Tel pays that price deliberately. Tel's answer is a
`Context`: a value that originates in `main` and is **declared in the signature
and passed explicitly** by every function it flows through, exactly like any
other argument. There is no implicit forwarding; the compiler never supplies a
`ctx` the author did not write (see [why explicit](#why-explicit-not-implicit)).

The rule:

- **Declared explicitly.** Any function through which the context flows names it
  in its signature. If `ctx` reaches a function — whether the function uses it
  or only forwards it to a callee — it appears in that function's type. There is
  no function that secretly carries a context it did not declare.
- **Passed explicitly.** Every call that forwards `ctx` writes it: `f(ctx, …)`.
  A function that does not need it does not take it, and you can see that at the
  call. The cost is boilerplate; the payoff is that what a call hands its callee
  is always visible at the call site.

```tel
fn main(ctx: Context) {
    handle_request(ctx)          # passed explicitly
}

# declares ctx because it flows through, even though it only forwards it
fn handle_request(ctx: Context) {
    validate()                   # validate takes no ctx, so none is passed
    write_result(ctx)            # write_result needs ctx; we hand it over
}

fn validate() {                  # genuinely context-free: not in its signature
    # ...
}

fn write_result(ctx: Context) {
    ctx.log.info("done")         # uses the part of ctx it asked for
}
```

The effect: a logger or trace scope reaches wherever it is wanted, yet it is
**not** a global — it has a clear origin (`main`), it cannot be mutated into a
shared back-channel, **every function that participates says so in its
signature**, and **every call that forwards it shows it.** You understand what a
function can touch, and what it hands onward, from the source alone; local
reasoning holds.

`Context` is therefore an ordinary value and an ordinary parameter, not a
language mechanism. Tel deliberately does *not* adopt the Scala 3 `using`/`given`
or Kotlin context-parameter model where the compiler forwards it for you — see
[Antifeatures: no implicit or contextual parameter passing](../02-philosophy/04-antifeatures.md#inheritance-and-dynamism)
and [why explicit](#why-explicit-not-implicit) below.

### Why explicit, not implicit

Two looser designs were considered and rejected, in order of how much they hide.

**Invisible context** — intermediate functions omit `ctx` from their signatures
entirely, yet it still flows *through* them. Rejected outright: it is dynamic
scoping by another name (what a function sees depends on its caller, not its
signature), it breaks the "understand a function from its signature" goal this
page rests on, and it does not survive generics. Consider:

```tel
fn map(xs: List[A], f: fn(A) -> B) -> List[B]
```

Does `f` need a context? With an invisible context there is no way to say, and
no way to type the "needs it" and "doesn't" cases apart — so either *every*
callback silently receives it (a global again) or callbacks can never reach it.

**Implicit passing** — `ctx` is *declared* in every signature (so the generics
problem above is solved: "needs it" lives in the type) but the compiler
*forwards* it at call sites so you never write it. This is the Scala
`using`/`given` and Kotlin context-parameter model. It was the working proposal
for a while, and it is more honest than invisible context — but it too was
rejected. It reintroduces the one bit of action-at-a-distance this page exists
to remove: the compiler hands a callee an argument the author never wrote, so
what a call passes stops being visible at the call. And every language that
shipped it reports the *resolution rules* — which `Context` is in scope, what
happens when two are, shadowing, import-driven "magic" — as a headline regret
(Scala 3 split the overloaded `implicit`; Kotlin deprecated context receivers).

Tel takes the **explicit** branch. With explicit passing the generic case needs
no special clause at all — a callback that wants the context simply takes it:

```tel
fn map(xs: List[A], f: fn(Context, A) -> B) -> List[B]
```

The boilerplate is real, paid down by bundling capabilities into one `Context`
value (Go's `context.Context`), and the visibility is worth it. Full rationale:
[Antifeatures](../02-philosophy/04-antifeatures.md#inheritance-and-dynamism).

TODO(open): the `Context` design is still loosely specified. Remaining points:

- Is `Context` one host-supplied type, a bag of capabilities, or just a record
  the script defines? It overlaps with the capability model — the capabilities
  the host injects (`Clock`, `Log`) might *be* the context or live inside it.
- Is the context immutable? If a logger inside it buffers, that is mutation —
  how does that square with "no mutable globals"? Likely: the context holds
  *capabilities* (host-owned effectful handles), not Tel-mutable data.
- Closures and host callbacks: a closure that needs the context captures the
  `ctx` in scope at its definition (ordinary lexical capture — no different from
  capturing any other value), so a callback the host invokes later carries the
  context it was built with. Confirm this against
  [concurrency](../14-concurrency-and-parallelism/) (scoped values) and the
  philosophy gap noted below — `scoped values` is itself a dynamically-scoped
  context, so check it does not quietly reintroduce implicit passing.

TODO(open): philosophy gap — [`02-philosophy/04-antifeatures.md`](../02-philosophy/04-antifeatures.md)
lists "global mutable state" as an open question (forbid outright vs. allow).
This topic assumes the *forbid* answer ("No global state", "everything
top-level is const"). Confirm in the
philosophy chapter.

## Why

- **Embedding demands it.** Multiple Tel scripts may run inside one host,
  possibly concurrently. A mutable global is shared state across all of them —
  a correctness and security hazard the host cannot police.
- **Reproducibility.** No hidden global means a function's result depends only
  on its inputs (and explicitly injected capabilities). Tests and repeated runs
  behave identically.
- **Data-race safety.** Removing ambient mutable state removes the main way two
  tasks could race without either of them naming a shared value.
- **Local reasoning.** What a function can read and affect is exactly what its
  signature shows. This is the unifying goal: you understand a function from
  its body and signature, never by hunting for some far-away variable that
  might change underneath it. Several features serve the same end — transitive
  immutability and the lack of interior-mutability hatches
  ([mutability](02-mutability.md)), per-task heap isolation
  ([concurrency](../14-concurrency-and-parallelism/07-memory-model-for-concurrency.md)),
  and encapsulation through visibility and opaque types
  ([visibility](../11-modules-and-packages/03-visibility.md)). No global
  mutable state is the piece that removes *action at a distance*: nothing
  mutates behind a call that did not name the value.

## Bugs the no-globals rule prevents

A few catalogue cases that drive the
strict no-globals stance:

- **"Class loader weirdness when moving from executor service to parallel
  stream."** Code that worked under one runtime model failed under
  another because some implicit global (the thread-context class loader)
  changed. Tel has no class loader and no thread-local-via-the-VM
  globals; the equivalent — a logger, a context — flows explicitly through
  the `Context` so the move from one runtime shape to another doesn't
  reach a different invisible value.
- **"Hazelcast `serialVersionUID` mismatch from a stray invisible client."**
  A separate process running in the background presented a different
  identity for the same class; the production process saw mismatched IDs
  it could not source. The same shape — *something I didn't explicitly
  ask for is participating in my computation* — is what the no-globals
  rule blocks.
- **"Feature toggle negation bug, hidden because the report didn't show
  the affected variable."** A feature thought to be off was on because
  of a sign flip; the report showed the raw value rather than the
  modified one, hiding the discrepancy. Tel cannot prevent a sign flip,
  but the no-ambient-power rule means every feature toggle is a
  *capability* or a *typed value passed in*, not a global flag set
  somewhere — and the closure/context approach to "feature-toggle wraps
  this block" (see
  [`../15-metaprogramming/01-macros.md`](../15-metaprogramming/01-macros.md))
  makes the active-toggle scope visible at the call site.
- **"Server config got reverted to several months ago because it was
  never committed."** Tel cannot police source control, but a per-script
  declared configuration (read from a typed config passed in by the
  host) is harder to lose than a config-by-side-effect on a running
  process.

## See also

- [Constants](03-constants.md) — why the top level is constant-only.
- [Mutability](02-mutability.md) — mutation stays small and local.
- [Antifeatures](../02-philosophy/04-antifeatures.md) — no ambient I/O, no
  global state.
