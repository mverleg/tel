# TIP-0002: Untagged Unions, Sealed Traits, and the One Closed-Set Construct

**Status:** Accepted and migrated into the chapter docs (2026-06-16); kept as
the historical record. (Recommendation was reversed 2026-06-13 — was "keep both,
do not unify"; now "unify onto unions." See
[History](#history-the-earlier-keep-both-recommendation).)
**Created:** 2026-06-04
**Touches:** `10-data-modelling/02-union-types.md`,
`10-data-modelling/03-traits-or-interfaces.md`,
`08-control-flow/02-match-expressions.md` (pattern syntax),
`05-types/01-type-system-overview.md` (nominal / structural / bound),
`05-types/12-refined-types.md` (newtype identity & inheritance),
`02-philosophy/03-features.md`, `02-philosophy/04-antifeatures.md`
**Downstream of:** the union-member-types question (concrete-only members) and
the parked *closed/sealed trait* open question in
[`02-union-types.md`](../10-data-modelling/02-union-types.md) and
[`03-traits-or-interfaces.md`](../10-data-modelling/03-traits-or-interfaces.md).

## Summary

Tel had two candidate ways to express "a closed set of cases": an **untagged
union** (`(A | B | C)` — a structural set, matched exhaustively) and a **sealed
trait** (a trait whose implementer set is fixed and known, the Rust/Kotlin/Scala
enum shape). The worry was one-good-way bloat — two constructs for one job.

This TIP concludes they should **not both exist as case-modelling constructs**,
and **unifies onto the union**:

- the **union** is the single *closed-set-of-data* construct;
- the **trait** is *only ever* a behaviour **bound** — never a way to model
  variant data;
- **"sealed" stops being a kind of trait.** The thing a sealed trait was reaching
  for — *a closed set whose members all provide some behaviour* — is expressed as
  a **union with a trait bound on its members**: `(A | B | C) : Trait`.

This reverses the earlier draft's "keep both, do not unify" recommendation. The
analysis that justified keeping them — that the two are **duals** — is exactly
what now shows one construct plus a bound suffices.

## Recommended outcome

- **One case construct: the untagged union.** A "closed set of cases" is always a
  union. No `sealed trait`-as-data, no `union { … }` keyword block.
- **Traits are bounds only.** A trait constrains types and abstracts behaviour; it
  is never the type of a value (`dyn Trait` is the one erased-value exception) and
  never a case/data construct. See the
  [nominal / structural / bound split](../05-types/01-type-system-overview.md#nominal-structural-or-a-bound).
- **`: Trait` is a capability floor on a union.** `(A | B) : Drawable` *requires*
  every member to implement `Drawable`, checked at the union, so member churn
  cannot silently drop the capability. It is a floor, not a ceiling — the union
  still auto-exposes the whole shared intersection.
- **Matching is always on the union** — by member type, exhaustive by default
  (non-exhaustive is an opt-out). This is the home for "exhaustive match /
  downcast"; you never match *a trait*.
- **Structural identity.** A named union is a transparent alias: `type A = (X | Y)`
  and `type B = (X | Y)` are the same, interchangeable type, `(X)` *is* `X`, and a
  distinct closed family is `newtype (X | Y)`.
- **Parens (P): mandatory `(A | B)`** for an anonymous union, everywhere it is
  spelled. Strict now, relaxable to bare `A | B` later (one-way), and it dissolves
  the lambda-pipe collision for free.

## The duality — why one construct suffices

The two mechanisms are **duals** along two independent axes:

| | **member set** | **behaviour surface** |
|---|---|---|
| **Union** | **listed** at the union | **discovered** — the intersection of what members happen to share |
| **Sealed trait** | **collected** from whoever `impl`s it | **dictated** — the trait declares what must be supported |

So the real axes are *listed vs collected* and *discovered vs dictated* — not
"union vs trait." Read that way, the four corners are:

- **(listed, discovered)** = a plain union `(A | B)`.
- **(listed, dictated)** = a union with a required contract `(A | B) : Trait`.
- **(collected, dictated)** = a sealed trait.
- (collected, discovered) — degenerate, no one wants it.

A sealed trait is just the *collected* member-set spelling of "closed set with a
contract." Tel already chose **listed** members everywhere else (union members are
standalone, reusable, host-mappable concrete types — see
[`02-union-types.md`](../10-data-modelling/02-union-types.md#why-untagged--and-the-pitfalls-accepted)),
so the *collected* corner buys nothing a listed union with `: Trait` doesn't, and
costs the inverse ownership (variants belonging to the trait), the loss of set
algebra and subtyping (`(A | B) ⊂ (A | B | C)`, load-bearing for the absence
story), and clean host mapping. **Drop the collected corner; keep the union, add
the contract as a bound.**

## The `: Trait` capability floor

A bare union auto-exposes the **intersection** of its members' fields, methods,
and trait impls — `p.name` works if every member has it. That intersection is
*discovered and fragile*: add a member lacking `area()` and the union silently
stops being `Drawable`, with the error surfacing far from the cause.

`: Trait` turns the discovered surface into a **declared floor**:

```tel
type Shape = (Circle | Square | Triangle) : Drawable
```

- **Member-wise requirement.** `: Drawable` means *every member* implements
  `Drawable`, checked **at the union declaration**. Adding a non-`Drawable` member
  is a compile error *here*, where the mistake is. This is the guard against
  accidental capability loss — the same "assert a property so a later edit cannot
  break it" pattern as the requestable `Alias` in
  [TIP-0001](0001-mutability-and-borrowing.md).
- **A floor, not a ceiling.** The union still exposes *everything* its members
  share; `: Drawable` only pins the part you promise to keep. If members also
  share `Hashable`, that surface is available too — but it is *not* edit-stable
  unless also pinned. Pin more with the [trait-list](../10-data-modelling/03-traits-or-interfaces.md#trait-lists-and-bound-aliases)
  grammar or a bound alias: `(A | B) : Drawable + Hashable`, `(A | B) : Renderable`.
- **Behaviour the members don't individually have** is still possible — write
  `impl Drawable for Shape { … match … }` on the union itself. The `: Trait`
  floor (members each implement it, forwarded) and an explicit union `impl` (one
  match-based implementation) are complementary.

This is precisely the "dictated behaviour" half of the duality, expressed as one
optional annotation on the one construct — not a second construct.

## Structural identity and the `newtype` opt-in

Unions are **structural** (see
[nominal / structural / bound](../05-types/01-type-system-overview.md#nominal-structural-or-a-bound)):
two unions are equal exactly when their member sets are equal.

```tel
type A = (X | Y)
type B = (X | Y)        # A, B and (X | Y) are the SAME type — interchangeable
```

A `type X = …` declaration over a union is therefore a **transparent alias**, not
a new type, and `: Trait` does not change that — it is a checked *constraint*, not
an identity, so `(X | Y) : T` named two ways is still one type. A single-member
union collapses to its member (`(X)` is `X`), so a union narrowed by a guard to
one remaining case *is* that case's type.

When a **distinct** closed family is wanted instead of a structural alias, wrap
the union: `newtype (X | Y)` opts into nominal identity on top of the structural
set (see [refined types](../05-types/12-refined-types.md)). That is the nominal
escape hatch — and the only reason to reach for it, since "sealed trait for a
distinct family" no longer exists.

## Matching, exhaustiveness, and "downcasting"

Because the *type is the tag*, a union is consumed by matching on the member type
— which **is** type-safe, exhaustive downcasting, sound *because* the member set
is closed:

```tel
match shape {
    c: Circle    => c.radius * c.radius * pi,
    r: Rectangle => r.width * r.height,
    t: Triangle  => t.base * t.height / 2,
}
```

This **absorbs the sealed-trait "exhaustive match" question** (the earlier
A-vs-B fork on whether sealing unlocks downcasting): in the unified design you
never match *a trait*, you match a *union*, and unions have always been
matchable. "Sealing unlocks exhaustive downcast" is reframed as "model the closed
set as a union" — the capability lands, the second construct does not. An *open*
trait stays purely a dispatch bound; there is nothing to downcast out of it.

Three points specific to union matching (the broader grammar — arrow spelling,
guards, literals, nesting — is owned by
[`08-control-flow/02-match-expressions.md`](../08-control-flow/02-match-expressions.md)):

- **An arm is `binding: Type`** — narrows and binds at the member type; the
  binding is optional for data-less variants (`Triangle => …`).
- **`|` groups members into one arm** — `Circle | Rectangle => …`. A grouped arm
  sees the **sub-union** `(Circle | Rectangle)`, i.e. that sub-set's shared
  surface. `TODO(open):` confirm a grouped arm binds at the sub-union type, and
  pin a binding spelling if one is wanted.
- **Exhaustiveness is over the member set.** No constructor catch-all; only `_`
  (or a bare binding), required exactly for non-exhaustive unions.

## What this retires

Unifying onto unions resolves several previously-parked questions at once:

- **"Sealed trait as a data/case construct"** — *rejected*, now firmly (it was the
  *collected* corner of the duality).
- **"Can a sealed trait be a union member?"** (parked in
  [`02-union-types.md`](../10-data-modelling/02-union-types.md#members-are-concrete-types))
  — *dissolved.* You never put a bare trait in a union; you list the concrete
  members and add `: Trait`. Members stay concrete and disjoint.
- **Sealed/closed-trait declaration spelling; per-module vs per-crate
  closedness** — *not needed*, because closedness lives at the union (the member
  list *is* the closure).
- **"Does a union auto-expose shared *methods/traits*, not just fields?"**
  ([`02-union-types.md`](../10-data-modelling/02-union-types.md#shared-fields-and-methods),
  [type overview](../05-types/01-type-system-overview.md#nominal-structural-or-a-bound))
  — *yes*, decided; and `: Trait` is the explicit way to *require* it.
- **`union { … }` keyword block** (the old S2/S3) — *rejected*. "Closed set with
  attached behaviour" is `(A | B) : Trait` plus, if needed, an `impl … for` the
  union. No keyword block earns its place.

## Union syntax — parens (P)

A union surfaces in three syntactic positions, kept the same `|` operator
throughout:

| Position | Spelling | Where |
|---|---|---|
| **Declaration** | `type Shape = (Circle \| Square)` | a named alias for a union |
| **Type** | `fn area(s: (Circle \| Square)) -> Real64` | anywhere a type is written |
| **Pattern** | `match s { c: Circle => …, s: Square => … }` | match arms / destructuring |

**Anonymous unions are mandatorily parenthesised — `(A | B)`** — everywhere they
are spelled, including declarations. The decisive reasons:

- **One-way restriction.** Requiring parens now and relaxing to bare `A | B`
  later is backward-compatible; the reverse is a breaking change. Start strict.
- **One invariant, zero edges.** "An anonymous union is always `(A | B)`" needs no
  outermost-vs-nested qualification and no special case.
- **The lambda-pipe collision dissolves for free.** A union annotation inside
  `|…|` would clash with the closing pipe (`|x: A | B| …` — union-continue or
  close?); parens settle it with one token of lookahead — `|x: (A | B)| …`.
- **Clarity at the hard sites.** `((1 | 2), (3 | 4))` is readable where
  `(1 | 2, 3 | 4)` is not; always-parens makes the easy sites pay the same small,
  reversible tax.

`|` is otherwise unambiguous: Tel has **no** bitwise-or (`& | ^ ~` are free;
bitwise is `bin_or`/`bin_and`/… functions), and the type, pattern, and
expression positions are disjoint. The lambda-pipe case is the only real
collision, and parens close it.

`TODO(open):` refinement types ([TIP-0004](0004-how-far-refinement-types-go.md))
are the one thing that blends value-expressions into type position; re-check the
parenthesised form still delimits cleanly where a predicate sits beside a union.

## Decision table (revised for Tel1)

| Question | Verdict |
|---|---|
| Separate constructs for "closed set of cases"? | **No — one construct, the union** |
| Unify union + sealed trait? | **Yes — onto the union** (the *listed* corner of the duality) |
| `sealed trait` as a data/case construct? | **Reject** |
| "Closed set with a required contract"? | **`(A \| B) : Trait`** (capability floor) |
| Sealed trait as a union member? | **Dissolved** — list concretes + `: Trait` |
| Traits as types? | **No** — bounds only; `dyn Trait` is the one erased-value exception |
| Auto-expose a union's shared methods/traits? | **Yes**; `: Trait` requires it |
| Matching / downcasting | **On the union**, by member type, exhaustive; never on a trait |
| Named-union identity | **Transparent alias** — `type A = (X\|Y)` ≡ `type B = (X\|Y)`; `(X)` is `X` |
| Distinct closed family | **`newtype (A \| B)`** |
| Anonymous-union spelling | **Mandatory parens `(A \| B)`** (strict now, relaxable later) |
| `union { … }` keyword block | **Reject** |

## Open questions

- **`: Trait` × non-exhaustive unions.** A `nonexhaustive` union with `: Trait` —
  the floor must bind future members too (that is the point). Confirm the rule and
  the error wording when a later member would violate it.
- **`impl … for <union>` coherence.** Writing a trait `impl` on a *structural*
  union type raises ownership/orphan questions — who may write it, and how it
  interacts with the orphan rule in
  [TIP-0005](0005-trait-coherence-and-the-orphan-rule.md). Pin this down.
- **`newtype` over a union** — match/unwrap behaviour of a nominally-wrapped union
  is not yet documented (flagged in
  [refined types](../05-types/12-refined-types.md#trait-inheritance-for-newtypes)).
- **Grouped match arm `A | B => …`** — confirm it binds at the sub-union and pin a
  binding spelling. Downstream of the match-doc pattern grammar.
- **Auto-exposure depth** — exact rules for forwarding a *trait* (not just a
  field) across members, and how `: Trait`-required forwarding composes with an
  explicit union `impl`.

## History: the earlier keep-both recommendation

The first draft (2026-06-04) recommended **keeping both** untagged unions and a
*sealed-as-a-trait-attribute*, and **not unifying**, on the argument that they
answer different questions (set-of-cases vs contract-of-behaviour) and that
collapsing them re-introduces nominal/structural confusion. That argument was
correct about the *axes* but drew the wrong conclusion: the two are **duals**, and
the cleaner resolution is to keep the one *listed* construct (the union) and
express the contract as a **bound** on it (`: Trait`), rather than keeping a
second *collected* construct. The duality table above is the same analysis,
turned to the unify conclusion. The earlier sealed-trait-as-union-member
lean is **superseded** — the member-overlap problem it was solving does not arise
when members are always concrete and the contract is a bound.

## See also

- [`10-data-modelling/02-union-types.md`](../10-data-modelling/02-union-types.md)
  — the closed-set mechanism; concrete-members rule, structural identity,
  exhaustive matching.
- [`10-data-modelling/03-traits-or-interfaces.md`](../10-data-modelling/03-traits-or-interfaces.md)
  — traits as bounds, the trait-list grammar, bound aliases, trait objects.
- [`05-types/01-type-system-overview.md`](../05-types/01-type-system-overview.md#nominal-structural-or-a-bound)
  — the nominal / structural / bound framing this TIP rests on.
- [`05-types/12-refined-types.md`](../05-types/12-refined-types.md) — `newtype`
  identity and opt-in trait inheritance.
- [`08-control-flow/02-match-expressions.md`](../08-control-flow/02-match-expressions.md)
  — the pattern grammar union matching plugs into.

TODO: review
