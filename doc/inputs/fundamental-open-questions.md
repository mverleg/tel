# Most fundamental unresolved questions

A scan of the ~700 `TODO(open):` markers across `book/src/`. Most are local
(syntax spellings, stdlib shape, tooling). These two are *fundamental*: each
gates large parts of the language, and several other open questions are
downstream of them. Ranked by how widely a decision cascades.

(A third — the ownership / mutability model — was resolved by the now-Accepted
[TIP-0001](../book/src/tips/0001-mutability-and-borrowing.md): mutability is
both a type-level property (`!T`) and a binding modifier (`uniq`), `mut` is the
single derived root of affinity, and freeze (`finish()`) is a library
convention with compiler help. It is removed here; its one remaining tail —
whether immutable graphs with cycles can be built at all — survives in
`TODO.md`.)

## 1. How far the type system reaches (value-dependence)

Does Tel stay a conventional generic language, or edge toward dependent types?
Several open markers point the same direction:
- **Const generics — adopt or reject?** The *mutability-as-const-generic*
  variant is now moot — [TIP-0001](../book/src/tips/0001-mutability-and-borrowing.md)
  settled mutability with the `!` type sigil and the `uniq` binding, not a const
  generic — but const generics *in general* (a value parameter a type depends
  on) remain undecided.
  ([`05-types/07-generics.md`](book/src/05-types/07-generics.md))
- **Unify type parameters, const generics, and ordinary parameters** into one
  mechanism, or keep them separate? (same file; current lean: separate.)
- **"All values are also types."** Adopted only for data-less types so far; a
  general value-as-type machinery is powerful but "edges toward" dependent
  typing. ([`05-types/01-type-system-overview.md`](book/src/05-types/01-type-system-overview.md))
- **Nominal vs structural** reach — whether shared methods/traits across union
  members auto-expose, and whether records ever match structurally. Also flagged
  as a philosophy gap. (same file.)

Why fundamental: this defines the expressiveness/complexity ceiling of the type
system and weighs directly against the "frozen, conservative" and compile-speed
priorities. It also feeds into refined types and units.

## 2. How capabilities / effects are plumbed

The effect/capability mechanism is unified (panic and allocation are ambient
capabilities; `pure`/`total` opt out), but the *implementation surface* is open:
- **Runtime values vs compile-time const-generic (`comptime`-style)?** Same
  source-level surface; the difference is codegen cost vs ABI uniformity, and
  the cost of const-generics on every type and function signature.
  ([`05-types/05-function-types.md`](book/src/05-types/05-function-types.md))
- **Is "effect" even a distinct concept**, or are `panics`/`allocates`/`pure`
  just inferred function properties? (same file.)
- **How effects interact with dynamic dispatch** — trait bounds must
  over-promise the effect set at a `dyn` boundary. (same file.)

Why fundamental: this decides what is visible in every function and trait
signature, how the compiler threads capabilities through generated code, and the
ABI. It also touches #1 — const-generic plumbing is the same machinery the type
system would need for const generics generally.

---

Honourable mention (chapter-local but pervasive): the surface **syntax is
largely unpinned** — function-type and lambda spellings, mutable-binding
spelling, `spawn`/`join`/task-handle names. Pervasive but cosmetic; resolving it
does not change the design, only its notation.
