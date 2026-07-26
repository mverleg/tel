# TIP-0004: How Far Refinement Types Go

**Status:** Draft
**Created:** 2026-06-03
**Touches:** `05-types/12-refined-types.md`, `05-types/13-units.md`,
`05-types/01-type-system-overview.md`, `05-types/07-generics.md`,
`05-types/09-subtyping-and-variance.md`,
`02-philosophy/01-priorities.md`, `02-philosophy/03-features.md`,
`tips/0001-mutability-and-borrowing.md`
**Downstream of:** fundamental open question #2 (*how far the type system
reaches / value-dependence*).

## Summary

Refined types are a spectrum, not a switch. At the cheap end sit newtype
wrappers and construct-time predicates — used in production for decades. At the
expensive end sit SMT-backed refinement (LiquidHaskell, F\*) and full dependent
types (Idris, Agda), which buy more guarantees at a cost in compile time,
learnability, and *determinism across hosts*. This TIP surveys where other
languages have settled, asks what evidence we actually have about the sweet
spot, and proposes a tier line for **Tel1** — committing the cheap, decades-proven
end and deferring the solver-backed end.

It exists because `12-refined-types.md` leaves the central call (`TODO(open):`
how far constraint propagation goes — options **(a)/(b)/(c)**) open, and that
call cascades into generics, units, subtyping, and the mutability model.

## Recommended outcome (one-line summary)

- **Commit the bottom two rungs of the ladder for Tel1: newtypes and
  construct-time predicates** (option **(a)** in `12-refined-types.md`). These
  are the decades-proven, zero-learning-curve, high-ROI core.
- **Adopt three *scoped, frozen* refinement features** that have strong
  independent precedent: **units of measure** (own chapter), **flow-sensitive
  narrowing** of an already-checked predicate (`if user.is_admin { … }` refines
  `user` *inside the branch only*), and **subrange numeric types**
  (`Int64 in 1..=31`) as a construct-time predicate with no value-tracking through
  arithmetic.
- **Reject SMT-backed general refinement (option (c)) and dependent types for
  Tel1.** They violate three top priorities at once: *determinism across hosts*
  (solver results and timeouts vary by machine and version), *compile speed*,
  and *frozen + easy to learn*. This is a **philosophy-chapter gap** to close
  explicitly.
- **Hold option (b) — a small frozen set of propagation rules (non-zero divisor,
  range arithmetic) — as the one genuinely open call.** It is the only rung with
  *no clean precedent* (languages ship either (a) or full (c)), so it is the
  highest-risk thing to freeze at 1.0. Lean: ship the most-requested one or two
  rules behind a clearly-bounded list, or defer entirely.
- **Defer the value-dependent cases** — matrix/tensor *dimensions* in the type,
  branding/generativity, and *mutability-as-a-refinement* — to const generics
  (TIP elsewhere) and TIP-0001, not to the refinement-predicate machinery.

## The ladder of power

Every "refinement type" feature is one rung on a ladder. Each rung up adds
expressiveness and subtracts on cost (compile time, learnability, portability,
determinism).

| Rung | Feature | What it proves | Checked | Cost |
|------|---------|----------------|---------|------|
| 0 | **Newtype** (`type Id = newtype Int64`) | nominal distinctness | compile, free | ~none |
| 1 | **Construct-time predicate** (`Real64 > 0`, smart constructor) | value valid *at construction* | construct site (compile if literal, else runtime) | tiny |
| 1b | **Subrange** (`Int64 in 1..=31`), **units** (`weight = Real64`) | bounded / dimensioned at construction | as rung 1, plus dimension algebra | tiny |
| 2a | **Flow-sensitive narrowing** (`if p(x) { x: T where p }`) | predicate holds *on this path* | compile, structural | small |
| 2b | **Frozen propagation set** (option (b): `x>0 / y>0 -> >0`) | a *fixed list* of arithmetic facts | compile, bespoke rules | medium, must freeze the list |
| 3 | **General SMT refinement** (LiquidHaskell, F\*, Dafny) | arbitrary first-order facts | compile, SMT solver | high: slow, non-portable, non-deterministic |
| 4 | **Dependent types** (Idris, Agda, Lean) | arbitrary, proof-carrying | compile, manual/tactic proofs | very high: research-grade |

