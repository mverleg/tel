# TIP-0006: Tuples as Argument Bundles — Splat, Tuple Returns, Partial Application

**Status:** Accepted (partially) and migrated into the chapter docs
(2026-06-16); kept as the historical record. **What shipped:** monomorphic splat
(`f(...b)`) and tuple-return composition. **Adopted (2026-06-17):**
exact-match forwarding — a function exposes implicit `::Args` / `::Return`
associated types and a bundle of type `f::Args` may be splatted into `f`, with
**no subtyping**. **Deferred:** partial application (the no-partial lean is kept
— use a lambda). **Rejected (2026-06-17):** row *subtyping*
(width / permutation / optional). See [Resolution](#resolution-2026-06-16).
**Touches:** `05-types/04-tuples-and-arrays.md`, `05-types/05-function-types.md`, `07-expressions/06-function-application.md`, `09-functions/02-parameters-and-arguments.md`, `09-functions/03-return-values.md`, `09-functions/04-default-and-named-arguments.md`, `09-functions/05-variadic-functions.md`

## Summary

The docs already lean on one equivalence: **a call's argument list and a tuple
have the same shape** — an ordered, partly-named, heterogeneous group (see
[tuples as argument bundles](../05-types/04-tuples-and-arrays.md#tuples-as-argument-bundles)),
and a function type's left side `(A, B) -> R` *is* a tuple type (see
[function types](../05-types/05-function-types.md)). This TIP proposes to **lean
into that equivalence deliberately** and harvest three features from it:

1. **Application based on a tuple** — splat a bundle into a call (`f(...b)`),
   and compose functions by feeding one's return tuple as the next's arguments.
2. **Return types are tuples** — already true; this TIP makes the *output* row a
   first-class partner of the *input* row, so composition lines positional
   returns up by position and named returns up by name.
3. **Partial application based on tuples** — supplying a valid *prefix* bundle
   yields a residual function over the complement. Varargs and block parameters
   participate; the trailing-block-outside-parens **syntax** does not.

The unifying claim: one object — a **labelled tuple (row)** — is viewed three
ways: a tuple value, a call's argument bundle, and the parameter side of a
function type. The output side of a function type is a tuple too, so a function
reads as `InRow -> OutRow`.

## Recommended outcome (one-line summary)

- Adopt **monomorphic splat / partial application** now: where the bundle's
  shape is statically known, splat and prefix-partial are **shape-checked
  sugar** that desugar to ordinary positional/named calls — no new runtime
  machinery, no row polymorphism.
- **Gate the polymorphic forms** (forwarding an *unknown*-shape bundle, generic
  `compose`) behind the row-polymorphism decision tracked in the tuples and
  function-types chapters. They are a real commitment and need not ship together
  with the monomorphic sugar.
- Keep every bridge between "tuple" and "argument list" **explicit and
  syntactic** — this is the single rule that separates Tel's design from the
  Swift regret it superficially resembles.

## The Swift regret this must not repeat

Tel's own `language-design-regrets` input (in the repo's `input/` directory)
records two Jordan Rose / Swift regrets that sit *directly* on this feature:

- **"Using tuples to represent function argument lists in the type system"** —
  called *"always wrong for Swift,"* the source of persistent compiler
  complications and years of disentangling proposals.
- **"Labeled tuple elements"** (`-> (x: Double, y: Double)`) — once labels are
  allowed, the language must answer casting between differently/reordered labels,
  colon-overloading ambiguity, and runtime label questions.

Tel has **already** adopted named tuple fields (the second regret is off the
table — see the tuples chapter), so this TIP cannot pretend the risk is absent.
What it argues is that Swift's pain came from the *implicit* conflation, and Tel
avoids precisely that:

| Swift's trap | Tel's rule |
|---|---|
| Auto-tupling / un-tupling: `f(b)` silently spreads a tuple `b` into a call, or wraps args into a tuple | **No implicit spread.** `f(b)` always passes `b` as **one** argument. Spreading is a **written operator** (`f(...b)`), never inferred. |
| Reordered/renamed labels require runtime casts between tuple types | **Positional names are not part of the type** (only keyword-only names are — see [function types](../05-types/05-function-types.md)); label questions are confined to the named tail, decided structurally at compile time. |
| Tuple-vs-arglist distinction blurred, so the compiler can never tell them apart | One object, but the **discriminator is syntactic** and resolved on the opening token with bounded lookahead (the existing grouping-vs-tuple rule). The *views* differ at the surface even though the value is one. |

The test for every spelling decision below: **could a reader confuse a tuple
argument with a spread argument list?** If yes, the spelling is wrong.

## 1. Application based on a tuple (splat)

`apply` takes `f: R -> S` and a bundle `b: R` and yields `S`. There are two
tiers, and only the first is cheap.

### Monomorphic splat — shape-checked sugar

When `b`'s shape is statically known, splatting it is checked position-by-position
and name-by-name, then desugars to an ordinary call:

```tel
let b = (value, low, high)          # a 3-tuple, shape (Int64, Int64, Int64)
clamp(...b)                          # ≡ clamp(b.0, b.1, b.2)

let opts = (port = 9000, retries = 5)
connect("example.org", ...opts)     # ≡ connect("example.org", port = 9000, retries = 5)
```

No row machinery: the shape is known, so the compiler emits the same code as the
spelled-out call. This is the 95% case and the one this TIP recommends shipping.

The splat marker is written and explicit — `f(b)` still passes `b` as a single
tuple-typed argument. This is the anti-Swift rule made concrete: a reader never
has to guess whether a tuple at a call site spreads.

**Resolved (2026-06-16): splat is spelled `...b` inside the call.** A leading
`...` in *argument position* cannot be confused with the `...rest` destructuring
marker (a *binding* position) or the `vararg` parameter marker (a *signature*
position) — different positions, so the token is unambiguous at a call site,
despite the optical overlap. Migrated to
[function application](../07-expressions/06-function-application.md#splatting-a-bundle-into-a-call)
and the [variadic](../09-functions/05-variadic-functions.md) splat-into-vararg
case. (The word-marker `spread b` and apply-operator alternatives were not
taken.)

### Exact-match forwarding — adopted (2026-06-17)

A wrapper that forwards a bundle needs the bundle's row to be a **type variable**.
A function type `R -> S` exposes that row as two implicit associated types:

- **`f::Args`** — the input row (a tuple type), and
- **`f::Return`** — the output row.

Forwarding is then "splat a value whose type *is* `f::Args` into `f`." With
**exact match** — `args` must be precisely `f::Args`, no narrowing or widening —
the splat is well-typed by construction:

```tel
# Forwards whatever it was given. R is the *exact* arg row; no subtyping.
fn timed[R, S](f: R -> S, args: R) -> S {
    let t0 = now()
    let out = f(...args)
    log.debug("took " & (now() - t0))
    out
}
```

`f: R -> S, args: R` names the row directly; the `f::Args` / `f::Return`
projection is the same statement for when no binder is available (return position,
a field whose type tracks another field). Under monomorphization a concrete call
stamps `R` to a known row and `f(...args)` becomes the ordinary **monomorphic
splat** above — so this needs **no runtime row machinery**.

Two honest limits of exact-match:

- **Forward only, don't construct.** With an abstract `R` you can splat a row you
  were *given*; you cannot build one (its shape is unknown in the generic body).
  Wrappers (`timed`, `retry`, `memoize`, `compose` over matching rows) are fine;
  "build the args, then call" still needs the concrete shape.
- **No looser fit.** `compose(f, g)` works when `g`'s param row *equals* `f`'s
  return row; anything looser would need subtyping, which is rejected (below).

### Row subtyping — rejected (2026-06-17)

The looser forms — forwarding into a function that expects a *different*-shaped
row via **width** (extra named fields), **permutation** (named order-free), or
**optional** (defaults satisfy a wider row) subtyping — are **not adopted**. The
same no-implicit-row-subtyping stance is recorded in the
[tuples chapter](../05-types/04-tuples-and-arrays.md#tuples-as-argument-bundles),
the [function-types chapter](../05-types/05-function-types.md), and the
no-`**kwargs` rule in
[parameters](../09-functions/02-parameters-and-arguments.md#no-keyword-arguments-dictionary).
Exact-match `::Args` / `::Return` covers the forwarding and matching-`compose`
cases without it, so the subtyping lattice buys too little for its cost. Where
rows genuinely differ, the answer stays the explicit lambda.

## 2. Return types are tuples — composition lines up both rows

A function returns a tuple already (`fn divmod(a, b) -> (Int64, Int64)`). Unifying the
*output* side with the *input* side buys **composition by splat**: when `g`'s
parameter row equals `f`'s return row, `f`'s result feeds straight in.

```tel
fn divmod(a: Int64, b: Int64) -> (Int64, Int64) { (a / b, a % b) }
fn show(q: Int64, r: Int64) -> Text { q & " rem " & r }

show(...divmod(17, 5))              # positional return → positional args, by position
```

Named returns line up with keyword parameters **by name**:

```tel
fn stats(xs: List[Int64]) -> (sum = ..., count = ...) { ... }
fn report(*, sum: Int64, count: Int64) -> Text { ... }

report(...stats(xs))                # named return → keyword args, by name
```

A **single** return value is *not* a one-tuple. The grouping-vs-tuple
discriminator already settled for literals (`(x)` is grouping, `(x,)` is a
1-tuple) applies unchanged in return position: `-> Int64` returns an `Int64`, only
`-> (Int64, Int64)` returns a row. This keeps the common single-value case free of
tuple ceremony and means splatting a single-value return is simply an ordinary
one-argument pass.

This also answers the "where do defaults live" sub-question for returns: a return
tuple is a *value*, carrying only what the body produced; defaults are a property
of the *callee signature* the bundle is later matched against — never of the
tuple. (Same rule as the captured-bundle defaults TODO in the tuples chapter.)

## 3. Partial application based on tuples

> **Deferred (2026-06-16).** This whole section is **not adopted**. The
> function-types chapter's *no partial-application sugar* lean is **kept** — a
> partial application stays a lambda (`|v| clamp(v, 0, 1)`), per *one good way*.
> The prefix-bundle grounding below is recorded as the case that *could* justify
> reversing it later; until then nothing here ships, and the vararg-reach-in
> sub-question is moot.

A partial application supplies **a valid prefix bundle** and yields a residual
function over the complement.

A bundle's positional section is **dense by construction** (keys `0..k`). So a
partial bundle is:

- a dense positional **prefix** — positions `0..k` for some `k ≤ positional
  arity`, possibly empty; plus
- any **subset of the named** arguments (the named tail is sparse/optional, so
  any subset is itself a valid bundle).

The residual function's parameter row is the complement: positions `k..n` still
to be supplied, plus the keyword parameters not yet set.

```tel
fn clamp(value: Int64, low: Int64, high: Int64) -> Int64 { ... }

let clamp_unit = clamp.partial(low = 0, high = 1)   # residual: (Int64) -> Int64  (only `value` left)
clamp_unit(x)

# prefix of positionals:
let from_zero = clamp.partial(0)                     # residual: (Int64, Int64) -> Int64 (low, high left)
```

`TODO(open): partial-application spelling.` Shown as a `.partial(...)` form for
concreteness. Alternatives: a splat where an *incomplete* bundle yields a residual
rather than calling (elegant — the same operator calls when the bundle is complete
and curries when it is not — but makes "did this call or curry?" depend on bundle
completeness, which fights explicitness); or a placeholder/keyword form. The
current docs *lean against* a partial-application sugar
([function types](../05-types/05-function-types.md#partial-application-and-currying));
**this TIP reverses that lean** on the strength of the tuple grounding, and the
reversal is itself the open decision. Confirm whether the reversal is wanted before
picking a spelling.

### Why prefix-only — no arbitrary positional holes

Supplying positions `0` and `2` while leaving `1` would make the prefix
non-dense — **not a valid bundle**. The row model therefore *forbids* arbitrary
positional holes for free, with no extra rule. This is a feature, not a
limitation:

- It keeps partial application explicit and shaped like the rows everything else
  uses.
- It draws a clean "one way" line: **tuple-partial does prefixes; lambdas do
  everything else** — holes, reordering, computing an argument from another.
  `clamp.partial(low = 0)` for the prefix case, `|v, h| clamp(v, 0, h)` when you
  need the hole.

Named arguments have no ordering, so pre-setting any subset of them is always
valid and drops them from the residual's keyword section.

### No silent currying

Partial application is **always explicitly written**. Under-application of an
ordinary call stays an arity error:

```tel
clamp(7)            # ERROR: clamp takes 3 positional args
clamp.partial(7)    # OK: residual (Int64, Int64) -> Int64
```

This preserves the existing no-implicit-currying stance
([function-application antifeatures](../07-expressions/06-function-application.md))
while granting the feature through an explicit door.

### Varargs participate (because the bundle is the pre-collection spread)

A vararg ends the positional section and collects trailing positionals — *but
only at a full application*. A bundle is the **pre-collection spread form** (the
[spread-vs-collect TODO](../05-types/04-tuples-and-arrays.md#tuples-as-argument-bundles)
resolved spread-side), so partial application interacts with it cleanly:

```tel
fn draw(x: Int64, y: Int64, vararg points: Point, *, color = "black") -> Canvas

let at_origin = draw.partial(0, 0)          # prefix stops before the vararg
at_origin(p1, p2, color = "red")            # vararg still open, collected here

let red_origin = draw.partial(0, 0, color = "red")   # also pre-set a keyword
red_origin(p1, p2, p3)                       # collected at the full call
```

Two cases:

- **Prefix stops before the vararg** — the residual keeps the vararg fully open.
- **Prefix reaches into the vararg** — supplied trailing positionals sit in the
  spread; the residual's vararg can still extend (a vararg is `≥ 0` trailing),
  and collection into the list happens only at the eventual full application.

The only thing the signature must decide is "is this trailing positional a fixed
parameter or a vararg element?" — answered by the fixed positional arity, known
statically. Keyword-only parameters are unambiguous because they are named.

`TODO(open):` confirm that a partial bundle may reach *into* the vararg at all, or
whether prefix-partial stops at the vararg boundary (supply the rest only at the
full call). Stopping at the boundary is simpler; reaching in is more expressive.
Tied to the spread-vs-collect resolution.

### Block parameters participate; trailing-block *syntax* does not

The trailing [block parameter](../09-functions/02-parameters-and-arguments.md#trailing-block)
is the last section and an ordinary bundle element. Partial application may
pre-bind it or leave it for the residual.

What does **not** carry over is the trailing-block-**outside-the-parens** sugar
(`f(x) { ... }`). That sugar is a *call-site* convenience keyed on the call
parentheses; a tuple literal / bundle value has no parens to trail. So in bundle
or partial form the block is supplied as an element *inside* the bundle:

```tel
repeat.partial(3)                 # residual still wants the block
repeat.partial(3)({ print("hi") })   # block as an in-bundle element — OK
# repeat.partial(3) { print("hi") } # NOT the trailing-block sugar in partial form
```

This is a **surface-only** restriction: the block is still fully passable, just
not through the outside-parens shortcut. Semantically nothing is lost.

## What this costs

- **Monomorphic splat + prefix-partial:** shape-checked desugaring. No runtime
  rows, no new type machinery. The risk is purely *grammar* — choosing splat and
  partial spellings that can never be read as an ordinary tuple argument.
- **Exact-match forwarding (`::Args` / `::Return`):** invariant row variables, no
  subtyping. Type-level cost only; under monomorphization it collapses to the
  monomorphic splat, so no runtime rows. *Adopted.*
- **Row subtyping (width / permutation / optional):** the standing big-ticket
  commitment. *Rejected* — exact-match covers forwarding and matching-`compose`,
  and the lattice buys too little for its cost.

## Resolution (2026-06-16)

The five decision points were resolved as follows and migrated:

1. **Reverse the "no partial application" lean?** — **No.** Keep the lean;
   partial application stays a lambda. §3 is deferred.
2. **Splat spelling** — **`...b` inside the call.** Disambiguated from
   `...rest` / `vararg` by position (argument vs. binding vs. signature), the
   optical overlap accepted.
3. **Partial spelling** — **moot** (partial application deferred).
4. **Vararg reach-in** — **moot** (part of deferred partial application).
5. **How far the polymorphic forms go** — **two tiers, settled 2026-06-17.**
   *Exact-match forwarding* is **adopted**: functions expose implicit `::Args` /
   `::Return`, and a bundle of type `f::Args` splats into `f` with no subtyping
   (collapses to the monomorphic splat under monomorphization). *Row subtyping*
   (width / permutation / optional) is **rejected**. Where rows genuinely differ,
   "forward arbitrary args" stays an explicit lambda.

**Shipped:** monomorphic `...b` splat
([function application](../07-expressions/06-function-application.md#splatting-a-bundle-into-a-call),
[variadic](../09-functions/05-variadic-functions.md)) and tuple-return
composition ([return values](../09-functions/03-return-values.md)); plus the
settled bundle rules in [tuples](../05-types/04-tuples-and-arrays.md#tuples-as-argument-bundles)
(defaults live on the signature; a bundle is the pre-match spread shape).

## See also

- [Tuples and Arrays](../05-types/04-tuples-and-arrays.md) — the argument-bundle
  section and the spread-vs-collect rules this builds on.
- [Function Types](../05-types/05-function-types.md) — `InRow -> OutRow`, name
  significance per section, partial-application lean.
- [Function Application](../07-expressions/06-function-application.md) — calls,
  trailing block, lazy args.
- [Parameters and Arguments](../09-functions/02-parameters-and-arguments.md) — the
  binary positional/keyword section model and the no-`**kwargs` rule.
- [Variadic Functions](../09-functions/05-variadic-functions.md) — the splat /
  spread-vs-collect open questions.
- [TIP-0001](0001-mutability-and-borrowing.md) — `FnOnce` and capture-based
  function flavours that a residual closure inherits.
