# The Fallback Operator

The **fallback operator** supplies a default when a value is absent — `none` for
an [`Option`](../05-types/06-option-and-nullability.md) or `Err` for a
[`Result`](02-result-types.md).

This page is the error-handling view, and it is intentionally thin. The full
mechanics — laziness, chaining, the no-truthy/falsy rule, and the `??`-vs-`or`
spelling question — live in the expression-chapter topic
[Fallback Operator](../07-expressions/11-fallback-operator.md).

## Swallowing an error is explicit

Used on a `Result`, the operator falls back on `Err`: `parse(text) ?? 0`
discards the error and substitutes `0`. This **swallows** the failure rather
than forwarding it. Swallowing is a real decision, so Tel makes you write it —
the operator *is* that explicit act, which keeps it consistent with *an error is
never dropped silently*.

To forward the failure to the caller instead of replacing it, use
[error propagation](03-error-propagation.md). Propagation *moves* the error
outward; the fallback operator *ends* it and supplies a value in its place.

## See also

- [Fallback Operator](../07-expressions/11-fallback-operator.md) — mechanics,
  the no-falsy rule, chaining, and spelling.
- [Error Propagation](03-error-propagation.md) — the other half of the choice.
- [Result Types](02-result-types.md)
