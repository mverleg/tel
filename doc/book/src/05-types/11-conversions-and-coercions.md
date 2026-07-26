# Conversions and Coercions

A **conversion** in Tel is something the author writes. There are no implicit
coercions: a value of one type never silently becomes another. This is one of
Tel's loudest antifeatures (see [`../02-philosophy/04-antifeatures.md`](../02-philosophy/04-antifeatures.md))
and the reason the language can stay both small and safe.

## The rule

> If the types do not match, the script does not compile, until the author
> writes a conversion.

Concretely, Tel rejects:

- **Numeric widening** — `Int64` does not become `Real64` on its own. No `Int32 → Int64`
  silently.
- **Truthy / falsy.** `Bool` is the only type usable where a `Bool` is expected;
  `if 0 { ... }` or `if some_list { ... }` does not compile.
- **Text ↔ number.** `"3" + 4` is a type error; the script either parses or
  formats.
- **Boxing or wrapping a primitive into its newtype.** A bare `Decimal` is not
  a `EuroAmt`; `EuroAmt(d)` is.
- **Quiet integer overflow.** Arithmetic that would overflow is a loud error,
  never wrap-around (see [`02-primitive-types.md`](02-primitive-types.md)).
- **Quiet NaN propagation hiding a real bug** — a separate story, also covered
  in [`02-primitive-types.md`](02-primitive-types.md).

The point of these refusals is *not* purity. It is that, in a language that
runs unchanged across many host runtimes for decades, every silent coercion is
a place a script can change meaning without the author noticing.

## Three kinds of explicit conversion

Conversion splits effectively into three families:

### 1. Total conversions — always succeed

A conversion that cannot fail returns the converted value directly. This covers
**narrowing into a less constrained type** (a `Real64 > 0` flows into a `Real64`
freely; a `Cat` flows into `(Cat | Dog)`) and **unwrapping a newtype**:

```tel
let r: Ratio = ...
let plain: Real64 = r.value                # newtype unwrap, total

let cats: List[Cat] = ...
let a:    (Cat | Dog) = cats.first()     # narrow → wider union, total
```

These are conversions in the "I am writing it down" sense, not in the
"something can go wrong" sense. The compiler still asks for them in writing.

### 2. Checked conversions — may fail

A conversion that can fail returns a `Result` or an `Option`. The author
handles the failure where it happens, not where it bites later:

```tel
let parsed: Result[Int64, ParseError] = Int64.parse(text)
let euro:   Result[EuroAmt, ConstraintError] = EuroAmt.from(d)   # d: Decimal
```

A refined-type constructor is the canonical checked conversion: the constraint
runs, and the result is either the refined value or a typed error. To be
explicit — a refined type's constructor is the point where a value
*becomes* the refined type.

### 3. Aborting conversions — fail loudly

When the author knows a conversion cannot fail in practice and would rather
crash than thread an `Option`, Tel offers an aborting form — the equivalent of
Rust's `unwrap` or `expect`. It returns the converted value; on failure it
aborts the task with a clear message. Loud failure is in the
[maxims](../02-philosophy/02-maxims.md) — better than silent corruption.

```tel
let port = Int64.parse(env.PORT).unwrap_or_abort("PORT must be an integer")
```

TODO(open): exact spelling of the aborting form. Candidates: `.unwrap()`,
`.expect("msg")`, `!.value`. Prefer one canonical name; per
[antifeatures](../02-philosophy/04-antifeatures.md), do not surface a long
family of single-purpose variants.

## Where conversions live

There is a practical question: where do conversions *live*? Three
candidates:

- **Constructors on the target type** — `EuroAmt.from(d)`, `Int64.parse(s)`.
  Reads as the conversion's destination; works well for refined types and
  parses.
- **Methods on the source value** — `d.as_euro()`, `s.parse_int()`. Reads
  fluently; less discoverable.
