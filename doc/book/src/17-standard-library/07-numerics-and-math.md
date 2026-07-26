# Numerics and Math

<!-- TODO: review -->

## What

`std` provides the numeric types a script reasons about — fixed-width
integers, reals, an arbitrary-precision integer, and **decimals** — together
with the maths operations on them. Tel names every numeric type by its width
(`Int64`, `Real64`, `UInt8`, …); see
[primitive types](../05-types/02-primitive-types.md) for the full set and the
rationale.

## Widths are explicit; domains are newtypes

Consistent with *high abstraction over low-level control*
([`../02-philosophy/01-priorities.md`](../02-philosophy/01-priorities.md)), the
abstraction does **not** come from hiding the width. The widths are real, named
types (`Int64`, `Real64`, …). High-level reading comes instead from *naming the
domain*: a script wraps a width in a
[newtype](../05-types/12-refined-types.md) such as `type Age = newtype Int16`,
so call sites read in domain terms while the chosen width stays pinned and
visible in one place. For throwaway local arithmetic a bare literal defaults to
`Int64` / `Real64`.

There are **no implicit numeric conversions** and **no quiet integer
overflow**: widening, narrowing, and int ↔ real conversions are written
explicitly, and overflow is a defined error, not a wraparound. See
[`../02-philosophy/04-antifeatures.md`](../02-philosophy/04-antifeatures.md).

## Arbitrary-precision integers — `DynInt`

`std` provides **`DynInt`**, a dynamically-sized (arbitrary-precision) signed
integer. Because it has no fixed width it **never overflows** — it grows to fit
the value — which makes it the right choice for combinatorics, exact factorials,
cryptographic key material, or any count that can outrun 64 bits.

```tel
let f: DynInt = factorial(50)   # far beyond Int64, exact
```

`DynInt` is a standard-library value type, not a language primitive: the core
numeric types are all fixed-width
([primitive types](../05-types/02-primitive-types.md)), and `DynInt` is built on
top of them. That placement is deliberate — it keeps unbounded, heap-allocating,
non-constant-time arithmetic a *visible, named* choice rather than something a
script falls into by writing a bare literal (which defaults to `Int64`).

Conversions to and from the fixed-width types are explicit, like every other
numeric conversion: narrowing a `DynInt` into an `Int64` can fail (the value may
not fit) and so is a checked, fallible operation, never silent truncation.

`TODO(open): the `DynInt` surface — construction from literals and from
fixed-width ints, the fallible-narrowing API shape, and how it interacts with
the bitwise helpers below (which are defined on fixed-width types).`

## Decimals

`std` provides a **decimal** type for money and other exact base-10
quantities. It has **unlimited range** — it does not silently overflow — so a
valuation or accounting script does not accumulate binary-floating-point
error.

```tel
let price  = Decimal("19.99")
let qty    = 3
let total  = price * qty           # exact: 59.97, no rounding drift
```

`TODO(open): is the decimal *domain* (range/precision)
itself bounded or genuinely unlimited, and how is rounding behaviour selected?
Unlimited range is stated; precision and rounding policy are not. Decide.`

Refined numeric wrappers — `EuroAmt`, bounded ranges, `Id[Person]` — are
encouraged on top of the base numeric types; that machinery is the refined-
types story in [`../02-philosophy/03-features.md`](../02-philosophy/03-features.md),
not a separate numeric feature.

## Refinement-aware arithmetic (open)

A richer ambition: numeric *types that carry value
constraints*, so the type system can reason about division by zero and
sign-preservation.

```text
x: Real64 / y: Real64        -> (Real64 | inf)
x: Real64 / y: Real64 != 0   -> Real64
x: Real64 > 0 / y: Real64 > 0 -> Real64 > 0
```

The idea is that a constraint on the inputs specialises the result type — a
positive divided by a positive is positive; a non-zero divisor removes the
`inf` case. This overlaps with design-by-contract and refined types.

`TODO(open): refinement-aware arithmetic is unresolved and ambitious. It needs
a decision on how far the type system tracks value ranges, how it interacts
with the frozen-language goal, and whether it is worth the compile-time cost.
Treat as a direction, not a committed feature; coordinate with the
design-by-contract chapter.`

