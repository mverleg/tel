# TIP-0003 — Open Questions: remaining

Nearly everything in this file was resolved by
[TIP-0003](../book/src/tips/0003-module-levels-and-dependency-direction.md)
(three levels module/crate/workspace and their names; non-transitive deps;
no parent packages with dotted lexical names; mandatory crate export block and
three-level visibility; namespaces as a separate axis, crate wins; dev-only
workspace members; version-conflict rule; major-bump backwards-compat
enforcement; and the one-rule api/impl flag — with the requested list of what
the flag buys over convention). Those bullets are removed.

One sub-question the TIP explicitly left open remains:

- The major-version backwards-compat check bumps major when calling code could
  stop compiling. Should it *also* cover **pre/postconditions**? Or is that too
  strict — because Tel checks conditions against *listed examples*, so even
  adding an example case could count as breaking. (If examples can't be removed,
  a non-major bump can't prune them — too strict; if they can, the contract's
  tested surface can shrink silently.) Depends on what
  [TIP-0004](../book/src/tips/0004-how-far-refinement-types-go.md) settles for
  the contract/example model.
