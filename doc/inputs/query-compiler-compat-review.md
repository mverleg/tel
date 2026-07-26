# Query-compiler compatibility review — flagged issues

**Date:** 2026-07-06.
**Scope:** the book chapters and TIPs, checked against
[typecheck-query-compiler-constraints.md](typecheck-query-compiler-constraints.md)
(C1–C10 and the six-question checklist).
**Verdict in one line:** the adopted core (explicit public signatures, local
inference, coherence + orphan rule, same-crate specialisation, no macros, no
SMT) is compatible; the open edges that were **not** have since been resolved
— inferred effects and script-mode inference via the file-boundary rule
(Issues 1, 6), coinductive capability derivation (Issue 2), rejection of
polymorphic recursion (Issue 3), and the compiler chapter rewritten off Salsa
(Issue 5). Still open: the CTFE chapter's C8 requirements (Issue 4) and the
minor notes (Issues 7–8). Cross-machine cache sharing itself is deferred to
the appendix deferred-features list, with "no machine-local identity in keys
or answers" as the one standing guard-rail.

---

## Issue 1 — Inferred effects are a body-leaks-into-signature feature (C1) **[resolved 2026-07-07]**

**Resolved.** Decision: inference — for types *and* effects — never crosses a
file boundary. Anything visible outside its file has an explicit signature:
types written, effects declared or defaulted to may-panic/may-allocate.
Rationale recorded in the chapter: an effect is mechanically an injected
ambient-capability parameter, i.e. a signature ingredient, so it follows the
same explicitness rule as types. Which effects are opt-in vs opt-out is
deliberately unchanged (purity stays opt-in; the cross-file default is the
worst-case row, so ordinary code still writes nothing). Applied in
`05-types/08-type-inference.md` and `05-types/05-function-types.md`. The
`total`-termination-check and dyn-boundary notes below remain open.

<details><summary>Original analysis</summary>

[`05-types/05-function-types.md`](../book/src/05-types/05-function-types.md)
§"Effects belong on the function type": effects (`panics`, `allocates`,
`pure`/`total`) "flow through function types and are **mostly inferred** for
concrete functions", and "a concrete function that calls a `panics` function is
itself `panics`."

If the effect row is part of a function's externally-visible type *and* is
inferred transitively from bodies, then:

- **C1 is violated as stated.** A body edit (add a call to a panicking
  function) changes the effective signature of `F`, of `F`'s callers, and so on
  up. "Signature of F" no longer answers from F's declaration alone; every
  public signature silently depends on the bodies of everything it transitively
  calls, and the signature firewall — the enabling decision of the whole
  architecture — is gone for the effect component.
- **Recursion makes the query cyclic.** `EffectOf(F)` for mutually recursive
  functions needs a fixpoint over the SCC. The query model has no cycles; an
  SCC-level fixpoint query is possible but must be specified (canonical SCC
  identification, deterministic solution), and it makes the *SCC*, not the
  item, the unit of recompilation — checklist question 6 fails with "the
  strongly-connected call graph".