## Math utilities

`std` ships the small numeric helpers that scripts reach for repeatedly. They
are deliberately mundane; their value is that *every* Tel script has them by
the same name:

- **`gcd(a, b)` / `lcm(a, b)`** — greatest common divisor, least common
  multiple. Defined on integers (the refinement-aware story above could
  later carry through "result is positive" types).
- **`mod` and `rem`** — both modulo and remainder, kept distinct because
  they differ for negative operands (`-7 mod 3` is `2`; `-7 rem 3` is `-1`).
  Naming them the same is the bug; the library names them differently.
- **`clamp(x, lo, hi)`** — pin a value to a range. Also lives in the
  prelude.
- **`clip(x, max_abs)` / `pin_away_from_zero(x, min_abs)`** — recurring
  needs: cap an absolute value, and the reverse
  — push a value away from zero by at least `min_abs` so a subsequent
  division can't blow up.
- **`ssqrt(x)`** — *signed* square root, returning a negative root for a
  negative input (`ssqrt(-9) = -3`). Useful in stats and signal processing.
- **`smape(a, b)`** — symmetric mean absolute percentage error, with a
  defined answer for `0 / 0`. `TODO(open): pick the convention for `0/0` —
  this is the awkward case.`
- **Primes** — `is_prime(n)`, `next_prime(n)`, and a crypto-grade
  `random_prime(bits, rng)` that takes a `Random` capability. Crypto
  primes belong here so a script doesn't reinvent Miller–Rabin badly; see
  also [`15-randomness-hashing-and-crypto.md`](15-randomness-hashing-and-crypto.md).

## Bitwise operations

Bit manipulation — bitwise logic, shifts, single-bit access, and conversions
between integers, bytes, and hex/binary text — lives in `std` as named
functions, **not** language operators, and has its own topic:
[Bitwise and Binary Operations](21-bitwise-and-binary.md).

In brief: Tel reserves no `& | ^ ~ << >>` operators (they stay free for other
roles); bit work is done with `bin_and` / `bin_or` / `bin_xor` / `bin_not`,
`bin_left_shift` / `bin_right_shift`, and the conversion helpers. The idiomatic
way to model a set of flags is still a `Set[Flag]`, not OR-ed bits.

## Floating-point: fast by default, strict on request

Floating-point edge cases — `x + 0 ≠ x` for `x = -0`, `x + 1 - 1 ≠ x` for
tiny `x`, `NaN ≠ NaN` — are real but expensive to honour in every
expression. The lean is **fast-math semantics by default** for the
ordinary `Real64` type, with a separate **strict** float type (`StrictReal`
or similar) for code that needs full IEEE 754 fidelity.

- Default `Real64` may be reordered, folded across operations, and treated
  as if `NaN` never appears. The implementation is allowed to use
  `-ffast-math`-equivalent flags. In debug builds the runtime inserts
  checks for `NaN` / `Inf` to catch the case where these *do* appear.
- `StrictReal` opts back into bit-exact IEEE behaviour: deterministic
  rounding, no reassociation, defined `NaN` propagation. Slower; choose
  it when you need it.

`TODO(open): fast-math-by-default is a load-bearing call — it trades a
class of subtle bugs for performance. It must be reconciled with the
"reproducible runs" priority: two hosts may not agree bit-for-bit on a
fast-math result. Decide whether the *observable* contract is "an answer
within an error bound" or "the same answer everywhere," and document the
debug-mode checks.`

## Fixed-point and rationals

Two further numeric flavours:

- **Fixed-point** — base-2 or base-10 fractions with a *number of decimals
  in the type*, every step rounded by a stated mode. Useful where decimal
  is overkill (`Decimal` is unbounded; fixed-point fits in a machine
  word). `TODO(open): whether the number of decimals is a const generic, a
  refinement, or a per-type newtype.`
- **Rationals** — exact `p/q` numbers. They represent finite values in any
  base exactly, at the cost of larger storage and slower arithmetic. Worth
  including for the few scripts that need them (financial conversions,
  exact geometry) but not the default.

