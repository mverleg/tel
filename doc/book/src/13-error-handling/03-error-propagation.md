# Error Propagation

When a function gets an `Err` from something it calls and cannot handle that
error itself, it **propagates** it: hands the same failure to its own caller.
Tel makes this terse so the happy path stays readable, and **hard to forget** so
a failure is never silently lost.

This is the error-handling view; the control-flow view is in
[`08-control-flow/07-error-propagation.md`](../08-control-flow/07-error-propagation.md).

## The propagation operator

A function that calls a [`Result`-returning](02-result-types.md) function has to
decide what to do with the result. When the decision is "I can't handle this,
my caller must," the propagation operator does it in one token:

- on `Ok(v)` it evaluates to the unwrapped `v` and execution continues;
- on `Err(e)` it performs an early [`return`](../08-control-flow/06-early-return.md)
  of `Err(e)` from the current function.

```tel
fn total_with_tax(order: Order, rates: RateTable) -> Result[EuroAmt, Reject] {
    let base = subtotal(order)?           # Err here -> return Err here
    let rate = rates.lookup(order.region)?
    Ok(base * (1 + rate))
}
```

Without the operator the same function is all staircase:

```tel
fn total_with_tax(order: Order, rates: RateTable) -> Result[EuroAmt, Reject] {
    let base = match subtotal(order) {
        Ok(b)  -> b,
        Err(e) -> return Err(e),
    }
    let rate = match rates.lookup(order.region) {
        Ok(r)  -> r,
        Err(e) -> return Err(e),
    }
    Ok(base * (1 + rate))
}
```

The operator collapses the boilerplate while keeping the propagation *visible* —
*error handling is explicit but terse*. The `?` spelling here is a placeholder;
the behaviour is the contract.

A propagating function's return type must itself be a `Result` whose error type
the propagated error fits — you can only forward an error out of a function that
is declared able to fail. Where the error types differ, the conversion is
explicit (a `map_err` or similar); Tel does not silently widen error types.

## Errors are hard to forget

The danger the design notes call out: with results-as-values it is easy to
*forget to handle* an outcome — especially when the happy path produces nothing,
so an ignored `Result[()]` looks like a complete statement. Exceptions do not
have this failure mode; plain return values do.

Tel's answer: **a `Result` is not freely discardable.** Ignoring one is a
deliberate, visible act, never the default. The maxim is *an error is never
dropped silently — discarding one is explicit*. Concretely, calling a
`Result`-returning function and doing nothing with the value should not compile;
the author must either handle it, propagate it (`?`), or explicitly discard it
with a named construct that says "yes, I mean to drop this."

This is not a special case: `Result` is a **relevant** type. Its `Err[E]`
variant is declared relevant regardless of the payload `E`, so by the
[union meet rule](../12-memory-and-runtime/08-substructural-types.md#unions-derive-relevance-as-the-meet-of-their-members)
every `Result` has a relevant member and never derives `Discard` — the
substructural rules that apply to every must-use value apply here too (see
[Substructural Types](../12-memory-and-runtime/08-substructural-types.md)). The
type system insists it is consumed exactly once rather than allowed to fall on
the floor. The same reasoning is why an un-awaited task handle is a compile
error — handles are relevant for the same reason.

Because relevance is a compile-time obligation on the normal path and Tel
[aborts rather than unwinds](04-panics-and-aborts.md), this carries no
"destructor must run on failure" cost: an abort drops the whole heap, including
unused `Result`s, without ceremony.

### Settling a `Result`: handle, propagate, or discard

There are exactly three ways to discharge a `Result`'s must-use obligation, and
the unifying rule is that each one **moves the value somewhere** — only letting
it fall on the floor (reach end of scope unused, or `let _ = …`, which *drops*
rather than moves) fails to compile:

1. **Handle it** — `match` both arms (or otherwise destructure it). Consuming
   the union *is* the use; this is where a `Result` is finally settled.
2. **Propagate it** — `?`. On `Ok` it unwraps and continues; on `Err` it
   early-returns the `Err` from the current function.
3. **Discard it** — a named `ignore(…)` / `.discard()` construct for the rare
   deliberate "I don't care about the outcome" case.

The subtlety is that **`?` does not settle anything — it relocates**. Because a
propagating function's own return type is a `Result`, and that return value is
itself relevant, each `?` discharges the obligation here only by minting an
identical one in the caller. The must-use-ness rides up the call stack
unchanged and must eventually **bottom out** in a place that does *not*
re-propagate: a function that `match`es both arms and acts, an entry boundary
(`main`, a spawned task body) with no further caller, or an explicit
`.discard()`.

So across a whole program the obligation is conserved: every `Result` that is
created is settled by exactly one **handle-or-discard**, with any number of `?`
hops in between merely passing it along. That is precisely what makes `?` terse
without being unsafe — it defers the real decision to a single well-chosen place
upstream instead of forcing one at every call site, and relevance guarantees the
chain can never just evaporate.

TODO(open): decide the explicit-discard spelling — the named "yes, drop this"
construct for the rare deliberate case (an `ignore(...)` function or a
`.discard()` method that is the `Result`'s sanctioned use). Not `let _ = …`:
a bare `_` *drops* a relevant value rather than moving it onward, so it is a
compile error, not a discard (see
[Substructural Types](../12-memory-and-runtime/08-substructural-types.md#destructuring-discharges-the-obligation-onto-the-parts)).

## Bugs the must-use rule prevents

A representative subset of catalogue cases
that drive making `Result` undroppable:

- **"Empty fit output got persisted as state for the next run."** A
  fitting process returned an empty result; the caller had a path for
  *missing* state but not for *empty* state; the empty result fed into
  the next iteration. A `Result`-shaped return (`Ok(state)` vs
  `Err(reason)`) makes the failure case structurally distinct from a
  successful-but-empty case; the must-use rule prevents the caller from
  treating the `Err` as `Ok(empty)`.
- **"Opval failed for every expiry but we still published fallback vols."**
  A validation step indicated every expiry had failed; downstream code
  persisted fallback values anyway because the validator's result was
  not threaded into the publish decision. A `Result`-returning validator
  whose result *must* be consumed forces the publisher to either handle
  the failure or propagate it.
- **"Viper export threw an exception that wasn't logged nor reported."**
  An exception travelled up to a thread that didn't have a handler, and
  vanished. Tel has no thrown exceptions; the only error-of-shape an
  ordinary call can produce is a `Result`, and that `Result` cannot be
  silently dropped.

## Propagation is control flow, not unwinding

Propagation looks a little like an exception travelling up the stack, but it is
not. Each `?` is an ordinary early `return` at *that* call site — there is no
unwinding, no handlers run on the way up, no hidden machinery. A function is
either written to forward errors (`Result` return type, `?` at call sites) or it
is not, and that is visible in its signature. See
[panics and aborts](04-panics-and-aborts.md) for the genuinely
non-propagating path.

TODO: review — new section; the discard-enforcement mechanism is the open call.
