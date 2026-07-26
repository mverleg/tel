# Error Propagation

Propagating an error — passing a failure from the function that detected it up
to a caller that can handle it — is a *control-flow* operation: it cuts the
current function short and hands an `Err` value back. Tel gives it terse,
explicit syntax so the happy path stays readable.

This page is the control-flow view. The full treatment — the propagation
operator, why it cannot be silently forgotten, and how it interacts with
`Result`-shaped types — lives in
[`13-error-handling/03-error-propagation.md`](../13-error-handling/03-error-propagation.md).

## The shape

A function that can fail returns a [`Result`-shaped
type](../13-error-handling/02-result-types.md). A caller has three honest
choices:

- **Handle it** — [`match`](02-match-expressions.md) on the result and deal with
  the `Err` case.
- **Propagate it** — if this function cannot handle the error, pass it straight
  to *its* caller. The propagation operator does this in one token: on `Err` it
  performs an early [`return`](06-early-return.md) of that error; on `Ok` it
  unwraps the value and execution continues.
- **Abort** — for a *can't-happen* error, convert it to an
  [abort](../13-error-handling/04-panics-and-aborts.md). This is not propagation;
  it ends the task.

```tel
fn total_with_tax(order: Order, rates: RateTable) -> Result[EuroAmt, Reject] {
    let base = subtotal(order)?          # on Err: return Err(...) right here
    let rate = rates.lookup(order.region)?
    Ok(base * (1 + rate))
}
```

The `?` here is a placeholder for whatever spelling Tel settles on. The
behaviour is the contract: it is an early return on the error path and a no-op
on the success path.

## Why explicit, not exceptions

Tel has [no exceptions and no *recovering*
unwinding](../13-error-handling/04-panics-and-aborts.md) — a panic aborts the
task and can only be contained at a task boundary, never caught mid-function.
Propagation is therefore a visible token in the source, not invisible control
flow. The cost is one mark per fallible call; the payoff is that *every* place
an error can leave a function is written down — *if it looks correct, it
probably is correct*.

What Tel must avoid is making propagation *forgettable*: ignoring a `Result` has
to be a deliberate, visible act, not the default. See
[`13-error-handling/03-error-propagation.md`](../13-error-handling/03-error-propagation.md)
for how `Result` values resist being silently dropped.

TODO: review — new section; this is intentionally a thin pointer to chapter 13.