`TODO(open): whether to also build in symbolic constants (`pi`, `e`) so a
rational expression can keep them exact until the final conversion. The
input gestures at this without committing.`

## Monotonic counters

`std` offers **monotonically increasing** number types — a counter
whose type guarantees it never decreases. Common uses: epoch counters,
trace IDs, version numbers. The wrapper rejects assignment to a smaller
value at compile time where the value is constant, and at runtime
otherwise. `TODO(open): exact API; whether this overlaps with the refined
numerics in
[`../02-philosophy/03-features.md`](../02-philosophy/03-features.md).`

## Vectorized numerics

Bulk numeric work — matrices, FFTs, linear algebra — uses the
vectorized/transposed collections from
[`04-core-collections.md`](04-core-collections.md) rather than SIMD or GPU
intrinsics, which Tel deliberately does not expose. The library exposes
the operations one expects of a numerics stack — matrix multiply,
solve, decompositions, FFTs, basic statistics — implemented on the
transposed-collection substrate. A host with the relevant native
acceleration (BLAS/LAPACK) can wire that in behind the same surface;
without it the operations still work, just slower. See
[`../19-use-cases/05-matrix-and-fft.md`](../19-use-cases/05-matrix-and-fft.md).

`TODO(open): whether `std` ships BLAS/LAPACK *wrappers* at all, or only
the pure-Tel fallback. Bundled native code conflicts with the embedding
priority (the host owns the machine); a capability-based wrap is the
cleaner answer.`

## Matrices

A multi-dimensional matrix has user-visible shape choices, separate from
the SIMD/GPU representation question:

- **Storage layout**: row-major (outer index runs slowest) by default —
  the conventional choice, easier to read a 2-D matrix as a list of rows.
  column-major can be faster for some math and for
  column-typed data, but the design lands on row-major for familiarity.
- **Shape is fixed; there is no growable matrix.** A matrix's dimensions
  are set when it is created and do not change — there is no "grow a
  dimension" or capacity-reservation story. Data that genuinely grows uses a
  [`List`](04-core-collections.md), not a matrix.
- **Mutable vs immutable refers to *values*, not shape.** Both forms exist,
  and the distinction is whether the *elements* can change in place — never
  whether the dimensionality changes (it never does). The **mutable** form
  matters for peak performance: in-place updates avoid copying a large
  buffer on every operation.
- **Internal padding** — rows may be padded to a vector-alignment
  boundary for SIMD-friendly traversal; this is invisible to the user.

A *pandas-style* "named axes" frame — heterogeneous, per-column types, with
`filter`/`groupby`/`pivot` — is **not** a matrix: it has a different type per
column and wants relational, not linear-algebra, operations. So a matrix has **no
heterogeneous named-axis mode**; any axis labels it carries are labels for integer
indices only. That feature is the separate **dataframe** (`Table[R]`), documented
in [Dataframes](../10a-dataframes/01-overview.md).

## Optimisers and fitters

`TODO(open): **A small catalogue of optimisers and fitters.** The
scientific and financial use cases — see
[`../19-use-cases/04-spline-interpolation.md`](../19-use-cases/04-spline-interpolation.md)
and the showcase plans — repeatedly want a least-squares fit, a
non-linear solver, an unconstrained minimiser, and a couple of standard
fitters (polynomial, spline). Decide whether these live in
`numerics-and-math` alongside linear algebra, or in a separate
`optimisation` topic. Both are defensible; lean toward keeping them
*near* linear algebra because they share the matrix/vector types and
because users reaching for one almost always reach for the other. Keep
the catalogue short and curated — a handful of well-chosen methods, not
a SciPy-shaped surface.`

## See also

- [Core Collections](04-core-collections.md)
- [Strings and Text](06-strings-and-text.md)
- [Features Tel Embraces](../02-philosophy/03-features.md)
- [Internationalisation and Formatting](16-internationalisation.md) —
  locale-aware number rendering
- [Matrix Math and FFT](../19-use-cases/05-matrix-and-fft.md)