Tel's `12-refined-types.md` options map as: **(a) = rungs 0–1b**, **(b) = rung
2b**, **(c) = rung 3+**. The recommendation here is **0–1b committed, 2a added,
2b open, 3+ rejected**.

## What evidence we actually have

There is little controlled empirical data on "refinement-type ROI". What exists
is three weaker-but-real evidence types — and they point the same way:

1. **Longevity in production.** A feature that has been in a shipping language
   for decades *without churn* is a strong "freeze candidate" — it has already
   passed the stability test Tel cares about most.
2. **Deployment in high-assurance domains.** Avionics, rail, and crypto stacks
   adopt the cheap rungs widely and the expensive rungs only behind dedicated
   verification teams. That split is itself the signal.
3. **Famous-bug case studies.** Where a single missing distinction caused a
   well-documented failure, the wrapper that would have caught it is a
   high-ROI candidate. Tel's own bug catalogue
   ([`12-refined-types.md` § Bugs this prevents](../05-types/12-refined-types.md#bugs-this-prevents))
   is this evidence type.

### Cross-language survey

| Language / tool | Rung shipped | Since | Verdict it supports |
|---|---|---|---|
| **Pascal / Modula-2** subranges | 1b | ~1970 | subranges are *ancient* and safe |
| **Ada** subtype + `Static_Predicate`/`Dynamic_Predicate`; **SPARK** proves them | 1b (+3 opt-in) | 1983 / 2012 | cheap rungs in avionics & rail for 40 yrs; full proof only behind a team |
| **Nim** `distinct` + `range[0..10]` | 0, 1b | 2008 | newtype + subrange, mainstream, low-ceremony |
| **Haskell** `newtype` + smart constructors; `refined` lib | 0, 1 | ~1996 | newtype is universal idiom; predicates done by convention |
| **LiquidHaskell** | 3 | 2014 | bolt-on SMT refinement; research / high-assurance, not default |
| **F\*** | 3–4 | 2011 | SMT + dependent; Project Everest / miTLS, expert-only |
| **Dafny** | 3 | 2009 | subset types + SMT verification; verification-first niche |
| **Idris / Agda / Lean** | 4 | 2007+ | dependent; proofs, not general scripting |
| **F# units of measure** | 1b | 2008 | dimensions, *erased at runtime, zero-cost*, 15+ yrs stable |
| **Boost.Units (C++)** | 1b | 2007 | compile-time dimensional analysis, zero runtime |
| **Scala 3** opaque types; **`refined`** lib | 0, 1 (lit) | 2021 | newtype standardised; `refined` checks *literals* at compile, values at runtime |
| **Kotlin** value classes | 0 | 2020 | newtype, zero-cost where erasable |
| **TypeScript** branded types, type guards | 0, 2a | — | branding by structural hack; *narrowing* (rung 2a) is heavily used and loved |
| **Whiley** flow typing + refinement | 2a, 3 | 2010 | flow-sensitive refinement as a *core* idea; runtime + optional proof |
| **Rust** newtype; const generics (ints) | 0 | 2015 / 2021 | newtype universal; value-in-type limited to const generics, no predicate refinement |
| **Futhark / Dex** size-typed arrays | (shape) | 2016+ | array *dimensions* tracked in types — research, dependent-flavoured |

The shape of the data is consistent: **rungs 0–1b are everywhere, old, and
boring** (the good kind). **Rungs 2a appears as a beloved feature wherever it
ships** (TypeScript narrowing, Whiley). **Rung 2b ships nowhere as a clean
"small frozen set"** — languages jump from (a) to full (c). **Rungs 3–4 cluster
in high-assurance and research**, always gated behind expertise.

## The three lenses the user asked for

### Lens A — "copy and stay frozen for years"

Mature enough to lift from another language and freeze:

- **Newtypes (rung 0).** 30 years of `newtype`/`distinct`/opaque-type/value-class.
  Nothing to invent. **Commit.**
- **Construct-time predicates / smart constructors (rung 1).** The universal
  Haskell idiom, formalised by Ada predicate subtypes. **Commit.**
- **Subrange types (rung 1b).** Pascal-to-Nim, ~50 years. **Commit** — as a
  construct-time predicate (`Int64 in 1..=31`), *without* value-tracking through
  arithmetic.
- **Units of measure (rung 1b).** F# (2008) and Boost.Units (2007): erasable,
  zero-cost, dimension-checked. **Commit**, in its own chapter
  ([`13-units.md`](../05-types/13-units.md)).

### Lens B — "gradual: don't pay if you don't use it"

Refinement is *inherently* opt-in: a script can stay on bare `Int64`/`Real64`/`Text`
forever. But "gradual" has a subtle edge — a feature is only free for
non-adopters if it does not change *their* inference or error messages.

- **Truly gradual (no tax on non-users):** newtypes, construct-time predicates,
  units, subranges. A user who never writes one never sees one. **These are the
  safe gradual core.**
- **Gradual *to write* but leaks into inference:** flow-sensitive narrowing
  (rung 2a) and propagation (rung 2b) change the *result types* of ordinary
  operations. Even a user who never *declares* a refinement may see `Real64 > 0`
  in an error message or hover. Still opt-in to *author*, but no longer
  invisible. Acceptable for 2a (narrowing reads naturally); a real cost for 2b.
- **Not gradual:** SMT refinement (rung 3) taxes *everyone* via compile time and
  toolchain weight even on un-refined code, because the solver sits in the build.

### Lens C — "largest bug-prevention / better-modelling potential"

Ranked by documented ROI against cost:

1. **Newtypes for argument-swap / unit / id / scope mix-ups.** The single
   highest-ROI item — see the six real bugs in
   [`12-refined-types.md`](../05-types/12-refined-types.md#bugs-this-prevents).
   Cost ~0. **Top priority.**
2. **Units of measure.** Mars Climate Orbiter is the canonical case; the
   nanos/millis and strategy-vol bugs in our catalogue are the same shape.
   High ROI in scientific/financial scripting — Tel's wheelhouse.
3. **Non-empty / bounded / subrange.** Kills off-by-one, empty-collection, and
   out-of-range-index classes at the boundary.
4. **Flow-sensitive predicate narrowing** (the `user where is_admin` case
   below). Models *authorisation and state-machine* invariants the type system
   otherwise can't — high modelling value, and ties to capability-gated I/O.
5. *(rung 2b/3)* General arithmetic propagation and SMT facts add real
   guarantees but with steeply rising cost; their marginal bug-prevention over
   1–4 is small for the *scripting* workloads Tel targets.

## Three concrete pressure-tests

The user named three cases that sharpen exactly where the line falls.

### 1. Propagating `user where .is_admin` statically

The wish: after a check, the fact flows into the type.

```tel
# Sketch — syntax not settled.
fn delete_account(u: User where .is_admin) { … }   # callable only with proof

fn handler(u: User) {
    delete_account(u)            # ERROR: u not known to be admin
    if u.is_admin {
        delete_account(u)        # OK: inside this branch, u: User where .is_admin
    }
}
```

This is **flow-sensitive narrowing (rung 2a)** — *occurrence typing*, the same
mechanism as TypeScript's type guards and Whiley's flow typing. The key
restriction that keeps it cheap: it narrows only along the **path** where the
predicate was *syntactically checked*, on an **immutable** binding, for a
**decidable, side-effect-free** predicate (a field read, a tag test, a
`len > 0`). It does **not** require a solver, because the compiler is not
*deriving* the fact — it is *propagating one the program already tested*.

It edges toward rung 3 only if you ask for facts the program did **not** test
(`if a < b` then later "we know `a < b-1`"): that is arithmetic the compiler must
*derive*, which is the SMT slope. **Lean: ship the propagate-a-tested-predicate
form (2a); reject derive-new-facts (3).** This is also the cleanest non-I/O way
to model authorisation, and dovetails with capabilities in
[`03-features.md`](../02-philosophy/03-features.md).

`TODO(open): does narrowing survive a function-call boundary, or only within one
function body? Cross-call narrowing needs the predicate in the *signature*
(`u: User where .is_admin`), which is fine; intra-body narrowing of a local is
the rung-2a feature. Pin which bindings are narrowable (immutable only, surely).`

### 2. Matrix / tensor dimensions in the type

The wish: `Matrix[3, 4] * Matrix[4, 2] -> Matrix[3, 2]`, with the inner-dimension
mismatch a *compile error*.

This is **value-dependent typing**: the type contains *values* (the dimensions)
and the result type runs *arithmetic on them*. It is the strongest "edges toward
dependent types" case in the whole language (Futhark and Dex make a research
language out of exactly this). Two honest options:

- **Const generics over dimensions** (the inputs' `arrays are rectangular, rank
  is not generic` rule helps): `Matrix[N, M]` with `N, M` const-generic
  naturals, and a *small fixed* multiply rule `[N,M]·[M,P] -> [N,P]`. This is
  rung-2b-shaped: a frozen rule, not a solver — but it needs **const generics
  with naturals** first, which is fundamental open question #2, not this TIP.
- **Dimensions as ordinary runtime values** (checked at construction / at the
  multiply): no compile-time shape guarantee, but zero type-system weight.
  This is what Python+NumPy does, and what most scripting actually tolerates.

**Lean for Tel1: runtime-checked dimensions by default; static shapes *only* if
const-generic naturals land for independent reasons.** Do not let tensor shapes
be the tail that wags the type system into dependent territory. Track under
const generics ([`07-generics.md`](../05-types/07-generics.md)), cross-reference
from here.

`TODO(open): if const-generic naturals are adopted, is shape arithmetic limited
to `+`, `·`, and equality of dimension expressions? Anything beyond (e.g.
`reshape` proving `N*M == P*Q`) is the dependent-type slope again.`

### 3. Mutability as a refinement

The wish (from TIP-0001 and fundamental question #2): treat `mutable` / `uniq`
as a *refinement* or *const-generic* of a type, so one declaration yields both a
mutable and an immutable variant (`ListBuilder` vs `List`).

This *looks* like a refinement type, but it is a different axis:

- A value-predicate refinement (`Real64 > 0`) narrows the **set of values** a type
  admits and is checked at **construction**.
- A mutability refinement narrows **what you may do with a value** and is a
  **substructural / capability** property, enforced by the borrow checker, not a
  construct-time predicate.

Folding mutability into the *same* machinery as value predicates is seductive
(one mechanism) but couples the borrow model to the refinement solver — exactly
the entanglement fundamental-question #2 warns about. **Lean: keep mutability in
TIP-0001's substructural model; if it is expressed as a const generic, that is
the const-generic mechanism, not the refinement-predicate mechanism.** They may
*share surface syntax* (`Type where …`) without sharing semantics.

`TODO(open): decide whether `where` clauses on a type can mention *both* value
predicates and capability/mutability facts, or whether these are syntactically
distinct. Sharing one keyword for two enforcement engines risks confusing both
users and the compiler. Defer to TIP-0001's resolution.`

## Why the expensive rungs lose *for Tel specifically*

General refinement (rung 3) and dependent types (rung 4) are rejected not because
they are bad — they are excellent in their domains — but because they collide
head-on with Tel's top priorities:

- **Determinism across hosts (priority #2, and the bedrock of #1).** An SMT
  solver's result depends on solver *version*, *timeout*, and *machine speed*: a
  proof that succeeds on one host can time out on another, so the *same program*
  would type-check differently across hosts. That breaks the central promise
  that a Tel program compiles and means the same thing everywhere. A frozen
  language cannot ship a typechecker whose answers drift.
- **Compile speed & portability (priority #3, #6).** Every host implementation
  would have to embed (and agree on) the same solver. For a language meant to be
  re-implemented by host authors who are *not* the language team, a mandatory
  SMT dependency is disqualifying.
- **Easy to learn, one good way (priority #5, #7).** Refinement proof obligations
  and dependent eliminators are expert tools. They contradict "a Python/Kotlin
  reader is barely surprised."

These three points are not currently stated in
[`02-philosophy/`](../02-philosophy/01-priorities.md). That is the **philosophy
gap** this TIP surfaces: *the philosophy chapter should explicitly rule out
solver-in-the-typechecker*, so future "let's add Liquid types" proposals have a
standing answer.

## The one genuinely open call: option (b)

Everything above is a confident lean. The single hard call is rung **2b** — a
*small, frozen* set of propagation rules (the `12-refined-types.md` examples:
`Real64 != 0` divisor making division total; `>0 / >0 -> >0`; range arithmetic).

The case *for*: it delivers the headline "division is total when the divisor is
provably non-zero" win without a solver, by special-casing a handful of
operators.

The case *against*, and why it is risky to **freeze**:

- **No precedent ships exactly this.** Languages do (a) or jump to (c); a
  curated middle list is untested territory, and Tel1 is frozen forever.
- **Every rule is bespoke** and the set must be complete-enough at 1.0, because
  adding rule #N+1 later changes inference for existing code (a soft
  compatibility break).
- **It is the rung that leaks into Lens B** — non-adopters see refined result
  types in errors.

`TODO(open): the call on option (b). Three sub-options: (b0) ship none — pure
rung-1 construction-time checking, simplest and safest to freeze; (b1) ship a
*tiny* list (non-zero divisor → total division, and non-negativity preservation)
and freeze it loudly; (b2) defer the whole question to a post-1.0 Tel2-only
feature. Lean: **b1**, but only if the list can be capped at ≤3 rules with a
written rationale for the boundary. Otherwise b0.`

## Decision table (proposed for Tel1)

| Capability | Rung | Tel1 verdict |
|---|---|---|
| Newtype wrappers | 0 | **Commit** |
| Construct-time predicates / smart constructors | 1 | **Commit** |
| Subrange numeric types | 1b | **Commit** (construct-time only) |
| Units of measure | 1b | **Commit** (own chapter) |
| Flow-sensitive narrowing of a *tested* predicate | 2a | **Commit** (immutable bindings, no derivation) |
| Frozen arithmetic propagation set | 2b | **Open** — lean tiny-or-none |
| Static matrix/tensor dimensions | (2b) | **Defer** to const-generic naturals |
| Branding / generativity | (2b) | **Defer** (per `12-refined-types.md`) |
| Mutability-as-refinement | — | **Defer** to TIP-0001 (different engine) |
| SMT-backed general refinement | 3 | **Reject** for Tel1 |
| Dependent types | 4 | **Reject** for Tel1 |

## Open questions

- The option-(b) call (see TODO above) — the only thing here without a clear
  lean.
- Whether `where` is one keyword for two engines (value predicates *and*
  mutability/capability facts) — defer to TIP-0001.
- Narrowing scope: intra-body only, or also as signature refinements across
  calls (almost certainly both, but pin the binding-mutability rule).
- Const-generic naturals: a prerequisite for static shapes and the
  `[N,M]·[M,P]` rule — belongs to fundamental question #2, referenced not
  decided here.
- The philosophy chapter should gain an explicit antifeature: *no
  solver/SMT in the typechecker* (determinism + portability rationale above).

## See also

- [`05-types/12-refined-types.md`](../05-types/12-refined-types.md) — the home
  chapter; options (a)/(b)/(c) live there.
- [`05-types/13-units.md`](../05-types/13-units.md) — units of measure (rung 1b).
- [`05-types/07-generics.md`](../05-types/07-generics.md) — const generics, the
  prerequisite for static dimensions.
- [`tips/0001-mutability-and-borrowing.md`](0001-mutability-and-borrowing.md) —
  where mutability-as-refinement actually belongs.
- [`02-philosophy/01-priorities.md`](../02-philosophy/01-priorities.md) — the
  priorities the rejections appeal to.

TODO: review
