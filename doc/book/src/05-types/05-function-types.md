# Function Types

A function is a first-class value with a type that describes its signature.
Function types let scripts pass behaviour around — map/filter/reduce, callbacks,
handlers, comparators — without inheritance or virtual dispatch.

## What — anonymous, structural function types

A function type is written with the `fn` keyword, its parameter types in
parentheses, and its return type after a colon — **the type mirrors the value**,
with the lambda body replaced by `: <return type>`:

```tel
# A function from Int64 to Int64.
# Value:  fn(x) { x + 1 }   →   Type: fn(Int64) : Int64
let inc: fn(Int64) : Int64 = fn(x) { x + 1 }

# A two-argument function taken as a parameter.
fn apply(f: fn(Int64, Int64) : Int64, a: Int64, b: Int64) : Int64 { f(a, b) }
```

This follows one rule used throughout Tel: **a type is written like a value, with
each literal replaced by its type** (a tuple value `(1, 3, x = 8)` has type
`(Int64, Int64, x = Int64)`; see
[type system overview](01-type-system-overview.md#how-types-are-written)). For a
function, the value's *body block* is the part replaced — by `: <return type>`.

Function types are **structural and anonymous**: any function whose parameters
and return type match `fn(Int64) : Int64` *has* that type. Unlike records and traits,
there is no declared `MyFn` that a value must be branded with — the shape *is*
the type. Function types behave the way Java
functional interfaces should have, while struct types are nominal.

A function value can be:

- a **named function** — `fn add(a: Int64, b: Int64) : Int64 { a + b }`,
- an **anonymous lambda** — `fn(x, y) { x + y }` for two or more arguments, or the
  short `{ ... }` block (using `it` for the single argument) for zero- or
  one-argument closures,
- a **method on a type** used as a value — `Order.total` taking the receiver as
  the first argument.

All three share one type system: a value of type `fn(A, B) : C` is any of these
shapes that takes an `A` and a `B` and returns a `C`.

A method-as-value type carries its **receiver in front of the dot** —
`Order.total` is "the `total` method of `Order`, as a value," the receiver
written ahead of the `.`. The same shape declares a **receiver block parameter**
for a builder DSL: `Html.fn() : Unit` is "a block whose `this` is an `Html`,
taking no other arguments," so inside the block the builder's members are reached
by bare name (the method rule — see
[method syntax](../09-functions/08-method-syntax.md#receiver-closures-for-dsls)).
The receiver is entirely a property of the *type*; the block literal stays a bare
`{ … }`, and a modifier rides the type (`!Html.fn() : Unit` for a `uniq`
receiver). See [TIP-0010](../tips/0010-lambda-receivers-and-builder-dsls.md).
`TODO(open):` confirm the `Type.fn() : R` spelling reads unambiguously when the
block *also* binds an ordinary `|x|` parameter.

**Settled:** the function-type spelling is `fn(args) : ret`. Earlier sketches
weighed `(Int64) -> Int64` (Rust/Kotlin), `Int64 -> Int64` (ML/Haskell), and
`Fn[(Int64), Int64]`; the `fn(...)`-prefixed form wins for three reasons:

- The leading `fn` keyword gives the parser an unambiguous first token, keeping
  the grammar left-recursive with lookahead 1.
- It disambiguates a no-arg or one-arg function type from a bare parenthesised
  group — `fn() : Int64` vs `()`, `fn(Int64) : Int64` vs `(Int64)` — without the `->`
  an arrow form would need.
- It matches the *value* syntax for a multi-argument lambda (`fn(a, b) { ... }`),
  so the type and the value read the same way.

This same marker is what lets Tel reuse the tuple/row shape for argument lists
(see [tuples as argument bundles](04-tuples-and-arrays.md#tuples-as-argument-bundles))
without inheriting Swift's tuple-vs-argument-list conflation: a function type is
always `fn(...)`, never a bare `(...)`, so `fn(A, B) : R` (two parameters) and
`fn((A, B)) : R` (one tuple parameter) are visibly different — the very
distinction that, unmarked, cost Swift years (see
[arguments are tuple-shaped](04-tuples-and-arrays.md#arguments-are-tuple-shaped-kept-distinct-by-the-fn-marker)).

The `|args| ret` (pipe) form is **rejected** for function types: the `|` collides
with the union `|` — `fn(Int64 | text) : Int64` would be unreadable spelled
`|Int64 | text|` — and a leading `|` is a poor first token for the parser. The
return uses a colon, not an arrow, so it lines up with parameter annotations
(`a: Int64`) and with the type-mirrors-value rule above.

TODO(open): are parameter **names** part of a function type? The binary section
model answers it per-section: **positional** param names are *not* part of the
type (function types are structural/anonymous, and positional params may be
renamed without breaking callers), but **keyword-only** param names *are* —
callers match them by name, so a keyword type must spell them:
`fn(Text, *, retries: Int64) : Conn`. The default *value* is callee implementation
and stays out of the type; only the *optionality* of the keyword slot is type-level.
This makes the function type a row (`name-erased positional, name-significant
optional named`), the same shape as a tuple/record (see
[tuples-as-argument-bundles](04-tuples-and-arrays.md#tuples-as-argument-bundles));
rows are compared by exact shape, with no implicit row subtyping.

**Settled:** lambda spelling. A closure taking **two or more** arguments is
written like a function without a name — `fn(x, y) { x + y }`. A closure taking
**zero or one** argument can be written as just a block — `{ it + 1 }`, using
`it` for the single argument — usable anywhere an expression is expected (see
[closures and lambdas](../09-functions/06-closures-and-lambdas.md)). The `|x|`
(Rust) and `\x -> body` (Haskell) forms are not used.

### Grouping a function return type

The return type after `:` extends as far to the right as it can. Every type form
is **self-delimited** — its own syntax marks where it ends — *except* a function
type, whose trailing `: T` has an open right edge:

- atoms are one token: `Int64`
- tuples are parenthesised: `(Int64, Text)`
- unions are parenthesised where nested: `(Int64 | Text)`
- generics close with `]`: `List[Int64]`
- a function type does not close itself: `fn(Int64) : T` runs on

So when a return type is itself a function type, group it with ordinary
**grouping parentheses** `( )` — the same parens that group any expression or
type. There is no special bracket for this:

```tel
fn() : Int64                          # bare: atom is self-delimited
fn() : (Int64, Text)                  # bare: tuple closes itself
fn() : (Int64 | Text)                 # bare: union closes itself
fn() : List[Int64]                    # bare: generic closes itself
fn(Int64) : (fn(Int64, Real64) : Int64)     # grouped: the return is a function
```

`(fn(Int64) : Int64)` is **grouping, not a one-tuple**: a tuple needs a top-level
comma or named field (`(x,)`, `(a = 1)`), and this has neither, so the existing
[tuple-vs-grouping rule](04-tuples-and-arrays.md#literal-forms-grouping-vs-tuple)
already tells them apart with no new syntax. The trigger is precise — *the return
is a function type* — not a vague "wrap complex types": a function type is the one
form whose extent the reader cannot otherwise see. A function type used as an
**argument** or **tuple element** needs no grouping, because the surrounding `,`
or `)` already bounds it (`fn(fn() : Int64, Int64) : Int64`).

## Methods, fields, and the unified call story

A field of function type and a method should be callable
the same way:

```tel
let f: fn(Int64) : Int64 = inc
f(3)              # call

order.total()     # method
order.total       # if `total` is a field of function type, also callable
```

When a field name and a method name collide, the *method* should win — so a
record can later replace a stored field with a computed property without
breaking callers (see the same point in
[records](../10-data-modelling/01-records.md)). The flip side: a method's name
is reserved for that purpose on the type, so a record cannot quietly shadow it
with a field.

TODO(open): field-vs-method precedence and how it composes with externally
defined functions used in method position (uniform function call). Tied to the
method-resolution story in the syntax chapter.

## Inference and function types

Type inference fills in lambda parameter types from context where unambiguous:

```tel
let nums = [1, 2, 3]
let doubled = nums.map({ it * 2 })        # it: Int64 inferred from List[Int64]
```

What inference deliberately does **not** do is propagate function-type
information *backwards* across function boundaries. Public signatures are
explicit (see [`08-type-inference.md`](08-type-inference.md)), so a function
parameter typed `fn(Int64) : Int64` is always written out.

There is a related concern: when a function value is stored in a struct,
the function's anonymous type cannot easily be *named* in the struct's
declaration without making the struct generic. Closures inherit this — their
types are unnameable. The escape hatches in other languages (`Box[dyn Fn]`,
generics) both have costs.

TODO(open): how Tel stores function values in records / collections. Options:

- **Generic over the function type.** `struct Handler[F: fn(Event) : Unit]` —
  monomorphises, fastest, but every distinct lambda is a distinct concrete
  type, and the struct type itself becomes generic-everywhere.
- **Dynamic dispatch through a named function type.** A `Fn`-shaped trait
  bound, similar to Rust's `Box[dyn Fn]` — uniform type, indirect call.
- **Limit function values to the stack** — no storing closures in long-lived
  data. Simple but rules out callback registration.

Decide once the trait / dyn story is settled; the choice has visible perf
consequences for embedded host callbacks.

## Effects belong on the function type

A function's *type* tells you its parameters and return — but useful properties
go further: can it panic? Can it allocate? Is it pure? These are treated as
**effects** that flow through function types and are inferred for concrete
functions *within a file*; across files they are declared or defaulted, and
they have to be written when a generic parameter must promise (or deny) them. (Touching I/O is *not* an effect — it is a capability
the host hands in; see below.)

```tel
# Sketch — syntax not settled.
# `pure` says this function has no effects beyond its arguments and result.
pure fn add(a: Int64, b: Int64) : Int64 { a + b }

# `panics` is an effect: this function may abort the task on some inputs.
# (There is no `throws` — errors are values, returned as `Result`.)
fn must_parse(text: Text) panics : Json { ... }

# A higher-order fn whose own effect set depends on its argument.
fn each[T](items: List[T], f: fn(T) : Unit) : Unit { ... }
# `each` inherits whatever effects `f` carries.
```

The committed intent:

- **A function is a function.** Tel does not split functions into *async* and
  *sync* colours (see [antifeatures](../02-philosophy/04-antifeatures.md)).
  Effects live in the type, not in a separate "colour" of function.
- **Inferred within a file; explicit at file boundaries and at bounds.**
  Inside a file, a concrete function that calls a `panics` function is itself
  `panics`, with no annotation. Anything visible *outside* its file declares
  its effects or gets the default (may panic, may allocate) — the same rule as
  parameter and return types (see
  [type inference](08-type-inference.md#what--local-inference-explicit-public-signatures)),
  and for the same reason: under the hood an effect *is* a signature
  ingredient — an ambient capability parameter the compiler injects (see
  [below](#ambient-capabilities-panic-allocation)) — so it is explicit
  wherever types are. A body edit therefore never changes the effect row other
  files see; a declared `pure`/`total` that the body exceeds is an error at
  the declaration, not at distant callers. A generic parameter written
  `fn(T) : U` infers nothing either way, since the actual function is unknown
  at the bound — the caller must spell the effect into the bound to require
  it.
- **Aliases for effect sets.** Combinations of effects (e.g. allocate + panic)
  should have names so signatures stay readable; these are
  *bound aliases*.

This is conceptually close to Koka/Effekt-style algebraic effects, but Tel
adopts only the bits that pay for themselves in an embedded scripting setting:
effects are *properties tracked on the type* — does it allocate, can it panic,
is it pure — not a handler/resume mechanism. There is **no `throws` effect**;
errors are values returned through `Result`-shaped types (see
[antifeatures](../02-philosophy/04-antifeatures.md)), not control flow. A panic
*aborts* the task rather than yielding to a resumable handler (see
[structured concurrency](../14-concurrency-and-parallelism/04-structured-concurrency.md)).
Multi-shot effects (generators, async green threads) and first-class resumable
handlers are a much bigger commitment and are rejected — see
[rejected re-invocable continuations](../14-concurrency-and-parallelism/03-async-and-function-colouring.md).

The effect alphabet Tel tracks (spelling aside):

- `panics` — the function may abort the current task on some inputs. This is
  *not* a resumable throw: Tel has no exceptions, and a panic ends the task
  (see [structured concurrency](../14-concurrency-and-parallelism/04-structured-concurrency.md)).
- `allocates` — may allocate on the heap. Useful for hot loops and
  compile-time evaluation.
- `pure` / `total` — no tracked effects; referentially transparent (and, for
  `total`, terminating on all inputs).

I/O, networking, time, and randomness are **not** effects. They are
[capabilities](../02-philosophy/03-features.md) — ordinary values the host
hands in as arguments. A function that needs the clock takes a `Clock`
parameter; there is no parallel `time` effect, because the capability is
already visible in the signature. This keeps *one* mechanism for the host
boundary (capabilities) instead of two (capabilities plus an effect row that
mirrors them).

A useful consequence: code reached through *syntax* rather than an explicit
call — an [operator overload](../04-syntax/04-precedence-and-associativity.md#operator-overloading),
indexing, iteration — takes **no capability parameters**, so it cannot do I/O.
`a * b` and `xs[i]` compute from their operands and never lazily load, log, or
hit the network; every outside effect on a line stays in that line's named
calls.

### Ambient capabilities: panic, allocation

`panics` and `allocates` are the two tracked effects whose mechanism is an
**ambient capability** — every running task has a current `Panicer` and a
current `Allocator`, and code that panics or allocates calls into them through
the compiler-injected ambient set. `pure` / `total` opt out of these ambient
capabilities for the function body.

Two rules of ambient capabilities are different from ordinary capabilities
like `Clock` or `Http`:

- **They resolve at the call site, not at closure-capture time.** A closure
  does not bake in *its* `Panicer` and carry it around; the closure runs
  against whichever ambient set is current at the call site. This is the
  algebraic-effect-handler discipline (see
  [async and function colouring](../14-concurrency-and-parallelism/03-async-and-function-colouring.md#effects-handlers-and-implicit-arguments)).
  Without this, a `Fn` that captured a panicking ambient and was later passed
  into a `pure` region could call panic anyway, breaking the `pure` guarantee.
- **The compiler threads them through generated code automatically.** A
  source-level function does not list `Panicer` / `Allocator` in its
  signature; the codegen pass adds them as implicit parameters wherever they
  are needed. The user-visible surface stays effect-shaped; the underlying
  mechanism is capability-shaped.

Users **cannot write their own allocators.** `Allocator` is a capability the
host hands in (possibly with quotas or a budget — *"this script gets at most 1
GB"*), not an interface user code implements. A custom-allocator API would be
impractical to support across Tel's target hosts (browsers, mobile sandboxes,
embedded VMs, Wasm without a system allocator) and would contradict
[no low-level machine access](../02-philosophy/04-antifeatures.md). A host may
swap the underlying implementation; a script cannot.

TODO(open): whether ambient capabilities are plumbed as **runtime values**
auto-injected by the compiler, or as **compile-time generic / `const`
parameters** in the Zig-`comptime` style. Both produce the same source-level
surface; the difference is codegen cost vs ABI uniformity, and the cost of
the const-generic form on type and function signatures. Investigate before
committing.

TODO(open): whether the small tracked set is worth being a distinct "effect"
concept at all, or whether `panics`/`allocates`/`pure` are better framed as
plain inferred function properties — given that the underlying mechanism is
already unified (ambient capabilities), "effect" may just be a name.

TODO(open): whether `allocates` earns tracking given it touches almost
everything, or is better left to implementation notes as a backend concern.

TODO(open): whether a crate may mark an effect (or capability) as **"auto"**
— implicitly available everywhere inside the crate, yet still required at the
crate's public API boundary. This would let internal code stay annotation-free
while keeping the boundary contract explicit. Unintegrated idea; decide whether
it earns a place or collapses into the ambient-capability mechanism above.

TODO(open): whether **task cancellation** is a third ambient-capability effect
in this family — a `Canceller` every task carries, observed at yield points —
or an explicitly-injected capability like `Clock`. It is effect-shaped (like
`panics`) and would ride the same compiler-injected, call-site-resolved ambient
set, but it collides with the "nothing is ambient" discipline. Decided jointly
with [TIP-0012](../tips/0012-task-cancellation-abort-and-shutdown.md).

TODO(open): how effects interact with dynamic dispatch. At a trait boundary
the concrete function is not known statically, so the bound must over-promise
(state every effect the implementations might use). Lean: yes, trait bounds
state their effect set explicitly; inferred-effects work only behind static
dispatch.

Effects are the **default**: a function may do whatever its body and arguments
allow, inferred, with no annotation. Purity is opt-in — `pure fn` (and
`total fn`) ask the compiler to *verify* the absence of the tracked effects.
This keeps everyday scripts annotation-free while letting code that needs a
guarantee request one; a pure-by-default rule would force an effect annotation
onto the majority of ordinary functions, against *readability over
writability*. The design is still specified pure-first — define precisely what
`pure` excludes — even though effects are the default in practice.

## Function-value flavours

Function values come in a few relevant flavours:

- **Call-once / linear.** A function the type system promises will be called at
  most once. Useful for initializers and resource-claiming callbacks (a
  `with_brand`-style scope is the canonical example). Per
  [TIP-0001](../tips/0001-mutability-and-borrowing.md) this is the `FnOnce`
  form, **inferred from captures**: a closure that consumes an **affine**
  capture is `FnOnce`; a closure that owns a **relevant** capture is itself
  relevant (it must be called, because calling it is how the capture is used).
  See [Substructural Types](../12-memory-and-runtime/08-substructural-types.md).
- **Inlineable / non-escaping.** A lambda the compiler knows does not escape
  the calling scope can be inlined, can `return` from the enclosing function,
  and avoids closure allocation. Roughly Kotlin's `inline fun`.

The call-once flavour surfaces in the type as `Fn(...)` vs `FnOnce(...)`;
inlineability stays a quality-of-implementation property.

A closure needs **no new substructural traits**: it is an anonymous record whose
fields are its captures, so `Alias` / `Discard` / `Send` / `Sync` derive by the
standard field rule (see
[Substructural Types](../12-memory-and-runtime/08-substructural-types.md)). The
one function-specific axis is **call multiplicity**, and it is cleanest to frame
it as the affine/relevant lattice *re-applied to the call itself*:

- **`FnOnce`** = affine-on-calls (call ≤ 1), inferred when a call consumes an
  affine capture (a move-self call).
- a **must-call** closure = relevant-on-calls (call ≥ 1): it captures a
  **relevant** value, so the closure is itself `¬Discard` and cannot be dropped
  unused — calling it is how the capture is consumed. This needs no third marker;
  it *is* the closure being relevant.
- both together = linear-on-calls (call exactly once): the one-shot
  resource-claiming callback (`with_brand`).

TODO(open): adopt the "affine/relevant re-applied to the call" framing explicitly,
so `FnOnce` (affine-call) and must-call (relevant-call) read as one idea rather
than two special cases. Confirm must-call needs no surfaced marker beyond the
closure being `¬Discard`. Keep "no `FnMut`" (capture mutation is a borrow-rule
property) and "inlineable" (a QoI property, not a trait).

TIP-0001 settles the surface: **two** function-type forms, `Fn` and `FnOnce`,
not Rust's `Fn`/`FnMut`/`FnOnce` triple. There is no `FnMut` — mutation through
a capture is handled by the borrow rules on the capture itself. The `once`
property is inferred from captures and only written in signatures that *require*
a one-shot callback.

**Decided** — call-arity follows capture consumption: a closure is `FnOnce` iff a
call **consumes** (moves out) one of its captures, otherwise `Fn`. Capture mode
is inferred (immutable → value snapshot; `uniq` → reusable `&!` borrow → `Fn`),
with an optional, overrides-only capture clause spelling `move <name>` /
`borrow <name>` as **keywords**. `move`-ing a value in and handing it away on the
call is what makes a closure one-shot. See
[`../09-functions/06-closures-and-lambdas.md`](../09-functions/06-closures-and-lambdas.md).

TODO(open): variance of function types. Standard rule: arguments are
**contravariant**, return type is **covariant** — `fn((A | B)) : X` is usable
where `fn(A) : X` is wanted, and `X` is usable where `(X | Y)` is wanted. Confirm it
lands as documented, given the union
subtyping in [`09-subtyping-and-variance.md`](09-subtyping-and-variance.md).

## Partial application and currying

Should Tel support a clean partial-application form,
shorter than `fn(x, y) { f(1, 2, x, y) }`? The everyday alternatives are:

- A lambda — `let g = { f(1, 2, it) }`. Always works, slightly noisy.
- A named "partial" syntax — sketched as `fn g(z) = f(1, 2)`. Concise but
  another surface to learn, and the meaning of an under-applied call site is
  not always obvious.

TODO(open): commit to or reject syntactic sugar for partial application. Lean:
no — lambdas already do the job, and a partial-application form complicates
overload resolution and the function-call grammar. "One way to do a thing" per
the [maxims](../02-philosophy/02-maxims.md).
[TIP-0006](../tips/0006-tuples-as-argument-bundles.md) considered reversing this
on the strength of the tuple/bundle grounding (a partial application as a *prefix
bundle*) and **decided to keep the lean** — splat ships, but partial application
stays a lambda for now.

TODO(open): function subtyping by *arity* — whether `f(a)` is a subtype of
`f(a, b = 2)`. Lean: no — default-argument
relationships should not show up as silent subtyping; if you want both shapes,
write two functions or use [optional arguments](../05-types/01-type-system-overview.md)
explicitly.

## A function value with no return

A function whose return type is the [never type](14-never-type.md)
(`Never`) is known to never return normally — it aborts, loops, or hands
control away. This composes with effects: `fn unreachable() : Never` and
`fn shutdown(reason: Text) : Never`. Code after such a call is unreachable,
and the compiler may use this in flow-sensitive checks (exhaustiveness, missing
return).

## Callables that are not functions

One floated idea is a `Callable` trait — any type with a defined `call` method is
usable in function position, the way Python's `__call__` works. The tricky bit
is that `call` does not have one universal signature, so a true `Callable`
ends up either curried, or with a single-tuple argument, or generic per
implementing type. It is doubtful this earns its weight.

TODO(open): adopt or reject a `Callable`-style trait. Lean: reject — functions
are first-class already; "one way to do a thing" prefers a single mechanism.
A type that wants to act like a function can expose a `.run(...)` method and
let callers spell that out, which also documents the call shape.

## See also

- [Type System Overview](01-type-system-overview.md).
- [Generics](07-generics.md) — generic functions and bounds.
- [Type Inference](08-type-inference.md) — how lambda parameter types are
  inferred.
- [Subtyping and Variance](09-subtyping-and-variance.md) — variance of function
  types.
- [Traits or Interfaces](../10-data-modelling/03-traits-or-interfaces.md) — the
  bound story.

TODO: review
