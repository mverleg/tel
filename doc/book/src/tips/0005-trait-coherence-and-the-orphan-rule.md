# TIP-0005: Trait Coherence and the Orphan Rule

**Status:** Accepted — migrated into the Touches chapters (2026-06-16)
**Created:** 2026-06-05
**Touches:** `10-data-modelling/03-traits-or-interfaces.md`,
`10-data-modelling/07-equality-and-hashing.md`,
`10-data-modelling/08-ordering.md`,
`11-modules-and-packages/03-visibility.md`,
`11-modules-and-packages/04-packages.md`,
`05-types/07-generics.md`, `05-types/12-refined-types.md`,
`02-philosophy/01-priorities.md`, `02-philosophy/04-antifeatures.md`,
`20-appendix/05-design-history-and-changelog.md`
**Downstream of:** the changelog's already-stated lean — *"the orphan rule plus
crate-scoped trait implementations cover the realistic conflicts"*
([`05-design-history-and-changelog.md`](../20-appendix/05-design-history-and-changelog.md))
— and TIP-0003's api/impl dependency direction
([`0003`](0003-module-levels-and-dependency-direction.md)).

## Summary

When two crates can each write `impl T for D` for the same trait `T` and type
`D`, a program that pulls in both has a **conflict with no obvious winner**. The
sharp case is the *diamond*: crate `A` depends on `B` and `C`, and `B` and `C`
each implement trait `T` (from a fourth crate `D`) for the same type. Which
implementation is `A` using? If `T` is `Ord` or `Hash`, the answer is not merely
academic — a `Set` or `Map` built under one implementation and read under another
**silently corrupts**: lookups miss, sets contain "duplicates," merges
double-count.

Tel's [changelog](../20-appendix/05-design-history-and-changelog.md) already
leans toward *"the orphan rule plus crate-scoped trait implementations,"* but
no chapter spells out the rule, the alternatives, or why the alternatives lose.
This TIP does that, and covers the two cases the bare lean omits — **generic
impls** and **specialisation**. A key conclusion: the traits where coherence
actually bites — `Eq` / `Hash` / `Ord` — need **no special rule**. The orphan rule
plus the global no-overlap rule (cross-crate overlap is a rejected *conflict*,
not a silent pick) already give *exactly one* resolved implementation per (trait,
type) in any program, so the diamond **cannot form** for them any more than for an
ordinary trait. The separate concern — that a single `Eq`/`Hash` impl be
*internally* consistent (`a == b ⇒ hash a == hash b`) — is handled by the
[`identity` key-set / `derive`](../10-data-modelling/07-equality-and-hashing.md),
not by an ownership restriction. (An earlier draft special-cased these as
single-owner — "O6" below — but it proved redundant.)

## Recommended outcome (one-line summary)

- **Adopt the orphan rule, covering generic impls too.** A concrete `impl T for D`
  is allowed only in the crate that defines `T` *or* the crate that defines
  `D`. A **generic** `impl<…> T for D<…>` is allowed only when a type owned by the
  crate appears in the impl head *before any uncovered type parameter* (the
  *covering rule*, below). Neither owner present / nothing local covering →
  wrap in a [newtype](../05-types/12-refined-types.md). Checked at compile time;
  gives **one resolved implementation per (trait, applied-type) across the whole
  program** — global uniqueness by construction.
- **Specialisation is allowed — but only within one owner.** Overlapping impls
  (a general one and a more specific one) are permitted **provided they live in
  the same crate**, where the orphan rule has already forced every overlapping
  candidate to be co-located. Resolution is *most-specific-wins*, computed with
  the full candidate set in view, so it is **total and not scope-sensitive** — the
  hazard that sinks cross-crate specialisation simply cannot arise. Ties are a
  **declaration-site** error, never a use-site surprise.
- **`Eq`/`Hash`/`Ord` need no special ownership rule.** The orphan rule plus
  conflict-rejection already guarantee one resolved impl per type, so the
  container-corruption diamond cannot form for them. Their *internal* contract
  (`a == b ⇒ hash a == hash b`, and a specialisation agreeing with its general
  impl) is a separate matter, kept by the
  [`identity` key-set / `derive`](../10-data-modelling/07-equality-and-hashing.md)
  or author discipline. (Earlier drafts special-cased these as single-owner — O6
  below — but it proved redundant.)
