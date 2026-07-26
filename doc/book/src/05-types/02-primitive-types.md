# Primitive Types

The built-in types every Tel script can use without declaring anything. They
follow the same rules as user-defined types — see
[`01-type-system-overview.md`](01-type-system-overview.md) — so wrapping a
primitive in a [refined type](12-refined-types.md) loses no capability.

## Explicit-width numbers

**Decision.** Tel's numeric types are **named by their width**, and Tel
**commits to explicit widths everywhere**. There is no abstract `Int`/`Real`
whose size the host picks; a script always names a concrete, fixed-width type,
so the same name means the same range and the same overflow behaviour on every
host. This commits to Jordan Rose's "specify the widths" critique of Swift's
`Double`/`Float` naming;
a name like `Real64` says what it is, where `Real`/`Double` does not.

The only built-in numeric type that is *not* a fixed binary width is
**`Decimal`** (exact base-10, below), which exists precisely to escape one. An
arbitrary-precision integer is available too — `DynInt` — but it lives in the
[standard library](../17-standard-library/07-numerics-and-math.md), not among
the built-in primitives. Every built-in numeric type carries its width in its
name.

- **Signed integers** — `Int8`, `Int16`, `Int32`, `Int64`.
- **Unsigned integers** — `UInt8`, `UInt16`, `UInt32`, `UInt64`.
- **Floating point** — `Real32`, `Real64` (IEEE-754 single / double).
- **`Decimal`** — an exact base-10 number for money and other values where
  binary floating point drifts. Not a fixed *binary* width; see
  [Decimals and currency](#decimals-and-currency).
- **`Bool`** — `true` / `false`. No truthy/falsy coercion: only a `Bool` can be
  used where a `Bool` is expected. See also the
  [named-boolean two-variant enums](../10-data-modelling/02-union-types.md) for
  when a domain-specific yes/no reads better than a bare `Bool`.

Only widths that mainstream hardware and host runtimes support **ubiquitously**
are offered — 8/16/32/64. There is no `int128`, and no platform-dependent
`isize`/`usize`: a width that some hosts cannot represent identically would
break the determinism guarantee below.

### Behaviour is identical on every host

A fixed-width type has a fixed range and **fixed overflow behaviour on every
host**. A host whose hardware lacks a given width emulates it, but the
observable result of any numeric operation — the value produced, and whether it
errors — is fixed by the language and identical everywhere. This is the
[determinism guarantee](../02-philosophy/03-features.md): *one script, many
hosts, behaviour identical across them*. Explicit widths make this **easier**,
not harder — there is no "big integer here, 64-bit-with-abort there" ambiguity
to pin down, because the width is in the type.

### What a bare literal infers to

A bare `1` or `1.0` without annotation must resolve to *some* width.
**Decision:** the literal defaults are **`Int64`** and **`Real64`**. On the
64-bit hosts Tel embeds into these are the natural register width, and they give
the widest fixed range before a script has to think about overflow — an `Int32`
default would invite silent range surprises. The narrower types (`Int8`,
`Int16`, `UInt8`, …) and the standard library's unbounded `DynInt` are reached
for **deliberately** — wire formats, packed structs, byte buffers, or counts
that may exceed 64 bits — by annotating the binding or constructing the type by
name.

```tel
let n = 42            // Int64 by default
let r = 3.14          // Real64 by default
let b: UInt8 = 42     // deliberately one byte
```

The defaults exist only to keep throwaway arithmetic terse. **Domain values
should not stay bare** — wrap them in a newtype that names the concept and pins
the width (see below).

### Recommended: name the domain with a newtype

A bare width like `Int16` answers "how many bits", not "what is this". The
recommended style is to make the width an *implementation detail of a named
domain type* — a [newtype](12-refined-types.md):

```tel
type Age = newtype Int16   // 0…32767 is plenty of years; the choice is recorded
```

`newtype Age` is where domain knowledge ("an age is a smallish non-negative
count of years") gets translated into a hardware decision ("16 bits is more than
enough"). The width lives in one place, the name documents the intent, and the
compiler stops an `Age` being mixed with an unrelated `Int16`. See
[Refined and Newtype Types](12-refined-types.md) for the full story.

The arbitrary-precision integer is named **`DynInt`** (dynamically-sized
integer) and is a [standard-library](../17-standard-library/07-numerics-and-math.md)
type, not a built-in primitive. The distinct, non-`Int`-shaped name is
deliberate: it keeps the "explicit widths everywhere" message clean and stops
anyone reaching for unbounded heap arithmetic by habit when a fixed width would
do. Keeping it out of the primitives also means the core language is purely
fixed-width — `DynInt` is a value type built *on* the primitives.

TODO(open): integer overflow policy in full — error-and-abort vs checked
`Result` vs saturating. (Unbounded-by-promotion is off the table now that every
integer type has a fixed width.) Tied to the mutability/error chapters; the
maxim is "when in doubt, fail — fast and loud", so silent wraparound is out.
Whatever is chosen applies per width and is identical on every host; the
narrow-width types (`Int8`/`Int16`) make the overflow boundary something a
script meets in practice, so the policy must be pinned before 1.0. Explicit
`wrapping_*` / `saturating_*` ops live in the standard library for the rare
case that wants them.

TODO(open): **Bit-identical numerics across hosts.** The cross-runtime use case
(one script in JVM backend + JS browser + iOS app) demands that `1.1 + 2.2`
gives the same bytes on every host. IEEE 754 doubles agree on the basics but
diverge on transcendentals (`sin`, `log`, `pow`), on subnormals under some
JITs, and on the order of associative reductions. Decide which subset of
numeric operations Tel *promises* to make bit-identical, whether the
guarantee covers transcendentals or only the basic four ops, and whether
hosts may opt out for speed (and how visibly). Reinforces the
[determinism feature](../02-philosophy/03-features.md) and the maxim *one
script, many hosts — behaviour identical across them*.

## Decimals and currency

Money must not be a binary float (`Real32`/`Real64`). Binary floating point
cannot represent `0.10`
exactly, so sums drift. Tel provides **`Decimal`**: an exact base-10 number with
a large (conceptually unbounded) range, so a chain of additions and subtractions
is exact.

`Decimal` is the building block; a real currency amount is normally a
[refined / newtype](12-refined-types.md) wrapper over it, which adds a name, a
fixed scale, and a unit:

```tel
# A euro amount: an exact Decimal, tagged so it cannot be mixed with
# a bare number or with another currency.
type EuroAmt = newtype Decimal

let price: EuroAmt = EuroAmt(19.99)
let total = price + EuroAmt(0.01)   // exact: 20.00, not 20.00000001
```

TODO(open): the precise `Decimal` model — is the range truly unbounded, or
bounded with a defined precision? Is scale part of the type or the value? The
notes ask "unlimited range (domain?)" without settling it. A frozen language
needs this nailed down.

TODO(open): currency-specific behaviour — rounding modes, mixing currencies (a
hard error), display/formatting. Likely a standard-library concern built on
`Decimal` plus refined types and [physical units](13-units.md), not a language
primitive.

## The unit type

Tel has a **unit type**: a type with exactly one value, used where a function
"returns nothing" or a slot must be filled but carries no information. The name
and spelling are open; Rust's `()` is the leading candidate.

```tel
fn log_line(a_log: Log, msg: Text) -> Unit { a_log.info(msg) }
```

Why a real type and not a special "void": a function whose return type is the
unit type is still an ordinary function, so it composes with generics
(`Result[Unit, E]`, a `List[Unit]`) without a special case.

For *data-less* cases there is a subtlety: a unit-like type and
its single value can be made **interchangeable** — the type *is* the value. This
is what powers data-less union members and named-boolean enums (see
[`../10-data-modelling/02-union-types.md`](../10-data-modelling/02-union-types.md)),
where `:NotFound` names both a type and its sole inhabitant.

TODO(open): one unit type or many? A single shared `Unit` is simple, but has a
downside: a trait method declared to return `Unit` tells a
reader nothing, whereas distinct empty types (`Done`, `Ack`) carry intent. Lean:
one canonical `Unit`, plus user-defined data-less types when a name adds meaning.
Decide and document.

TODO(open): a floated idea was reusing `0` as the empty value and inferring
whether `0` means `Int`, `Real`, or "void". Rejected here as too clever and
inference-fragile — it conflicts with "no implicit conversions" and "surprise is
a cost". Recorded so the rejection is visible.

TODO(open): final name for the unit type. Candidates seen: `()` / `Unit`. Keep
a name that a Python/Java/Rust reader finds unsurprising — per the familiarity
priority. Settle alongside the never type's name (see
[The Never Type](14-never-type.md)).

## The never type

The bottom type — the type with *no* values, used as a diverging function's
return type — now has its own chapter: see [The Never Type](14-never-type.md).
It is documented separately from the value-carrying primitives because its rules
(uninhabitedness, and being a subtype of every type) are unlike theirs.

## See also

- [Strings and Text](03-strings-and-text.md)
- [The Never Type](14-never-type.md) — the bottom type, split out of this chapter.
- [Refined Types](12-refined-types.md) — the wrapper story for `EuroAmt` etc.
- [Physical Units](13-units.md) — units built on numeric primitives.
- [Conversions and Coercions](11-conversions-and-coercions.md) — why no number
  silently changes type.

TODO: review