- **A pair of traits** (Rust's `From` / `Into`). Universal but overloaded —
  `Into[X]` is everywhere.

Lean: **prefer constructors on the target type**. They read as "into a
`EuroAmt`", they cluster the validation alongside the type, and they avoid the
Rust ergonomics issue — `AsRef[str]` vs `Into[String]` vs
`Cow[str]` is *three* ways to write what should be one conversion, and the
author has to pick. Tel prefers a single canonical entry point per conversion.

TODO(open): commit to constructor-on-target as the canonical form, with a
narrow exception for traits where the conversion *is the trait* (e.g. an
`Encode[JsonValue]` whose only method serialises). Decide whether `From` /
`Into`-style cross-type conversion traits exist at all.

TODO(open): a broader pattern is an author wanting methods
that work uniformly with owned and referenced forms of a value. Without a
mutability/borrow model settled, this is on hold. See the open
mutability-model question in
[`../02-philosophy/04-antifeatures.md`](../02-philosophy/04-antifeatures.md).

## Conversion between layered shapes (containers of results, etc.)

A common transformation in real code, familiar from Java
experience, is shuffling `Result` and `Option` *through* a container:

```text
List[(Expiry, Result[T])]  <->  List[Result[(Expiry, T)]]  <->  List[(Expiry, T)]
```

Tel treats these as **library helpers**, not language features. The standard
library is expected to ship the recurring ones (a `traverse` / `sequence` for
`List[Result[T]]` ↔ `Result[List[T]]`, a `drop_errors` for the lossy case, a
zipping helper for the `Result`-inside-pair case). These are non-overlapping,
composable, and consistent with each other per
[the maxims](../02-philosophy/02-maxims.md).

What Tel does *not* do is silently flip layers in inference. If a function
returns `Result[List[T]]` and the caller wants `List[Result[T]]`, the
conversion is written.

TODO(open): final names and shape for the layer-shuffling helpers. Defer to
the standard-library design.

## Coercion-like conveniences that *are* in Tel

A few patterns look coercion-shaped but are not, and matter to spell out so the
reader does not feel the rule is being broken:

- **Union widening.** `A` is usable where `(A | B)` is expected — see
  [`09-subtyping-and-variance.md`](09-subtyping-and-variance.md). Same type
  identity, no conversion.
- **`Never` as any type.** A value of type `Never` (an aborting call, an
  infinite loop) is usable in any context — see
  [`02-primitive-types.md`](02-primitive-types.md).
- **Refined-type narrowing.** A more constrained refined type is usable where
  a less constrained one is expected — `Real64 > 0` flows into `Real64`. Same
  underlying value, the constraint is just dropped from the static view.
- **`Some[T]` as `Option[T]`.** A function known to always return present can
  declare `Option[T]` and a caller with the concrete `Some[T]` keeps that
  static knowledge. This is union widening again, not implicit coercion — see
  [`06-option-and-nullability.md`](06-option-and-nullability.md).

None of these *change* a value; they only let one statically-known type be
treated as a wider one. The author writes them implicitly by writing the
target type; nothing runs at the conversion point.

## Domain conversions are the host's job — sometimes

When a script crosses the host boundary (a Tel value goes out to the host or a
host value comes in), the conversion is *not* implicit. The host's binding
layer adopts the value as a Tel type, and any refined-type constraint runs
there — see [refined types and the outside world](12-refined-types.md). This
is the only "automatic" conversion in the system, and it is automatic only in
the sense that the host-binding layer applies the same constructor the
in-language code would.

TODO(open): exact mechanism by which host-side deserialisation calls into
refined-type constructors; tied to the schema-first serialisation story.

## Bugs the no-implicit-conversion rule prevents

A few concrete catalogue cases where
an implicit conversion was either the cause or would have masked the bug:

- **"Negative grid size from integer overflow."** A computation `(int)
  ((end - start) / 0.001) + 1` truncated a double to `int`; values beyond
  `INT_MAX` clamped to `INT_MAX`; the `+1` then overflowed in `Int64` space
  to `INT_MIN`. `Math.min` could not save it. Tel's *arithmetic does not
  silently overflow* rule (see
  [`02-primitive-types.md`](02-primitive-types.md)) and *no silent
  narrowing* rule (no implicit `double → Int64`) catch this at the
  conversion the author would have to write.
- **"Out-of-memory because we allocated a 2^31+ array."** A misdiagnosis
  triggered because the JVM throws `OutOfMemoryError` when an array's
  count exceeds `Integer.MAX_VALUE`, which looks like real OOM. Same
  root cause: silent integer arithmetic mis-sized the request. With
  explicit numeric types and explicit overflow handling, the failure
  surfaces at the size computation.
- **"`true.` instead of `true`."** A search-and-replace introduced a
  trailing dot in a string used as a boolean toggle.
  `Boolean.parseBoolean("true.")` silently returns `false`. Tel does
  not have truthy/falsy parsing of strings as booleans, and feature
  toggles are typed enums, not strings (see
  [antifeatures](../02-philosophy/04-antifeatures.md)).
- **"`=` instead of `==` disabled the algorithm."** A Java EL expression
  used `=` instead of `==`; the expression-language semantics made the
  assignment a truthy value that disabled the surrounding check. Tel
  has no expression language with implicit assignment-as-value
  semantics and no truthy coercion of arbitrary values to `Bool`.

## See also

- [Primitive Types](02-primitive-types.md) — numeric and overflow rules.
- [Refined Types](12-refined-types.md) — constructors as checked conversions.
- [Subtyping and Variance](09-subtyping-and-variance.md) — what widening is
  allowed without conversion.
- [Antifeatures — no implicit conversions](../02-philosophy/04-antifeatures.md).

TODO: review
