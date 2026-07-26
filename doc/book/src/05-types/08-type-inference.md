# Type Inference

Type inference lets a script omit type annotations the compiler can work out for
itself. Tel wants the convenience — annotations everywhere is noise — but
inference is in tension with Tel's top priority, **stability**, so it is kept
deliberately limited.

## What — local inference, explicit public signatures

The model:

- **Local bindings infer freely.** A `let` without a type annotation takes the
  type of its initialiser.
- **Public signatures are explicit.** A function, type, or other item that other
  code depends on states its parameter and return types in full. Inference does
  not cross these boundaries.
- **Inference never crosses a file boundary.** Anything visible outside its
  file has an explicit signature — parameter and return types written, effects
  declared or defaulted to may-panic/may-allocate (see
  [function types](05-function-types.md#effects-belong-on-the-function-type)).
  Within a file, inference is free.
- **Inference is local and one-directional-ish.** Tel does not attempt
  whole-program, cross-function inference. Type information mostly flows from
  declarations and known operations into expressions, not the reverse.

```tel
let count = orders.len()          // inferred: Int64
let total: EuroAmt = sum(amounts) // local annotation, fine either way

# Public — types are written out, not inferred:
fn score(an_order: Order) -> Result[Score, Reject] { ... }
```

## Inference only produces nameable types

Inference is a convenience for *omitting a type you could have written* — never a
way to obtain a value of a type you *could not* write. Concretely:

**The compiler will only infer a type that is [visible](../11-modules-and-packages/03-visibility.md)
at the point where the inferred result is used.** If the only type inference
could assign there is one not visible at that point — a `private` type leaking
out of its module, or a merely crate-visible type escaping the crate — that is a
**hard error**, not a silently-inferred unnameable type. The author must either
make the type reachable or restructure so a nameable type is inferred.

This is the rule that makes the explicit-signature requirement
([above](#what--local-inference-explicit-public-signatures)) impossible to dodge.
Forbidding inference *in* an exported signature, on its own, leaves a workaround:
let a non-exported helper infer a private type and let that type flow outward
through call sites and local bindings until it lands somewhere a consumer can
reach but cannot name. Capping inference at the visibility boundary shuts that
door — every type inference hands you is one you were entitled to name yourself,
so it cannot widen the effective visibility of any type.

The invariant: **anything inference gives you, you could have spelled by hand.**
It pairs with the visibility hard-error — an exported signature may only rest on
reachable types — so neither the *declared* surface nor the *inferred* interior
can expose a type a consumer cannot name.

```tel
private type Ledger = ...

# OK — inferred type `Int64` is visible here.
let count = entries.len()

# Error — inference would assign the non-visible `Ledger`. Name a visible
# type, export `Ledger`, or keep the binding inside `Ledger`'s module.
let snapshot = build_ledger()
```

This stance holds **for now, because Tel has no unnameable types yet.** Today
every type has a spelling, so "infer only visible types" is total: there is no
legitimate reason to infer a type the author could not write. When
[opaque types](../11-modules-and-packages/01-modules.md#module-level-apis) arrive
— the deliberate "expose an unnameable type by handle" mechanism — this rule will
gain a single, explicit exception for them; an *accidental* non-visible inference
stays an error.

TODO(open): confirm the precise interaction with opaque types once they land —
lean: an opaque handle is the only sanctioned way to carry a non-visible type
through an inferred position; a bare `private`/unexported type there remains an
error, mirroring the [visibility hard-error](../11-modules-and-packages/03-visibility.md#what-visibility-is).

## Why — stability caps how clever inference may be

This is the load-bearing rationale, and it is unusual enough to spell out.

**Inference rules are part of a script's meaning.** Two things follow:

1. **Cleverer inference is a hidden breaking change.** If a later compiler infers
   types more aggressively, an existing script can change meaning — or stop
   compiling — without its source changing. For a language that promises "same
   code, same results, decades later", inference rules can therefore only ever
   get *more* generous, never different — and even that is risky. Full
   (e.g. cubic) inference is also an easy way to break backwards
   compatibility.

2. **Library changes can perturb inference.** Adding a new type, or a new trait
   impl, can make the compiler suddenly infer that type where it previously
   inferred another — the way introducing a supertype shifts Java's inference.
   Limited, local inference keeps this blast radius small.

The file-boundary rule serves a second master, compile speed: because
everything visible outside a file is explicitly declared, another file's
checking depends only on this file's *declarations*, never on its bodies. A
body edit can never change what other files see, every file's exported surface
can be read straight off its parse (in parallel, before any checking starts),
and an incremental compile re-checks the smallest possible set (see
[the compiler](../18-tooling/01-compiler.md#two-compile-modes)).

There is also a plain compile-speed argument: full inference can be cubic or
worse, and fast clean compiles are a stated developer-productivity goal. A
Java-style limited scheme is cheap.

So Tel takes a **Java-style limited inference**: enough to drop obvious local
annotations, not so much that the meaning of a script depends on a sophisticated
solver. This is a case where a *less* powerful feature is chosen on purpose.

TODO(open): biunification (subtype inference by example) is a candidate
*algorithm*. That is an implementation-strategy question — it
belongs in `impl-notes/`, and only matters insofar as it stays within the
limited, stable surface this page commits to. The user-facing decision is
"limited inference"; the algorithm that delivers it is separate.

TODO(open): "type annotations fully optional for end users (not libraries)"
is a recurring idea — i.e. a script author may omit *all* types while a
library author may not. The file-boundary rule above settles the mechanism
with no special case: a single-file script has no cross-file surface, so
everything in it is local and inferable. What remains open is only the
multi-file script — items used from another file need signatures like anyone
else's. Decide whether that is acceptable script UX or scripts need more.

## Suggesting concrete types

Inference can hide a type that a reader, or a downstream call, actually needs to
know. The Rust example: a value inferred as a concrete `List[T]`
works with `.collect()`, but the *same code* generalised to an `impl AsRef<[T]>`
parameter does not — even though the relationship holds — because inference no
longer sees the concrete type. A library change that generalises a signature can
thus break callers purely through inference.

Two mitigations, both kept in mind for Tel:

- A way to **suggest or pin a concrete type** at a use site, both as inference
  guidance and as documentation for the reader — making the inferred type
  visible and stable rather than implicit.
- IDE support that **displays inferred types inline** (and lets the reader fold
  them away), so "limited inference" does not mean "the reader is left
  guessing". The IDE is a first-class reader (see
  [`../02-philosophy/02-maxims.md`](../02-philosophy/02-maxims.md)).

TODO(open): exact spelling and rules for a "suggest concrete type" annotation,
and whether it is purely advisory or load-bearing for inference.

## Two compile modes

Two compilation modes interact with inference and error
quality:

- a **clean** mode optimised for fast bulk compilation, and
- an **incremental / friendly** mode with fine-grained caching, recoverable
  errors, and rich metadata, used for the LSP and for re-compiles.

Inference must produce the *same* result in both — the modes differ in speed and
diagnostics, never in the types they assign. This is an implementation concern;
detail lives in `impl-notes/`.

## See also

- [Generics](07-generics.md) — what gets inferred for type parameters.
- [Type System Overview](01-type-system-overview.md).
- [Priorities — stability](../02-philosophy/01-priorities.md).

TODO: review
