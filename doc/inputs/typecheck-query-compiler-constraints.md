# Type checking and inference: constraints from the query compiler

Tel's compiler is planned as a query compiler with content-addressed caching:
every compilation step is a query with a stable cache key, results are shared
across runs and machines, and incremental recompiles follow dirty cones from
changed files. The architecture is worked out and validated in the sandbox —
see [keys-and-invalidation.md](../../tel/docs/keys-and-invalidation.md)
(the three-identifier model, content keys, early cutoff) and its companion
docs. That architecture is not free: it imposes concrete constraints on how
type checking and inference may be *specified*. This document collects those
constraints so language-design decisions (TIPs, open questions) can be checked
against them before they harden.

The companion perspective: where
[fundamental-open-questions.md](fundamental-open-questions.md) asks what the
type system should *do*, this asks what it must *avoid requiring* so that the
compiler stays a query compiler.

## The architecture in three sentences

Every query (e.g. "type-check function F", "solve goal `X: Y`") has a **content
key** — a hash of its stable arguments plus the answer fingerprints of its
*direct* dependencies — and the cache maps content keys to answers, forever,
with no invalidation. A cached answer is valid *by construction*: if any
transitive input had changed, the key would differ and the lookup would miss.
Early cutoff falls out: a change whose intermediate answer is byte-identical
(formatting edit → same AST; body edit → same signature) stops propagating at
that level.

## The load-bearing rule: dependency discovery

To build a query's key you need its dependencies' fingerprints *before running
the query body*. The sandbox design therefore requires every query's dependency
list to have a **fixed shape with dynamic contents**, discoverable in two tiers
(see "How a Query's Dependencies Are Discovered" in
[keys-and-invalidation.md](../../tel/docs/keys-and-invalidation.md)):

1. **Structural deps** — a pure function of the query's own arguments ("check F"
   needs the resolve of F's file; the file is in F's fully-qualified name).