- **When you want two *unrelated* behaviours, pass a value, not a second instance.**
  Two orderings of one type is a real need but *not* a specialisation (neither
  refines the other); Tel's answer is an explicit value-level comparator
  (`sort(xs, by = …)`), kept visible and local rather than resolved by import scope.
- **Reject** cross-crate local/overlapping overrides, named instances as the
  primary mechanism, and cross-crate specialisation. Each trades a real
  bug-class or the locality guarantee for expressiveness Tel recovers more safely
  via newtypes, same-owner specialisation, and value-level functions.
- **No global conformance registry.** Enforced statically; nothing Swift-like at
  runtime. Suits AOT, monomorphisation, and embedding.

## The problem, precisely

Three properties are usually conflated under the word "coherence." Separating
them (after ezyang's framing) is what makes the options legible:

- **Confluence** — constraint solving reaches the *same* instance choice no
  matter the order it runs in. A prerequisite; nobody wants to drop it.
- **Coherence** — every valid typing of a program has the *same dynamic
  semantics*. Because a trait bound elaborates to a concrete implementation in
  generated code, coherence means the program does the same thing regardless of
  how the type-checker happened to thread the impls through.
- **Global uniqueness** — in the whole compiled program there is *at most one*
  implementation per (trait, type) pair.

The diamond is a **global-uniqueness** failure. The reason it *hurts* is a
**coherence** failure on top: a data structure carries an implementation inside
it. A `Set[D]` is organised by some `Ord D` / `Hash D`; hand it to code holding a
*different* `D` implementation and its invariants no longer hold — the structure
was built for one and is being read by another. This is the user's own
observation that hashing/ordering "should be constant per type": a value's place
in a hashed or sorted container must not depend on *which module is looking at
it*.

So "globally unique" is the property worth buying. The options below are ranked
by how they get it (or what they give up instead).

## Design space

### O1 — Free-for-all: global uniqueness, no orphan rule

Anyone may implement any trait for any type; rely on it "just working." This is
Swift's fully general **retroactive conformance**.

It does not work. Swift needs a process-global conformance registry (because
`as?` does dynamic conformance checks), and two libraries adding the *same*
conformance **collide at runtime** with no way to name or choose. Swift's own
designers now call it *"a giant hole in the library-friendly story"* and conclude
*"the next language should not allow retroactive conformances … in the fully
general form."*

- *Pro:* maximally expressive; no wrapper types ever.
- *Con:* the diamond becomes a runtime crash or a silently-wrong pick; needs a
  runtime registry Tel's [embedding/AOT priority](../02-philosophy/01-priorities.md)
  cannot pay for. **Rejected** — it violates *surprise is a cost* and
  *reproducible by default* at once.

### O2 — Orphan rule: global uniqueness by construction *(recommended backbone)*

An `impl T for D` is legal only in the crate owning `T` or the crate owning
`D`. Rust and Haskell (by default) both do this. The diamond cannot form: in
`A → {B, C} → D`, neither `B` nor `C` owns `T` (that is `D`'s crate, or the
trait's) nor the target type, so neither may write the impl. The impl lives once,
with an owner, and every dependent sees the same one.

- *Pro:* global uniqueness is a *static, local* check — no runtime machinery, no
  scope-sensitivity, and "looks correct ⇒ is correct" holds. Familiar to Rust
  readers. Composes with TIP-0003's crate model directly.
- *Con:* the one real cost — you **cannot** add `impl ForeignTrait for ForeignType`
  even when it would be harmless; you must newtype-wrap. This is the expressiveness
  tax, and it is the price of the bug-class going away.

### O3 — Orphan rule + local overrides / overlapping instances

Keep the orphan rule but let a crate override locally, or allow overlapping
impls resolved per-module — Haskell's `OverlappingInstances` / `IncoherentInstances`,
and GHC's stance of checking only *"the subset of the instance database it uses
when it compiles any given module."* This **drops global uniqueness** and keeps
only *local* coherence.

This is precisely the user's *"dangerous if e.g. hashcode changes at crate
boundary."* The danger is real and documented: when two modules with different
instances **compose**, you get a `Set` with duplicate elements, or a `Map` whose
keys are unreachable. The bug surfaces far from the override, at the seam where
the two halves meet.

- *Pro:* the expressiveness of O1 without a runtime registry.
- *Con:* trades a compile error for *silent data corruption that travels across
  module boundaries* — the worst possible shape under *prevent, don't fix* and
  *if it looks correct it is correct*. **Rejected.**

### O4 — Named instances / explicit dictionaries

Give implementations names and make the use site pick one: Scala 3 `given`s
imported into scope, ML functors, Idris named implementations, Coq canonical
structures, and the Rust `MyHashTable[i8: my_mod::MyHash[i8]]` dictionary-passing
proposal — *which Tel's [changelog already rejected](../20-appendix/05-design-history-and-changelog.md)
as "surface complexity … enormous."*

This is the user's *"impls have names, and you have to be specific (same
problem)."* Two distinct problems, and naming only solves the lesser one:

1. **Resolution ambiguity** ("the compiler can't pick") — naming *does* fix this:
   you say which one.
2. **Container identity** ("a `Set` built with X must never be read with Y") —
   naming **does not** fix this. Scala givens are explicitly *not coherent*; the
   classic failure is exactly passing an incompatible `Ordering` to code holding a
   `Set` built with another. The hazard moves from the compiler to *every use
   site that must remember to thread the same name through.*

It also taxes the common case (you now name an instance you never had two of) and
is *unfamiliar* to the mainstream audience Tel targets.

- *Pro:* maximal expressiveness; the genuine "two orderings" case is sayable.
- *Con:* keeps the dangerous half of the problem, taxes the 99% case, unfamiliar.
  **Rejected as the primary mechanism.** Tel recovers the legitimate need more
  safely below (value-level comparators).

### O5 — Most-specific-wins specialisation

Allow overlap, resolve by a specificity ordering — Rust *specialisation*,
Haskell `OverlappingInstances`' most-specific rule. **Two variants**, and they
fall on opposite sides of Tel's line.

**O5a — cross-crate overlap.** Impls from *different* crates may overlap and
are resolved by specificity. This is the user's *"rules to say which is more
specific — but how to know which is in scope?"* — and that question is the defect:
resolution depends on **what is visible**, so adding an `import` (or a dependency)
can change which body runs. Rust specialisation has been unstable for ~a decade
because making it *sound* — an impl you can't see still has to be respected — is
genuinely hard. Contradicts *reasoning stays local* and *stability*. **Rejected.**

**O5b — intra-crate overlap (specialisation within the orphan rule).** This is
the user's *"ideally allow specialising, even if only within orphan rules,"* and
it survives precisely because the orphan rule **defuses** O5a's defect. Since a
generic impl and any more-specific impl can each only be written by the owning
crate (O2 + the covering rule), **every candidate that could overlap is already
co-located in one crate**. The compiler therefore resolves most-specific-wins
with the *entire* candidate set in view — there is no "which is in scope," because
for a given (trait, applied-type) the set is fixed and crate-global, not
import-dependent. Ambiguous specificity is a **declaration-site** error (the same
containment Tel applies to the [cross-repo overload hazard](../10-data-modelling/03-traits-or-interfaces.md#overload-ambiguity-across-repositories)),
not a use-site surprise.

- *Pro:* recovers the genuinely useful pattern — `impl[T] Show for List[T]` plus a
  faster/nicer `impl Show for List[u8]`; a default behaviour with a tuned override
  — without any scope sensitivity. The hard part of Rust specialisation (respecting
  impls you cannot see) does not exist here because there are none.
- *Con:* the author must keep a specialisation *consistent* with the general impl's
  contract (a faster `Eq` must still agree); Tel can check this for derived/`identity`
  cases but leans on author discipline for fully hand-written pairs. **Accepted**,
  with that obligation stated. This is *not* Tel's earlier "no-specialisation lean"
  ([generics](../05-types/07-generics.md)) — that lean is about *cross-crate*
  specialisation as a power feature; same-owner overlap is a different, contained
  thing and this TIP narrows the lean accordingly.

### O6: Single-owner identity traits (dropped as redundant)

An earlier draft added a refinement layered on O2: the traits a container *stores*
— `Eq`, `Hash`, `Ord` — could be supplied **only by the crate owning the type**,
not even by the trait's owner, so that *exactly one* impl is conceivable.

It turns out to buy nothing O2 + conflict-rejection do not already give. The
corruption case needs **two different `Hash`/`Ord` impls for one type to coexist**.
Under the orphan rule the only crates that may write `impl Hash for T` are the
trait's owner (std) and `T`'s owner; std cannot see a user type, and if both wrote
one it is a **rejected cross-crate conflict**, not a silent pick. So there is
already one resolved impl per type, or a hard error — the diamond cannot form, and
"constant per type" holds without any extra rule.

- *Pro:* turns a (vanishingly rare) link-time conflict into a flat impossibility,
  and lets one say "`Hash` is a property of the type" with no "which owner"
  question.
- *Con:* a special-cased, enumerated trait list and a rule *stricter* than the
  orphan rule, bought for a marginal early-error / tidiness gain — it fights *one
  way to do a thing*. **Dropped.** `Eq`/`Hash`/`Ord` are governed by the plain
  orphan rule like any trait. (One std *design* note, not a language rule: std
  should not ship a *blanket* `Eq`/`Hash`/`Ord` impl, since a blanket is
  specialisable only by its own crate and would block user types from giving
  their own — but that is true of any blanket impl.)

## Recommendation: O2 + O5b, with these commitments

### The orphan rule, in Tel's spelling

For a *concrete* head, `impl T for D` is permitted in crate `P` iff `P` defines
`T` **or** `P` defines `D`. Otherwise it is a compile error directing the author
to a newtype. Scope is the **crate** (TIP-0003's distribution/identity unit),
not the module: a crate may organise its impls across its own modules freely,
but the *outward* guarantee — one resolved impl per (trait, applied-type) in any
program — is a crate-level property. This is the changelog's *"crate-scoped
trait implementations"* made precise.

### Generic impls: the covering rule

A *generic* impl head (`impl[T, U…] Trait[Args…] for Head<…>`) needs more than
"owns the trait or the type," because `Head` may be a foreign type constructor
applied to a local type, or vice versa. Tel adopts the standard *covering*
discipline (the shape of Rust [RFC 2451](https://rust-lang.github.io/rfcs/2451-re-rebalancing-coherence.html),
stated plainly):

> A generic impl is permitted in crate `P` iff either `P` owns the trait, **or**
> a type owned by `P` (the *local* type) appears somewhere in the impl head
> `Trait[Args…] for Head` **before the first uncovered type parameter** — reading
> the parameters left to right, trait type-arguments first, then the self type.

"Uncovered" means a type parameter not wrapped inside a local type constructor.
The point is mechanical: two different crates can never both write impls that
overlap on a concrete instantiation, because each would need to mention a type the
*other* doesn't own. Worked cases:

```tel
# P owns Wrapper:
impl[T] ForeignTrait    for Wrapper[T]     # OK — local self type
impl[T] ForeignTrait[T] for Wrapper<…>     # OK — local before the param
impl[T] ForeignTrait[Wrapper[T]] for T     # rejected — bare `T` self is uncovered,
                                           #   and the local type comes *after* it
impl[T] ForeignTrait    for T              # rejected — blanket over all types;
                                           #   only the trait's owner may do this
```

A fully generic *blanket* impl (`impl[T] T for …` over an unconstrained `T`) is
therefore allowed **only in the trait's own crate**, and even there interacts
with specialisation (below) — a downstream concrete type automatically picks up
the blanket impl, and a same-crate specialisation may refine it, but no foreign
crate can add an overlapping one. This closes the generic half of the orphan
question rather than deferring it.

### Specialisation, but only within an owner

Overlapping impls are legal **iff they are all owned by the same crate**. For
any concrete (trait, applied-type), the compiler gathers *every* candidate the
owning crate declares and picks the **most specific**; if two are incomparable
it is a **compile error at the impl declarations**, never at a call site. Because
the candidate set is crate-global and import-independent, the resolved impl is
the same everywhere — global uniqueness of the *resolved* answer is preserved even
though several impls exist.

This needs one rule the orphan rule does **not** give for free. Because a concrete
`impl T for D` may be written by *either* the trait's owner or the type's owner,
two **different** crates can each write impls that overlap — e.g. `P_show` writes
`impl[X: Show] Show for List[X]` while `P_list` writes `impl Show for List[byte]`.
Tel forbids that split: a general impl and its specialisations must be
**co-located in one crate**, and any overlap whose candidates span two crates
is a **conflict**, rejected — even when one side is the trait's owner and the other
the type's. Both owners are individually entitled to write the impl, so *which*
crate consolidates the pair is the authors' call; Tel only guarantees the split
form cannot silently resolve one way or another. `TODO(open):` where cross-crate
overlap is *detected* — it cannot be at either impl site (neither crate sees the
other's impl), so it surfaces only when a program links both, like Rust's global
coherence check. Confirm the diagnostic names both impls and both owners.

```tel
# All in the crate that owns Show (or the crate that owns List):
impl[T: Show] Show for List[T] { … }      # general
impl          Show for List[byte] { … }   # specialisation — wins for List[byte]
```

The obligation on the author: a specialisation must **refine, not contradict**,
the general impl's observable contract. For `Eq`/`Hash`/`Ord` this is load-bearing
(a faster `Eq` for a sub-case must still agree with the general one, or `Set`
breaks); Tel checks it automatically when both sides are derived or `identity`-based
and otherwise trusts a hand-written pair like any other hand-written impl.

### The specificity rule, and adjacency

"Most specific" is a partial order, deliberately simple:

- **Concrete beats generic, always.** `impl Show for List[byte]` wins over
  `impl[T: Show] Show for List[T]` at `List[byte]`. Two *concrete* heads never
  overlap — with no inheritance, distinct concrete types are disjoint, so there is
  nothing to rank between them.
- **Stronger bounds beat weaker ones, when one set is a superset.** A generic impl
  whose bound set is a **strict superset** of another's is more specific:
  `impl[T: Eq + Hash] …` refines `impl[T: Eq] …`.
- **Incomparable bound sets do not rank — and that is an error where they overlap.**
  `impl[T: Eq + Hash] …` and `impl[T: Eq + Display] …` are incomparable (neither
  set contains the other); a type that is `Eq + Hash + Display` would match both
  with no winner. Tel reports this at the **impl declarations**, not at a call
  site — the author disambiguates with a more specific impl or a single combined
  bound.

Because the whole candidate set is co-located, this order is computed once,
crate-locally, with every candidate in view.

**Adjacency.** A general impl and its specialisations must be declared **adjacent**
— grouped together, not scattered across the crate's modules — so a reader sees
the entire candidate set at the point any one of them is written. A crate may
still spread *unrelated* impls across its modules freely; the grouping requirement
applies only to a *specialisation family* (impls that overlap). This makes
"resolution draws on a fixed candidate set" a *visible* fact, not just a compiler
one, serving *reasoning stays local*.

### Eq, Hash and Ord need no special rule

`Eq`, `Hash`, `Ord` (and the partials) are governed by the **plain orphan rule**,
like any other trait — no stricter single-owner restriction (see
[O6, dropped](#o6-single-owner-identity-traits-dropped-as-redundant)). That is
enough: the orphan rule plus conflict-rejection give one resolved impl per (trait,
type) in any program, so a `Set`/`Map`/sorted collection can never be built under
one impl and read under another — the diamond cannot form.

What *is* load-bearing is a different, per-impl property: the **Eq–Hash contract**
(`a == b ⇒ hash a == hash b`) and the agreement of any same-owner specialisation
with its general impl. That is about a single impl being internally consistent, and
is kept by the
[`identity` key-set or `derive`](../10-data-modelling/07-equality-and-hashing.md)
(auto-checked) or by author discipline for hand-written impls — not by *who* is
allowed to write it.

**Identity traits may be specialised** (same-owner), like any trait. The classic
hazard — crate `B` overriding crate `A`'s `Hash` for `A`'s type — cannot be
written at all: only `A` or the trait's owner may impl it, and a cross-crate
overlap is a rejected conflict, so any specialisation that exists is same-owner,
under the contract obligation above.

### Two *unrelated* behaviours ⇒ a value, not a second instance

Specialisation (above) covers the case where one impl *refines* another. The
different case — *"I want this type sorted two genuinely different ways,"* where
neither ordering refines the other — is **not** a specialisation and is not a
second `Ord` impl either. It is served by an **explicit comparator value**, the
way sorting already takes one:

```tel
sort(people, by = |p| p.age)          # one ordering
sort(people, by = |p| p.surname)      # another, chosen visibly, here
```

A collection that must *retain* a non-default ordering takes the comparator at
construction (`SortedSet.with(cmp = …)`) and carries it as data. The choice is a
value flowing through the program — local, visible, and impossible to resolve
"by accident from import scope." This is *make the right thing easy and the wrong
thing hard*: the wrong thing (a silent second `Ord`) is simply not expressible;
the right thing (an explicit `by =`) is one keyword.

### The newtype escape hatch

When you genuinely need `ForeignTrait for ForeignType`, wrap it:

```tel
# Can't `impl Json for Uuid` — own neither. Wrap.
struct JsonUuid(Uuid)
impl Json for JsonUuid { … }
```

Tel makes this cheap on purpose: [refined/newtypes](../05-types/12-refined-types.md)
inherit the underlying type's behaviour, so the wrapper is thin and the impl is
unambiguously *yours*. The wrap is also the honest signal — "this representation
is mine, and so is this behaviour."

### No global conformance registry

Everything above is a static check. Tel keeps **no runtime conformance table**
and no `as?`-style dynamic conformance test (consistent with the
[no-downcasting / no-reflection](../10-data-modelling/03-traits-or-interfaces.md)
stance). This is what lets O1's failure mode not exist and what keeps the model
affordable for AOT and embedding.

### Interaction with TIP-0003 (dependency direction)

The orphan rule and api/impl flags reinforce each other. The
[two-branch independence result](0003-module-levels-and-dependency-direction.md)
— two private (`impl`) uses of a crate on different branches don't conflict —
holds for *types*, and the orphan rule is why it also holds for *trait impls*:
neither branch could have orphan-implemented a shared foreign trait, so there is
nothing to disagree about. The single-resolved-version rule still applies for
identity-bearing crates. `TODO(open):` confirm an orphan impl living in `D`'s
own crate is always visible through both an `api` and an `impl` edge (it should
be — it belongs to the type/trait, not to the dependency flag).

## Why this fits the philosophy

| Priority / maxim | How this TIP serves it |
|---|---|
| *Prevent, don't fix* | the diamond is a **compile error** under the orphan rule + conflict-rejection, never a runtime corruption |
| *If it looks correct, it is correct* | no scope-sensitive resolution (rejects O5a); specialisation resolves from a fixed, co-located candidate set |
| *Reasoning stays local* | which impl runs never depends on a distant `import`; specialisation overlap is all in one crate |
| *Surprise is a cost* | no silent which-wins (rejects O1/O3/O4); specialisation ties fail at the declaration, not the call |
| *One way to do a thing* | one *resolved* impl per (trait, applied-type); unrelated variants are explicit values, not parallel instances |
| *Stability* | adding a dependency can never change which impl an existing call uses |
| *Embedding* | no runtime conformance registry; static, AOT-friendly |
| *Familiarity* | the orphan + covering rules are Rust's; newtype-to-extend is a known idiom |
| *Expressiveness (3rd)* | recovered via same-owner specialisation (O5b), cheap newtypes, and value-level comparators — without paying the bug-class |

The one place expressiveness loses to safety — you must newtype to extend a
foreign trait on a foreign type — is exactly the trade the
[priorities](../02-philosophy/01-priorities.md) endorse: safety and avoiding
surprise outrank flexibility, and the lost flexibility is *recoverable*, just not
free. Specialisation, by contrast, is *kept* — Tel just confines it to where it
cannot turn scope-sensitive.

## Decision table (proposed for Tel1)

| Question | Verdict |
|---|---|
| Allow free retroactive impls (O1)? | **Reject** — runtime collision, needs a registry, unsafe for embedding |
| Adopt the orphan rule (O2)? | **Accept** — own-the-trait-or-own-the-type, crate-scoped |
| Generic impls covered? | **Yes** — the *covering rule* (local type before the first uncovered parameter) |
| Local overrides / overlapping across crates (O3)? | **Reject** — silent cross-module data corruption |
| Named instances as primary mechanism (O4)? | **Reject** — keeps the container-identity hazard, taxes the common case |
| Cross-crate most-specific specialisation (O5a)? | **Reject** — scope-sensitive, unsound, anti-local |
| Same-owner specialisation (O5b)? | **Accept** — co-located candidates, total resolution, ties fail at declaration |
| Blanket impl `impl[T] Trait for T`? | **Only** in the trait's own crate; then specialisable by that crate |
| `Eq`/`Hash`/`Ord` special single-owner rule? | **No** — the plain orphan rule suffices (O6 dropped as redundant); the Eq–Hash contract is a separate per-impl concern |
| Two *unrelated* behaviours for one type | **Explicit comparator value** (`by = …`), not a second impl |
| Extend a foreign trait on a foreign type | **Newtype wrapper** |
| Runtime conformance registry? | **No** — static checks only |

## Open questions

- `TODO(open):` confirm the **covering-rule ordering** ("trait type-arguments
  first, then self") is the spelling Tel wants, vs Rust's exact left-to-right
  fundamental-type rule. The committed *shape* (a local type before the first
  uncovered parameter) is settled; the precise parameter order and whether any
  type constructors are "fundamental" (auto-covering, like Rust's `&`/`Box`) is
  not. Tel has no references today, so the fundamental-type list may be empty.
- **Resolved — specificity lattice.** Concrete beats generic (and concretes never
  overlap, no inheritance); a strict-superset bound set beats a weaker one
  (`T: Eq + Hash` > `T: Eq`); incomparable bound sets (`T: Eq + Hash` vs
  `T: Eq + Display`) do not rank and are a declaration-site error where they
  overlap. See [the specificity rule](#the-specificity-rule-and-adjacency).
- **Resolved — identity-trait specialisation is allowed** (same-owner). The
  cross-crate-override hazard cannot be written under the orphan rule +
  conflict-rejection; see
  [Eq, Hash and Ord need no special rule](#eq-hash-and-ord-need-no-special-rule).
- **Resolved — adjacency required.** A specialisation family must be declared
  grouped, not scattered across modules; see
  [adjacency](#the-specificity-rule-and-adjacency).
- **Resolved — no single-owner special-casing.** `Eq`/`Hash`/`Ord` are governed by
  the plain orphan rule like any trait; the earlier single-owner rule (O6) was
  dropped as redundant given O2 + conflict-rejection. See
  [O6](#o6-single-owner-identity-traits-dropped-as-redundant).
- `TODO(open):` interaction with [`dyn Trait`](../10-data-modelling/03-traits-or-interfaces.md)
  — a trait object carries its vtable, so impl uniqueness must hold *before* the
  object is constructed. The orphan rule already guarantees this; confirm no gap
  when the trait and type come from different crates than the construction site.
- `TODO(open):` the precise diagnostic. A rejected orphan impl should name *both*
  owners and suggest the newtype, in the spirit of *compiler errors should teach*.
- `TODO(open):` confirm crate-scope (not module-scope) is the right boundary.
  (TIP-0003 dropped the sub-project level: an internal unit is now just an
  unpublished crate, so the only boundaries are module and crate.) Can a
  module hold an impl the crate then exports, and does that stay single? Lean:
  yes; the crate is the outward unit, its modules are internal.

## See also

- [`10-data-modelling/03-traits-or-interfaces.md`](../10-data-modelling/03-traits-or-interfaces.md)
  — traits as bounds; the cross-repo overload hazard this rule's cousin addresses.
- [`10-data-modelling/07-equality-and-hashing.md`](../10-data-modelling/07-equality-and-hashing.md)
  — the `identity` key-set and the Eq–Hash contract a specialisation must keep
  (the per-impl consistency that matters, now that the single-owner rule is
  dropped).
- [`10-data-modelling/08-ordering.md`](../10-data-modelling/08-ordering.md)
  — `Ord`/`PartialOrd`; the two-*unrelated*-orderings case answered by value-level
  comparators.
- [`05-types/12-refined-types.md`](../05-types/12-refined-types.md)
  — newtypes as the cheap escape hatch.
- [`05-types/07-generics.md`](../05-types/07-generics.md)
  — the specialisation lean this TIP narrows (cross-crate out, same-owner in) and
  the blanket-impl rule.
- [`11-modules-and-packages/04-packages.md`](../11-modules-and-packages/04-packages.md)
  and [`0003`](0003-module-levels-and-dependency-direction.md) — the crate as the
  scope of the rule, and the two-branch independence result.
- [`20-appendix/05-design-history-and-changelog.md`](../20-appendix/05-design-history-and-changelog.md)
  — the already-rejected dictionary-passing alternative (O4).

### External references

- ezyang, *Type classes: confluence, coherence and global uniqueness* —
  <https://blog.ezyang.com/2014/07/type-classes-confluence-coherence-global-uniqueness/>
  (the three-property split this TIP uses).
- *On the State of Coherence in the Land of Type Classes* (arXiv 2502.20546) —
  Swift/Rust/Haskell comparison and the newtype/local/retroactive taxonomy.
- *Swift Regret: Retroactive Conformances*, belkadan.com — the O1 failure mode
  from a Swift designer; *"the next language should not allow"* it.
- Rust RFC 2451, *re-rebalancing coherence* —
  <https://rust-lang.github.io/rfcs/2451-re-rebalancing-coherence.html>
  (generic-impl orphan refinement, the first open question).
- GHC User's Guide, *Instance declarations and resolution* — overlapping /
  incoherent instances (O3) and their documented hazards.
