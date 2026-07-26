# TIP-0010: Lambda receivers and builder DSLs

**Status:** **Accepted and migrated** into the chapter docs (2026-07-03) — the
receiver design is settled in
[Closures and Lambdas](../09-functions/06-closures-and-lambdas.md#lambda-receivers)
and [Method Syntax](../09-functions/08-method-syntax.md#receiver-closures-for-dsls),
with the type notation in
[Function Types](../05-types/05-function-types.md) and the builder call shape in
[Function Application](../07-expressions/06-function-application.md#param-less-blocks-builders-and-control-flow).
Consumers: the [typed Tel-data DSL](../17-standard-library/18-tel-as-data.md) and
[`log.sub(...)`](../17-standard-library/14-observability-and-logging.md). Kept as
the historical record.
**Created:** 2026-06-18
**Touches:** `09-functions/06-closures-and-lambdas.md` (the `lambda receivers`
open question), `09-functions/08-method-syntax.md` (implicit `self`, the
leading-`.` rule, and the *receiver / context functions* open question),
`05-types/05-function-types.md` (how a receiver is written in a function type —
the `Order.total` method-as-value shape), `07-expressions/06-function-application.md`
(param-less builder blocks — the call shape this rides on),
`17-standard-library/18-tel-as-data.md` (the typed Tel-data DSL),
`17-standard-library/14-observability-and-logging.md` (`log.sub(...) { … }`),
and [TIP-0009](0009-inline-lambdas-and-non-local-control-flow.md) (inline
lambdas — the *control-flow* half of the same DSL story).

## Summary

A **lambda receiver** is an implicit context object a block runs *against*, so
the members of a builder are reachable inside the block without naming the
builder on every line. It is what turns

```tel
html(|h| { h.head(|hd| { hd.title("Hi") }); h.body(|b| { b.p("hello") }) })
```

into

```tel
html {
    head { title("Hi") }
    body { p("hello") }
}
```

This is the feature that makes a *user-defined* builder read like dedicated
markup syntax — the headline win behind Kotlin's type-safe builders, Groovy's
Gradle DSL, and Ruby's `instance_eval` DSLs (RSpec, Rails routes).

But "an implicit `this`" is exactly the kind of magic Tel's *no implicit
anything* maxim distrusts, and the field is littered with the regret of getting
it wrong (JavaScript `with`, Groovy `resolveStrategy`, Kotlin's nested-receiver
leak that forced `@DslMarker`, and — most recently — Kotlin *removing* context
receivers altogether).

> **Revision note.** An earlier draft of this TIP proposed marking every receiver
> use with a *leading `.`* (`.head`, `.title`). That is **reversed.** Two reasons:
> (1) Tel already uses a **line-leading `.` for chain continuation**
> ([lexical-structure §08](../03-lexical-structure/08-whitespace-and-newlines.md)),
> so `html { .head() / .body() }` parses as the chain `body(head(it))`, not two
> receiver statements — and chaining is the far more established meaning of `.`;
> and (2) a **receiver closure is the same thing as a method** — a function with a
> `this`/`self` context — so it should obey the *same* rule, and a method body
> already reaches `self` by **bare name**, not a dot. (Visual Basic's dotted
> receiver works only because VB has no line-leading-`.` chaining to collide with;
> Tel does, so it cannot follow VB.)

The proposed shape, revised: **a receiver closure is a method with `this`
rebound to the builder. Bare names resolve through `this` exactly as in a method
body — this is not new implicitness, it is the rule methods already have. The
leading `.` is left entirely to chaining.** Explicitness is set at the
*declaration* (omit the receiver → implicit `this`; name it → must qualify),
there is **one** active context (innermost only, no scope chain), and the
no-shadowing rule makes a bare-name/member collision a compile error — so "where
did this name come from" still has an answer.

Receivers are **orthogonal** to [inline lambdas](0009-inline-lambdas-and-non-local-control-flow.md):
a receiver supplies *context*, `inline` supplies *control flow*. They are
designed to **compose** — the best DSLs use both — but they are not one feature,
and folding them together would conflate two independent axes.

## What Tel says today

The idea is referenced, leaning toward "wanted but unresolved", in three
places:

- **Closures and lambdas** carries the open marker directly:
  > `TODO(open): lambda receivers.` Whether a lambda can carry a Kotlin-style
  > *receiver* … so a DSL block like `html { … }` or `body { … }` operates on a
  > builder without naming it. This is the cleaner answer to "what does a bare
  > trailing `{ }` run against" than an implicit `it`, and it composes with the
  > param-less-block rule … Open: how a receiver is declared on the parameter,
  > how it interacts with method-call resolution, and whether it is worth the
  > extra implicitness given *no implicit anything*.

  It also fixes the surrounding rules this TIP must respect: a bare `{ … }` is
  **param-less** (parameters are never written inside braces), and `\{ … }` /
  `|x|` are the only ways a block gains a parameter.
- **Method syntax** already decided the two pieces a receiver needs — and they
  point straight at the answer:
  > `self` can be omitted when used … Inside an `impl` block, a method body
  > refers to fields and other methods of the implementing type without
  > prefixing them with `self.`
  and
  > A leading `.` may also stand in for `self.` (`.total` instead of `total`).

  It also flags the broader feature as open and *deferred*:
  > `TODO(open): receiver / context functions.` … Kotlin-style *extension
  > functions with a receiver* … expressive for DSLs but adds learning curve
  > and bumps the method-resolution rules … **Lean: not in 1.0**.
- **Function types** gives the notation a receiver rides on: a method used as a
  value is written `Order.total` — *the type carries the receiver in front of
  the dot*. A receiver-taking block parameter is the same shape.
- **Function application** supplies the call surface: a param-less trailing
  `{ … }` is "exactly what a builder … wants", and the nested `html`/`head`/
  `body` example is already in the docs as the motivating case.

So the call shape exists, the `self`/leading-`.` resolution rule exists, the
type notation exists, and the only open question is whether to wire them
together for block parameters — and how to do it without importing the regret.

## The problem precisely

A builder exposes a tree of methods. Without a receiver, every level must name
the intermediate value, which buries the structure the DSL exists to show:

```tel
fn build_page(h: !HtmlBuilder) {
    h.head(|hd| {
        hd.title("Report")
        hd.meta(charset: "utf-8")
    })
    h.body(|b| {
        b.h1("Sales")
        b.table(rows)
    })
}
```

The `h.` / `hd.` / `b.` noise is the whole problem: the reader wants to see
*head → title, meta; body → h1, table*, and instead reads a column of receiver
names. Three things are at stake, and a plain lambda gives none of them:

1. **Implicit context** — the block runs *against* a builder, so its methods are
   in reach without a name on every call.
2. **No parameter** — the block stays a bare `{ … }`; there is nothing to bind
   (the context is the binding), which is why it nests cleanly.
3. **Static, analysable resolution** — the IDE and the reader can still answer
   "what does this call resolve to" without running the program (the failure
   mode of Groovy's dynamic `delegate`).

A receiver delivers (1) and (2). The Tel-specific design work is delivering them
*without losing* (3) — which is where every cautionary tale below went wrong.

## Prior art — what others did, and what they like or regret

Six strategies, spanning "best builder DSLs in the industry" to "deprecated
language mistake". Tel can take the ergonomics of the first without the magic of
the last.

### 1. Receiver lambdas (Kotlin)

A function type `T.() -> R` is a lambda *with a receiver*: inside the block,
`this` is a `T` and its members are in scope unqualified. This powers `apply`,
`with`, `run`, `buildString`, `buildList`, and the canonical type-safe HTML
builder. `@DslMarker` annotations stop an inner block from accidentally seeing
an *outer* receiver's members; `this@html` labels disambiguate when you must
reach one.

- **Liked:** the best statically-typed builder DSLs in wide use — fully checked,
  IDE-completable, zero ceremony at the leaves.
- **Regretted:** the receiver is *fully implicit* — `foo()` might be a member of
  the nearest receiver, an enclosing receiver, or a top-level function, and you
  cannot tell by looking. Nested builders leaked scope badly enough that the
  language had to add `@DslMarker` *after the fact*, plus `this@label` escape
  hatches — surface area bolted on to contain the implicitness.

### 2. Delegate + resolve strategy (Groovy / Gradle)

A `Closure` has both an `owner` (lexical) and a `delegate` (the builder), and a
`resolveStrategy` (`OWNER_FIRST`, `DELEGATE_FIRST`, …) chosen at runtime decides
which a bare name hits. This is the engine under the Gradle DSL.

- **Liked:** maximally flexible; arbitrary dynamic DSLs with no type declarations.
- **Regretted:** *the* canonical Gradle pain — "where does this method come
  from" is unanswerable statically, resolution order is a runtime property, and
  no IDE can reliably complete or navigate it. Gradle's own push toward a Kotlin
  DSL is in large part an escape from this. A direct warning against
  *dynamically* resolved receivers.

### 3. `instance_eval` / `instance_exec` (Ruby)

Runs a block with `self` rebound to the receiver. The mechanism behind RSpec
(`describe … do … end`), Rails routing, and most Ruby internal DSLs.

- **Liked:** the prettiest internal DSLs of the dynamic-language era.
- **Regretted:** `self` switches *invisibly* mid-block; combined with
  `method_missing` you genuinely cannot tell what a bare call does without
  knowing the receiver's whole surface at runtime. Beautiful to write, hard to
  read — the exact trade Tel inverts.

### 4. Context functions / implicits (Scala 3)

A context function `A ?=> B` takes an implicit receiver-like parameter; combined
with `given`/`using` it threads a context (a builder, a transaction) without
naming it. Type-safe and composable.

- **Liked:** statically checked, composes through call chains, no second
  closure kind.
- **Regretted:** implicits are Scala's signature complexity sink — action at a
  distance, where the value supplying the context can be defined in another file
  and selected by type. Power at the cost of "what is in scope here?" locality.

### 5. The `with` statement (JavaScript) — *the antipattern*

`with (obj) { … }` puts an object's properties in the bare-name scope of the
block. It is **deprecated, banned in strict mode**, and universally advised
against: it makes scoping undecidable (a bare name might be a property or an
outer variable, decided at runtime), defeats optimisation, and is a frequent bug
source. JS *also* shows the restrained version working: arrow functions
deliberately **do not** rebind `this`, precisely so the receiver stays
predictable.

- **Liked:** nothing, in retrospect.
- **Regretted:** wholesale — the clearest "implicit receiver scope is a mistake"
  data point in any mainstream language. The lesson Tel takes: *do not let the
  receiver bleed into bare-name resolution.*

### 6. Extension methods + object initialisers (C#)

C# gives `this`-on-a-type via extension methods and a fixed builder syntax via
object initialisers (`new Foo { X = 1, Y = 2 }`), but **no** general
receiver-lambda: you cannot author an arbitrary nested DSL the way Kotlin or
Ruby can.

- **Liked:** extension methods are discoverable and statically resolved; object
  initialisers cover the common "set some fields" builder with zero machinery.
- **Regretted:** not general — the nested-tree DSL (`html { body { … } }`) is
  simply out of reach, so libraries fall back to fluent chains.

### 7. Beyond the JVM — the same idea, the same regret

The receiver pattern is not a JVM curiosity; it recurs across very different
languages, and so does its regret:

- **Visual Basic** `With obj … End With` reaches the receiver with a **leading
  `.`** (`.Name = "x"`, `.Save()`). It is the *one* mainstream design that uses a
  dot for the receiver and is **not** regretted — because VB has no line-leading-
  `.` chain continuation to collide with, so the dot is free. (Nested `With` →
  `.` means the innermost — VB already does innermost-only.) This is the direct
  precedent for, and against, a dotted receiver in Tel: it works only when `.` is
  *not* already spent on chaining.
- **Pascal/Delphi, D** `with rec do …` bring fields into **bare scope**. Pascal's
  is the original ambiguity regret — a bare name might be a field or an outer
  variable, decided silently; nested `with a, b` is worse, and style guides warn
  against it.
- **Lua** `_ENV` / `setfenv` swap the environment a chunk runs in so a table's
  fields become the bare-name scope (config DSLs, sandboxing). Powerful; costs a
  table lookup per name and the usual "which scope?" doubt.
- **Crystal** `with obj yield` runs a block resolving bare calls against `obj`
  first — Ruby's idea, but statically resolved (so much milder).
- **Smalltalk** cascades (`obj foo; bar; baz`) sequence messages to one receiver
  with an explicit separator and **no** rebinding — loved, no regret.
- **Swift result builders** (`@resultBuilder`, SwiftUI's `VStack { … }`) and
  **F# computation expressions** build nested DSLs with **no implicit receiver at
  all** — the block's statements are collected by desugaring. Proof the receiver
  is *optional*; their regret is opaque errors and slow type-checking, not scope.

### 8. What Kotlin has since learned

Kotlin is the most-cited success here, but its *own* evolution is the sharpest
warning, and it has moved in three corrective steps:

1. **`@DslMarker` (1.1)** — nested receiver lambdas leaked the *outer* receiver's
   members into inner blocks; an annotation had to be bolted on afterwards to
   scope them. (Tel's innermost-only rule is this, built in.)
2. **`this@label`** — extra surface area just to *name* which implicit receiver a
   call means.
3. **Context receivers → removed.** Kotlin's experimental multi-receiver
   `context(T)` (ambient implicit receivers, Scala-`given`-style) was **deprecated
   and removed (~2.1.20) and replaced by *context parameters*** (KEEP-367), which
   are **named, not anonymous `this`**. The stated reasons: with several implicit
   receivers a bare call could resolve to any of them; they could not be named or
   passed on; and they conflated "depends on" with "extends." The fix walked
   *back* from anonymous implicit receivers toward **named** context.

The lesson is precise, and it is *not* "no receivers": the **single** builder
receiver (`T.() -> R`) was kept — it is loved and stayed. What was removed is a
**stack of ambient** implicit receivers. So Kotlin independently validates two of
this TIP's calls: **single context / innermost only** (a chain of implicit
receivers is the thing that blew up), and **prefer named parameters for ambient
*context*** (logging, transactions) over yet another implicit `this`.

### Summary of the field

| Approach | Receiver surface | Bare-name resolution | Main regret |
| --- | --- | --- | --- |
| Kotlin `T.() -> R` | implicit `this` | static, receiver-first | nested-receiver leak → `@DslMarker`; context receivers later removed |
| Groovy delegate | implicit | **dynamic** strategy | unanalysable; Gradle pain |
| Ruby `instance_eval` | implicit (`self` rebound) | dynamic + `method_missing` | invisible `self` switch |
| Scala `?=>` | implicit (given/using) | static, by type | implicits' action-at-a-distance |
| JS `with` | bare names | **dynamic, undecidable** | deprecated outright |
| C# init / extension | partial | static | no general nested DSL |
| **VB** `With` | **leading `.`** | n/a (dot-marked) | minimal — but only because `.` isn't also chaining |
| Pascal/D `with` | bare names | static (ambiguous) | field-vs-outer-var captured silently |
| Lua `_ENV` | bare names | dynamic (table) | perf; sandbox foot-guns |
| Smalltalk cascade | none (`;` separator) | n/a | none of note |
| Swift / F# builders | **none** | n/a | opaque errors, slow compiles |

The pattern is sharp: **every regret is about bare-name resolution** — what an
unqualified identifier inside the block means. The *ergonomics* (reach the
builder without naming it) are loved everywhere; the *cost* is always "I can no
longer tell what a plain name refers to." Two data points tell Tel how to keep
the first and refuse the second: **VB** shows a marker *can* fix it (but its
marker, `.`, is one Tel has already spent), and the **bare-name `with` family**
shows the regret comes specifically from *silent* capture and *scope chains* —
both of which Tel's **no-shadowing** rule and **innermost-only** rule remove
without needing any marker at all.

## Proposed direction for Tel

**A receiver closure is a method with `this` rebound to the builder, governed by
the rules methods already have.** Take Kotlin's typed, statically-resolved
receiver, but reach it the way Tel reaches `self` — by **bare name**, not a dot —
and lean on Tel's existing **no-shadowing** and **innermost-only** rules to erase
the bare-name-resolution regret without any marker.

- **The receiver lives in the parameter type, not the literal.** A block
  parameter declares its receiver with the method-as-value shape from
  [function types](../05-types/05-function-types.md): `Html.fn() : Unit` is "a
  block whose `this` is an `Html`, taking no other arguments." The lambda
  *literal* stays a bare param-less `{ … }`; the receiver is entirely a property
  of the signature:

  ```tel
  fn html(body: Html.fn() : Unit) -> Html { ... }
  ```

- **Reach the receiver by bare name, exactly as a method reaches `self`.** Inside
  the block, `title("Hi")` is `this.title("Hi")` and `charset` reads the
  receiver's field — the **same** rule a method body uses
  ([method-syntax](../09-functions/08-method-syntax.md): inside an `impl`,
  `apply(rate, total)` is `self.apply(self.total)`), with `this` rebound to the
  builder. This is the unifying claim: a receiver closure and a method are *one
  concept* — a function with a context — so there is **no new resolution rule and
  no new implicitness**, just the method rule applied to a block.

  ```tel
  html {
      let now = clock.now()    # `clock`, `now` — plain lexical locals
      head { title("Report at " + now.iso()) }
      body {
          h1("Sales")
          table(rows)          # `rows` lexical; `table` is the receiver's
      }
  }
  ```

- **The leading `.` is left to chaining.** Tel uses a line-leading `.` to continue
  a method chain; reusing it for the receiver would make `html { .head() /
  .body() }` parse as the single chain `body(head(it))`, not two statements. So
  the receiver does **not** take the dot. (This reverses the earlier draft; see
  the *Revision note* in the Summary.)

- **Explicitness is set at the declaration.** Omit the receiver — a bare `{ … }`
  block — and `this` is implicit, with bare-name resolution on; the `self` /
  `this` keyword names it when you must (to disambiguate or pass it along). Give
  it a name — `|h| { … }` — and reaching it is explicit (`h.title`), with bare
  names **not** resolving through it. The *named* form is exactly how you reach an
  outer builder from an inner block (switch that one level to `|h| { … }`),
  keeping the cross-level reach visible rather than ambient. (`TODO(open):`
  whether the implicit form is plain omission or a visible placeholder such as
  `_` / `self` at the declaration — and the exact named-binding form.)

- **One context, innermost only.** At most one `this` is active; there is **no
  outer-receiver scope chain** — the hole `@DslMarker` was invented to plug, and
  the very thing Kotlin's context receivers were removed for. `title` inside
  `head { … }` is the head builder's, full stop. A nested block's `this` shadows
  an enclosing method's `self`; the outer one is reached only by an explicit name
  (you cannot have *both* an implicit method `self` and an implicit receiver
  `this` at once — there is one slot).

- **No-shadowing replaces the marker.** The old draft paid a leading `.` per call
  to keep bare names off the receiver. Instead: bare names resolve lexical-first,
  *then* through `this` (the method rule), and **a bare name that collides with a
  context member is a compile error** demanding qualification. So adding a method
  to a builder can never *silently* capture a name — the JS-`with` / Pascal /
  Ruby regret (silent capture) becomes a loud error, and "what does this name
  mean" still has one answer, with no per-call ceremony.

- **Receiver-ness is orthogonal to escape-ness.** A receiver closure is an
  ordinary value: it can be stored, returned, or run later, because binding a
  context to a block says nothing about control flow. This is the clean line
  between this TIP and TIP-0009: **a receiver closure may escape; an inline block
  may not.**

- **Reject dynamic resolution** (Groovy `resolveStrategy`, Ruby `instance_eval`,
  JS `with`). The receiver type is known statically from the parameter signature;
  bare-name and `this.method` resolution happen at compile time, fully
  IDE-navigable. No runtime strategy, no `method_missing`, no environment swap.

This keeps the maxims intact: *no implicit anything* (no *new* implicitness — a
method body already resolves bare names through `self`; collisions are errors,
not silent), *surprise is a cost* (a reader can always answer "what does this name
mean", and the `.` keeps its one established meaning), *one good way* (a receiver
closure *is* a method — one concept, one rule, no second closure kind, no
`@DslMarker`, no dynamic strategy), and *readability over writability* (the block
reads like its domain vocabulary, with no per-call dot fighting it).

This also **requires revising [method syntax](../09-functions/08-method-syntax.md)**:
its current "a leading `.` may stand in for `self.`" must be dropped, since the
leading `.` is chaining. Disambiguation there is handled the same way — by
no-shadowing and the explicit `self` keyword, not a dot.

## Synergy with inline lambdas (TIP-0009)

This is the heart of the user's question — and the answer is **orthogonal but
co-designed, not merged.** Receivers and `inline` are two independent axes:

- **Receiver = *context*.** What the block's `this` (and so its bare names) is —
  a *name-resolution* / scope property.
- **`inline` = *control flow*.** Whether a marked `outer return` / `outer break`
  can leave the declaring function, plus zero-allocation splicing — a
  *control-flow* property.

All four combinations are real and useful, which is the proof they are separate
features:

| | plain (local return) | `inline` (`outer` powers) |
| --- | --- | --- |
| **no receiver** | `keep \|it\| { … }` value lambda | `with_lock(m) { … }`, `repeat(3) { … }` |
| **receiver** | escaping builder you store/return | the *full* DSL: `html { … }` |

The bottom-right cell is why the question is worth asking: the **best** DSLs want
*both*. A markup or test DSL wants the receiver for bare `div` / `expect`
ergonomics **and** inline control flow so a guard can bail out of the *enclosing*
function mid-build:

```tel
fn render(rows: List[Row]) -> Html {
    html {                                  # receiver: bare head/body in reach
        head { title("Report") }
        body {
            if rows.is_empty() {
                outer return empty_page()   # inline: leaves `render`, not the block
            }
            h1("Sales")
            for r in rows { row(r) }
        }
    }
}
```

Here the receiver does the *scope* work (bare `head`, `body`, `h1`, `row`) and
`inline` does the *control-flow* work (`outer return`). Neither subsumes the
other; gluing them into one feature would force every receiver to be inline
(losing the escaping-builder cell) or every inline block to carry a receiver
(meaningless for `with_lock`). So:

- **Keep them separate features**, designed to interlock.
- **The one coupling point is escape-ness.** A receiver lambda that is *also*
  inline must obey TIP-0009's non-escaping condition to keep its `outer`
  powers — you cannot `outer return` into a frame that has gone. A receiver
  lambda that escapes is simply an ordinary (local-return) value that happens
  to carry a bound context. Escape-ness — already tracked — is the single
  property that decides, so there is still no second closure kind.
- **The three directions read cleanly:** a **bare name** is *here* (a lexical
  local) or, failing that, *into* `this` — the method rule; **`self`** names that
  context explicitly; **`outer x`** reaches *up* into the declaring function. None
  touches the `.`, which stays chaining. Three directions, no marker collision —
  the dividend of designing them as orthogonal axes rather than one fused "DSL
  block".

So: the user's instinct that receivers are "much better for DSLs" is right — but
it is the *receiver* doing the DSL-ergonomics work, with `inline` adding
control-flow capability on top. Tie the two together in *documentation and worked
examples*, not in the *language feature*.

## Open questions

- `TODO(open):` **how a receiver is declared on the parameter — confirm the
  `Type.fn() : R` spelling.** This TIP proposes the method-as-value shape from
  [function types](../05-types/05-function-types.md) (`Html.fn() : Unit`).
  Confirm it reads unambiguously when the block *also* takes ordinary
  parameters (a receiver *and* a `|x|`), and that it composes with the trailing-
  block grammar. Alternative considered: a `self:`-named first parameter
  (`fn() : Unit` where the first slot is spelled `self`) — rejected as less
  visibly "this is a receiver block" at the call's type.
- **Receiver-block call spelling — resolved: `receiver.closure()`.** A
  receiver-closure parameter is a *value bound to a local name*, never a method of
  the receiver type (a method is just UFCS over a free function; a closure value
  is not one). So `recv.run()` ≡ `run(recv)` with `recv` as the block's `this`,
  and there is no clash with a method `run` on `recv`: the parameter is the
  in-scope lexical `run`, and no-shadowing makes a same-named method a conflict
  rather than a silent alternative.
- **Implicit-context — resolved: the *type* carries the explicitness, not a
  per-literal token.** "No receiver vs implicit receiver" and "where modifiers go"
  are answered by the parameter *type*: `Fn() -> R` has no receiver, `Recv.fn() :
  R` has one, and a modifier rides the type (`!Recv.fn()` for a `uniq` receiver).
  The call-site literal stays bare — `{ … }` is implicit `this`, `|h| { … }` is an
  explicit named receiver — because requiring a token (`_` / `implicit`) per
  literal would force `html |_| { head |_| { … } }` and destroy the nested-DSL
  readability the feature exists for. `TODO(open):` if a *visible* literal marker
  is still wanted, prefer `|self| { … }` (binds the context as `self`,
  self-documenting, with a modifier slot `|uniq self|`) over `_` / `implicit`.
- `TODO(open):` **reaching an outer receiver.** Innermost-only is the default;
  reaching an enclosing builder requires switching that level to a named
  `|h| { … }` lambda. Confirm this is enough in practice (Kotlin needed
  `this@label`; Tel's bet is that explicit binding at the one site that needs it
  is clearer than a labelled scope chain) and pin the exact binding form —
  including whether a block can take *both* an implicit receiver and a `|name|`
  binding of that same receiver for the rare "pass the whole builder along" case.
- **Method-syntax revision — done.** The "leading `.` stands for `self.`" rule in
  [method syntax](../09-functions/08-method-syntax.md) has been dropped (leading
  `.` is chaining; reach `self` by bare name or the `self` keyword), and that
  chapter's receiver section now points here. `TODO(open):` still verify the
  in-method case against the [shadowing](../06-bindings-and-scope/04-shadowing.md)
  rule — a receiver block inside a method has one context (the block's `this`
  shadows the method's `self` for the block's extent; the outer `self` is reached
  only by an explicit name).
- **Inline + receiver escape-ness — resolved: non-escaping, same rule.** A
  receiver block passed to an `inline` parameter is checked non-escaping exactly
  like a plain inline block. Non-escaping is a *control-flow* soundness condition
  (you cannot `outer` into a frame that has gone); the receiver is an orthogonal
  axis and does not change it.
- **dataframe reads are receiver closures — resolved.** The
  [TIP-0008](0008-named-axis-dataframes.md) table carrier's value-position reads
  (`filter { status == … }`, the RHS of `extend(total = price * qty)`,
  `agg`/`pivot` reducers) are receiver closures, which is *why* columns are bare
  names. The receiver is **whatever the operation iterates**, not always "the
  row": `this` is a **row** for the per-row ops (`filter`/`extend`/`map`, so a
  bare name is a scalar field) and a **group** for the aggregations (`agg`/`pivot`
  cells, so a bare name is a `Column`). TIP-0008 commits to **one** lambda flavor
  (this general receiver closure) for *all* reads — there is no separate "blessed
  column-expression special form" — with **no structural no-row-allocation
  guarantee**; the row inliner is expected to vectorise the common elementwise
  case instead. Only the *labels* (schema-position column names) stay magic. See
  TIP-0008 *The governing principle* and *One lambda flavor*. `TODO(open):` confirm
  the query carrier (open in TIP-0008) as a concrete receiver+transform customer.
- `TODO(open):` **does the receiver participate in trait/method resolution like
  any other value?** A bare `method()` (i.e. `this.method()`) should resolve
  through the same trait machinery as `x.method` ([overloading and dispatch](../09-functions/09-overloading-and-dispatch.md)).
  Confirm there is genuinely no new rule — only a changed `this` — and that
  extension-style methods on the receiver type behave identically to a direct
  call.
- `TODO(open):` **stdlib DSL customers.** The [Tel-as-data](../17-standard-library/18-tel-as-data.md)
  typed DSL and [`log.sub(...) { … }`](../17-standard-library/14-observability-and-logging.md)
  are the first consumers; a test-DSL and a data-format builder are likely next.
  Decide which ship in 1.0 and whether any need the receiver+`inline`
  combination from *Synergy* (vs receiver-only), so the feature is justified by
  real callers rather than the `html` toy.
- `TODO(open):` **1.0 inclusion vs the method-syntax lean.** [Method
  syntax](../09-functions/08-method-syntax.md) currently leans *receiver /
  context functions: not in 1.0*. Unifying receivers with the existing method
  `self` mechanism (no new resolution rule) removes the cost that drove that
  lean — so re-decide 1.0 inclusion here, jointly with TIP-0009 (the DSL story is
  weaker if only one of the two ships).
- `TODO(open):` **trailing block that takes a parameter.** Receiver/builder blocks
  are param-less `{ … }`; a block that needs a bound parameter uses the `\{ … }` /
  `|x| { … }` *trailing* form (e.g. the router's `|req| …` handlers). Pin that
  trailing-block grammar in
  [function application](../07-expressions/06-function-application.md) — including
  a receiver block that *also* binds a `|name|` — so the param-less and
  param-bearing trailing forms compose without ambiguity.