2. **Answer-derived deps** — read off an already-obtained answer (a file's
   imports are read off its parse answer; a trait goal's subgoals are read off
   the impl index's answer).

A dependency discoverable *only by executing the body* may not be a key
ingredient — it must move into the **answer** (as follow-up work), or the query
must be **split** until the rule holds. The fallbacks for queries that cannot
be decomposed (salsa-style verified traces, trace-tries) forfeit the
shared-across-machines cache or complicate the store substantially; reaching
for them is a design smell, not an option.

Everything below is this rule, applied.

## Constraints

### C1. Public signatures are explicit; inference never crosses item boundaries

**Status: adopted** —
[type inference](../book/src/05-types/08-type-inference.md) commits to local
inference with explicit public signatures.

This is *the* enabling decision, and it must survive future proposals. The
signature is the firewall between "F's body" and "everyone who calls F": the
query "signature of F" answers from F's declaration alone, so a body edit
leaves the signature answer byte-identical → unchanged fingerprint → no caller
re-checks. Any feature that lets a body influence an exported type — return
type inference on public functions, cross-function Hindley–Milner, public
items whose type "falls out" of usage — collapses the unit of recompilation
from "the item" toward "the strongly-connected program", and with it the entire
benefit of symbol-level caching promised in
[the compiler chapter](../book/src/18-tooling/01-compiler.md).

### C2. Items must be checkable independently, in any order

"Check F" may depend on other items' *signatures* and on impl indexes, never on
the *checking* of other bodies, and never on declaration order or any mutable
checking-session state. If checking F and checking G in either order can give
different answers, the answers are not a function of their keys and the cache
is unsound. Corollary: diagnostics are part of the answer, so they must be
deterministic too — no "first error wins" across items, no error texts that
depend on visitation order.

### C3. Trait solving must decompose into goal queries plus an impl index

The naive worry — "solving `X: Y` against `impl[T: Z] Y for T` discovers the
dependency on `Z` only mid-inference" — dissolves under decomposition, and the
language must keep it dissolvable:

```
SolveGoal(X: Y)
  structural dep:   ImplsOf(Y)        — the index query; address derivable
                                        from the goal itself
  answer-derived:   SolveGoal(X: Z)   — one subgoal per where-clause of each
                                        candidate impl that unifies with X;
                                        the where-clause is *data* in the
                                        ImplsOf(Y) answer
```

The where-clause `T: Z` is syntax in an impl declaration — part of a lower
answer — not something conjured by the solver. What this requires of the
language: bounds and where-clauses must be **declared, finite syntax on the
impl**, so they can be read off an answer. Anything that makes candidate
applicability depend on information outside the declared clause set (checking
the candidate's *body*, global solver state, "whatever unifies after arbitrary
computation") breaks the decomposition.

### C4. The impl set is an index query; absence must be materialized

"Does any impl of Y for X exist?" and "no impl exists" (negative reasoning)
must both go through an **index answer** — `ImplsOf(Y)` as a real query whose
answer lists the impls. Absence cannot fingerprint itself; the fingerprint of
the *set* can. Then a goal that failed re-solves the moment an impl appears
(the index answer changed), and one that succeeded re-solves when an impl
disappears — both directions ride the same fingerprint.

To keep the index from depending on the whole world, it aggregates per-file
projections (the firewall pattern):

```
ImplsDeclaredIn(file)  — read off the file's resolve answer; body edits leave
                         it identical → cutoff at the file
ImplsOf(Y)             — aggregates the projections; an impl of an unrelated
                         trait re-aggregates but yields an identical answer
                         → cutoff again
```

Language constraint hiding in there: **impl declarations must be syntactically
recognizable per file** — discoverable from the resolved AST without type
checking. Which leads to:

### C5. Coherence and the orphan rule are load-bearing for caching

**Status: adopted** —
[TIP-0005](../book/src/tips/0005-trait-coherence-and-the-orphan-rule.md) and
[traits-or-interfaces](../book/src/10-data-modelling/03-traits-or-interfaces.md).

Usually argued from semantics (which impl wins must be unambiguous), the orphan
rule is *also* a caching feature: it bounds where impls of a trait may legally
live, shrinking `ImplsOf`'s fan-in from "every file in every dependency" to a
statically known set of crates/files. Crate-local specialisation (the adopted
stance) matters the same way: "most specific impl wins" is negative reasoning
("no more specific impl exists"), and confining it to one crate confines the
index dependency that captures it. Any future loosening of coherence should be
priced in re-check fan-out, not just semantic ambiguity.

### C6. Answers must be deterministic and canonical

Everything entering a fingerprint goes through stable hashing
([deterministic-hashing.md](../../tel/docs/deterministic-hashing.md)); the
type checker must produce answers that *can* be stably hashed:

- **Canonical solutions.** Fresh inference variables, numbering of
  intermediate unification steps, or any counter state must not leak into the
  answer — two runs that infer the same type must emit the identical
  representation.
- **No iteration-order dependence.** If inference visits constraints from a
  hash map, the *answer* (including which of several possible errors is
  reported) must not depend on that order.
- **Deterministic errors are answers; transient failures are not.** A type
  error is cached like a success. A solver timeout or resource exhaustion is
  *not an answer* and must never be cached (see C9/C10) — the distinction must
  be representable.

### C7. Monomorphisation must have a bounded, deterministic instance set

Generic instantiation as a query — `Mono(F, type-args)` — needs the set of
demanded instances to be finite and derivable: each instance's callees at
their instantiated types come out of *its own answer* (worked out in the
sandbox: the `needed` list). The language-side constraint is termination
policy: unrestricted polymorphic recursion generates unbounded instance sets.
Tel needs an explicit rule (reject it, or a fixed depth limit that is part of
the language spec — a limit that varies is a nondeterministic answer).

### C8. Compile-time evaluation must itself be a well-behaved query

The value-dependence direction (const generics, "values as types" — open
question #1 in [fundamental-open-questions.md](fundamental-open-questions.md))
pulls evaluation into type checking: if types depend on computed values,
`Eval(const-expr)` becomes a dependency of type-checking queries. That is
workable *only if* CTFE is pure (no IO, no ambient state), deterministic
(bit-identical results across machines — beware floats and platform-width
integers), and resource-bounded with the bound in the spec (an evaluation that
diverges must become a deterministic error, identically, everywhere). The
[compile-time evaluation chapter](../book/src/15-metaprogramming/04-compile-time-evaluation.md)
should be checked against exactly this list when it firms up. The same goes
for metaprogramming generally: macro expansion must be a deterministic query
whose answer downstream queries (including the impl index — see C4) can read;
a macro that can generate impls must expand in a phase *below* impl indexing.

### C9. Refined types and proof obligations: solver results as answers

[Refined types](../book/src/05-types/12-refined-types.md) imply discharge of
proof obligations, plausibly via an SMT solver. For the query model each
obligation's verdict is an answer, which requires: the solver pinned as part
of the compiler's schema version (a solver upgrade cold-starts those cache
entries — acceptable; a solver that answers differently under one version —
not), verdicts independent of resource luck (a timeout is transient and
uncacheable, which in practice pushes toward *deterministic, syntactic*
refinement checking rather than "whatever Z3 manages in 200ms"), and
obligations keyed per item so they ride the same signature firewall as C1.

### C10. Core answers carry no presentation metadata

Early cutoff works only if semantically-identical answers are byte-identical.
Source spans, doc strings, or formatting-sensitive data inside the checked
AST or inferred types would make every whitespace edit defeat cutoff. The
sandbox's stance (fast mode: metadata in sidecar queries, demanded only when
diagnostics are rendered — see
[fast-mode.md](../../tel/sandbox/plans/fast-mode.md)) should be assumed by
any language feature that wants rich diagnostics: the *fact* of an error is in
the core answer; its *presentation* is derived on demand.

## The checklist

When evaluating a language feature (TIP or chapter decision) for query-compiler
fit, ask, in order:

1. **What is the query?** Can the feature's checking be phrased as
   `(kind, args) → answer` at item granularity or finer?
2. **What is in the key?** Which direct dependency answers does it consume —
   and is that list discoverable from the args plus lower answers (the
   two-tier rule), without running the body?
3. **Is anything discovered mid-body?** If yes: can it move into the answer
   (follow-up work), or can the query be split? If neither — the feature is
   asking for verified traces; redesign it.
4. **Does it reason about absence?** Then it needs an index query whose answer
   materializes the set, plus a bound (coherence-style) on where members of
   the set may live.
5. **Is the answer canonical?** Deterministic across machines, iteration
   orders, and solver moods; errors included; nothing transient cacheable.
6. **What edit granularity cuts off?** Trace a body edit, a signature edit,
   and an impl addition through the feature: what minimal set re-checks? If
   the answer is "the crate" or "the program", the feature has hidden global
   coupling — find it.