**Suggested resolution.** Extend "public signatures are explicit" to the effect
row: effects on **exported/public functions are declared** (or default-assumed,
e.g. "may panic, may allocate" unless marked), and inference runs only where a
firewall already exists — inside a body, or crate-internally with the crate's
public API as the explicit boundary. The chapter's own `TODO(open):` about a
crate-level **"auto"** effect mode is essentially this fix and should be
promoted rather than parked. The other open `TODO(open):` ("are effects just
plain inferred function properties?") should be answered **with C1 as an
input**: "inferred property visible to callers" is exactly the thing C1
forbids on public items.

Related, same section:

- `total` ("terminating on all inputs") needs a **deterministic, syntactic**
  termination criterion. Any fuel/timeout-based check is a transient result and
  uncacheable (C6, same argument as C9's solver-timeout point).
- The dyn-dispatch `TODO(open):` lean ("trait bounds state their effect set
  explicitly; inferred effects only behind static dispatch") is the
  cache-compatible answer — adopt it, and note the caching rationale.
- The `FnOnce`-inferred-from-captures rule is body-local and fine; it surfaces
  in signatures only where declared. No issue.

</details>

## Issue 2 — Structurally derived capabilities need a cycle rule (C2, C6) **[resolved 2026-07-07]**

**Resolved.** Committed to coinductive derivation (assume the capability on
the cycle, look for a refuting field), resolved per cluster of mutually
referencing type definitions with a deterministic outcome. Documented in
`12-memory-and-runtime/08-substructural-types.md` §"Recursive types". The
query-mechanics note below (one `CapOf(SCC)` query keyed by the canonical SCC;
the condensation keeps the query graph acyclic) remains the implementation
guidance.

[`10-data-modelling/03-traits-or-interfaces.md`](../book/src/10-data-modelling/03-traits-or-interfaces.md)
§"Auto-traits" and
[`12-memory-and-runtime/08-substructural-types.md`](../book/src/12-memory-and-runtime/08-substructural-types.md):
`Alias` / `Discard` / `Send` / `Sync` / `Unpack` are **derived from a type's
fields**, not declared; unions derive relevance as the meet of their members.

As queries this is fine in shape — `CapOf(T)` has structural deps (T's own
declaration) and answer-derived deps (the field types, read off T's resolve
answer), so it does **not** need the ImplsOf index (derived capabilities are
not declared impls and never live in a foreign file). But **recursive type
definitions make `CapOf` cyclic** (`struct Node { next: Option[Node], ... }`),
and nothing in the docs states the fixpoint rule. Rust solves this with
coinductive auto-traits; whatever Tel picks, the answer for a recursive SCC of
types must be canonical and order-independent (C2/C6), and the rule belongs in
the spec, not the implementation.

**Suggested resolution.** Add to the substructural chapter: derivation is
coinductive (assume the capability on the cycle, look for a refuting field),
per-SCC, with a deterministic canonical answer. Also confirm the explicit
opt-out marker is *declared syntax on the type* (it is, per the traits
chapter) so it stays discoverable from the resolve answer.

## Issue 3 — No termination policy for monomorphisation (C7) **[resolved 2026-07-07]**

**Resolved.** Polymorphic recursion is rejected: inside a recursive call
cycle, calls to cycle members must pass the caller's own type parameters
unchanged or fully concrete types. Checked at type-checking time at the
offending call site; `dyn Trait` is the escape hatch for type-deepening
algorithms. Full worked rationale (why ordinary recursion is finite, the
`Nested[T]`/`depth` counterexample, why reject beats a depth limit, the
Haskell/Rust prior art) is now in `05-types/07-generics.md`
§"Monomorphisation terminates".

C7 requires an explicit language rule for polymorphic recursion (reject it, or
a fixed depth limit that is part of the spec).
[`05-types/07-generics.md`](../book/src/05-types/07-generics.md) commits to
"generic functions stay simple" and rejects HKTs/variadics, but nowhere in the
book is polymorphic recursion ruled on. Without a rule, `Mono(F, args)` can
demand an unbounded instance set, and a host-varying recursion limit is a
nondeterministic answer.

**Suggested resolution.** Add a `TODO(open):`-turned-decision in
`07-generics.md`: lean **reject polymorphic recursion** (a generic call inside
a generic body must instantiate at the caller's own parameters or at concrete
types). Rejection is also the frozen-language-friendly answer — a depth limit
is a magic number in the spec forever.

## Issue 4 — CTFE chapter doesn't state C8's requirements (C8) **[major if const generics land, medium otherwise]**

[`15-metaprogramming/04-compile-time-evaluation.md`](../book/src/15-metaprogramming/04-compile-time-evaluation.md)
states purity ("no capabilities") but **not**:

- **Bit-identical determinism across machines** — floats and anything
  platform-width. (Tel's explicit-width integers help; float const-folding
  needs an explicit "IEEE 754, one rounding mode, no FMA/x87 drift" rule.)
- **A resource bound in the language spec**, with divergence/exhaustion
  becoming a *deterministic error, identically everywhere*. The existing
  resource-bounded execution mode in
  [`18-tooling/01-compiler.md`](../book/src/18-tooling/01-compiler.md) is a
  host-facing runtime feature with `TODO(open)` op-accounting — it cannot be
  the CTFE bound, because a limit that varies by host makes the same program
  compile differently on different hosts (the exact failure C9 forbids for
  solvers).

While CTFE is only a folding *optimisation* the stakes are low (the fold must
equal the runtime result anyway). The moment **const generics** (open question
#1, `07-generics.md`) or any value-in-type feature lands, `Eval(const-expr)`
becomes a dependency of type-checking queries and all of the above becomes
load-bearing. The two decisions are currently not linked anywhere.

**Suggested resolution.** Add the C8 checklist to the CTFE chapter now (pure ✓,
deterministic across machines, spec'd bound, divergence = deterministic error),
and cross-reference it from the const-generics `TODO(open):` in
`07-generics.md` as a hard precondition. The chapter's three-mode caching
`TODO(open):` collapses under the query model — an `Eval` result is a cached
query answer like any other; only genuine answers (values and deterministic
errors) are cacheable.

## Issue 5 — Compiler chapter describes the pre-query architecture **[resolved 2026-07-06]**

[`18-tooling/01-compiler.md`](../book/src/18-tooling/01-compiler.md) committed
to incremental compilation "in the style of Salsa", which the constraints doc
treats as the fallback that forfeits the shared cache. **Fixed:** the
incremental-compilation paragraph now describes the content-key query model
(keys from args + direct-dependency fingerprints, validity by construction,
cross-machine sharing, early cutoff) and states the C10 rule that core answers
carry no presentation metadata, with spans/formatting in sidecar queries.
Remaining nit, not applied: note somewhere that friendly mode's
"continues past the first error" is a scheduling difference (which queries get
demanded) and per-item diagnostics must be identical in both modes (C2).

## Issue 6 — Script-mode inference + cross-file top-level bindings recreates cross-item inference (C1) **[resolved 2026-07-07 by the Issue 1 rule]**

**Resolved.** The file-boundary rule covers this: a single-file script infers
everything with no special case; cross-file-visible items need signatures.
The `08-type-inference.md` TODO is rewritten accordingly; TODO.md's cross-file
top-level-binding question stays open but is now constrained (an inferred
binding cannot be cross-file visible).

<details><summary>Original analysis</summary>

Two open items compose badly:

- [`05-types/08-type-inference.md`](../book/src/05-types/08-type-inference.md)
  `TODO(open):` "type annotations fully optional for end users (scripts)".
- [`TODO.md`](TODO.md) §"Cross-file access to top-level `let`/`var`".

If a script's top-level bindings have *inferred* types **and** are visible
from other files, then "signature of binding B" is inferred from B's
initialiser expression — a body — and inference crosses an item boundary that
other files depend on. That is the C1 collapse in miniature, confined to the
script module. The inference chapter's own reconciliation sketch ("a top-level
script has no public surface, so everything in it is effectively local") only
holds if cross-file visibility is **off**.

**Suggested resolution.** The query model adds a concrete argument for
TODO.md's option **(a)** (top-level bindings file-private) or (b) restricted to
explicitly-typed constants. Record it in both TODOs so the decision is made
with this input.

</details>

## Issue 7 — Host-injected AST transforms are an un-keyed compiler input **[minor, only if adopted]**

[`15-metaprogramming/01-macros.md`](../book/src/15-metaprogramming/01-macros.md)
Alternative 2 (host-injected AST transforms, open). Under the query model a
transform is an input to every parse answer, so its identity/version must be
part of the parse query's content key — otherwise a cached answer is not valid
by construction. Consequences to price in: hosts with different transforms
share no cache below parse, and a transform must itself be content-addressable
(a hash of its definition, not "the host had a plugin"). Add this to the
existing `TODO(open):` as an adoption cost. (Everything else in the macros
chapter is *good* news for the query model: no macros, no expansion phase,
`derive`/newtype-inheritance impls are syntactically visible on declarations,
so per-file impl projection — C4's `ImplsDeclaredIn(file)` — works off the
resolved AST. The one wrinkle: a `derive Ordinal` through a **bound alias**
means the per-file projection needs alias resolution before it knows which
trait indexes gain an impl — a resolve-answer dependency, allowed, but worth a
line in the impl-index design.)

## Issue 8 — Small confirmations and notes **[minor]**

- **`for`-loop dispatch over the two iterator protocols**
  ([linear-iterator-two-protocols.md](linear-iterator-two-protocols.md)):
  "dispatch by which `next` exists on the type" is method-existence
  (negative-reasoning) probing; "by the trait the type implements" rides the
  C4 impl index cleanly. The file's lean (by trait) is the cache-friendly one —
  one more reason to confirm it.
- **Union shared-interface auto-exposure**
  ([`05-types/01-type-system-overview.md`](../book/src/05-types/01-type-system-overview.md)):
  "any field, method, or trait present on **every** member is available on the
  union" is per-goal set-intersection over the members' method/impl surfaces.
  Compatible, but it must be phrased over **index answers** (a member gaining
  or losing an impl changes the union's surface — both directions must ride a
  fingerprint, C4). Fine as long as trait availability goes through `ImplsOf`
  and method availability through per-file resolve answers.
- **TIP-0004 / refined types**: already the right call for C9 — SMT rejected
  on determinism grounds; flow-narrowing (2a) is body-local and
  signature-refinements are declared syntax. Two additions: (1) the proposed
  philosophy antifeature "no solver/SMT in the typechecker" should cite cache
  soundness (a timeout is not an answer) alongside host determinism; (2) the
  option-(b) frozen propagation set, if shipped, changes *inferred result
  types* — i.e. answers — so the rule list must be versioned in the compiler's
  schema version (adding rule N+1 is a cache cold-start, another reason for
  b0/b1-tiny).
- **`schema_of`** ([`05-types/15-record-shape-calculus.md`](../book/src/05-types/15-record-shape-calculus.md))
  types an expression appearing in a *type alias* — a checking query whose deps
  (the named table/functions) are discoverable from the alias declaration's
  resolve answer. Compatible. The calculus itself (closed, first-order,
  monomorphic, no row polymorphism) is a model citizen for C6.
- **Effects-plumbing open question**
  ([fundamental-open-questions.md](fundamental-open-questions.md) #2): the
  const-generic/`comptime` plumbing variant multiplies monomorphisation
  instances (every function generic over its ambient set) — a C7 fan-out cost
  the runtime-value variant doesn't have. Add to the comparison.
- **tip3 leftover** ([tip3-open-questions.md](tip3-open-questions.md)): the
  major-bump check over pre/postconditions evaluates declared examples at
  publish time — if adopted, example evaluation inherits the C8 discipline
  (pure, deterministic, bounded), same as CTFE.
- **Specialisation** (same-crate + adjacent + most-specific-wins) and
  **TIP-0005 coherence**: exactly what C5 wants; the "adjacent, one owner"
  requirement even shrinks the index dependency below what C5 asks for. No
  action.
- **Negative trait bounds** (`TODO(open):` in the traits chapter, lean
  reject): rejection is also the C4-friendly answer — every `T: A + !B` goal
  adds an absence dependency on `ImplsOf(B)`, growing re-check fan-out. Add
  the caching note to the lean.

## Checklist coverage

| Constraint | Status in the book |
|---|---|
| C1 signatures explicit | Adopted for types; **violated by inferred effects** (Issue 1); script-mode edge (Issue 6) |
| C2 order-independent items | No violations found; needs the SCC rules of Issues 1–2 |
| C3 trait solving decomposes | OK — bounds/where-clauses are declared finite syntax; bound aliases are transparent synonyms |
| C4 impl set as index | OK — impls, derives, newtype-inheritance all syntactically per-file; alias-resolution note (Issue 7) |
| C5 coherence | Adopted (TIP-0005), specialisation same-crate — no action |
| C6 canonical answers | Mostly fine; `total`-checking and auto-trait fixpoints need rules (Issues 1–2) |
| C7 bounded mono | **No termination rule anywhere** (Issue 3); effects-as-const-generics fan-out note |
| C8 CTFE well-behaved | **Chapter missing the requirements** (Issue 4) |
| C9 solver verdicts | Satisfied by TIP-0004's SMT rejection — strengthen rationale with cache soundness |
| C10 no presentation metadata | Compatible with the two-mode split; compiler chapter should state it (Issue 5) |

TODO: review
