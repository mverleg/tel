# Tuples and Arrays

Tuples and arrays are Tel's two **anonymous composite types**: they group values
without forcing the author to declare a [record](../10-data-modelling/01-records.md)
first. The two are complementary: a *tuple* is a fixed-length, mixed-type group;
an *array* is a fixed-length, single-type group. Variable-length collections
live in [`../10-data-modelling/09-collection-types.md`](../10-data-modelling/09-collection-types.md).

## Tuples — anonymous, fixed-shape, mixed-type

A **tuple** packs a fixed, heterogeneous list of values into one value:

```tel
let pair: (Int64, Text) = (3, "apples")
let triple = (now(), Severity.Warn, "disk almost full")

# Returning multiple values without naming a record:
fn divmod(a: Int64, b: Int64) : (Int64, Int64) { (a / b, a % b) }
```

The shape — length and the types in each position — is part of the type.
`(Int64, Text)` and `(Text, Int64)` are different types; `(Int64, Int64, Int64)` and
`(Int64, Int64)` are different types.

A tuple **type** is written exactly like a tuple **value**, with each literal
replaced by its type — the general
[type-as-value notation](01-type-system-overview.md#how-types-are-written). Named
fields carry their name across into the type:

```tel
(3, "apples")          # value   →   (Int64, Text)                    # type
(1, 2, a = 3, b = 4)   # value   →   (Int64, Int64, a = Int64, b = Int64)   # type
```

### Literal forms: grouping vs tuple

Parentheses do double duty — grouping an expression *and* writing a tuple — the
same way Python's do. Tel has **no separate grouping bracket**: `[ ]` is reserved
for [arrays](#arrays--fixed-length-single-type) and collection indexing, `{ }`
for [blocks](../04-syntax/03-blocks.md). The two readings never collide because
**a tuple is made by the comma (or a named field), not by the parentheses**:

| Spelling                 | Meaning                                                            |
|--------------------------|-------------------------------------------------------------------|
| `(x)`                    | grouping — just the value `x`                                      |
| `(x + y)`                | grouping                                                           |
| `(f(a, b))`              | grouping — the comma is *inside* the call, not at the paren level  |
| `()`                     | the empty tuple — i.e. the [unit value](02-primitive-types.md)     |
| `(x,)`                   | one-element tuple (trailing comma required)                        |
| `(x, y)`                 | two-element tuple                                                  |
| `(a = 3)`                | one-element tuple with a *named* field                             |
| `(1, 2, a = 3, b = 4)`   | positional prefix, then named tail                                 |

The shape is decided on a **top-level discriminator**, so the parser commits on
the opening `(` and resolves the rest with bounded lookahead — it never scans
ahead to a far-off comma. Either of:

- a **top-level comma**, or
- a **top-level named field** (`name =`)

…means tuple; otherwise a single parenthesised expression is just grouping.
"Top-level" is load-bearing: commas nested inside a call, an array, or an inner
tuple belong to *that* construct and never promote the outer parens. Concretely
the parser reads a comma-separated list of elements at the paren's own level,
then:

- zero elements → the unit / empty tuple;
- one element, no trailing comma, no name → unwrap to grouping;
- anything else → tuple.

The single positional tuple still needs its trailing comma (`(x,)`), because
`(x)` is already spoken for as grouping. A single **named** field needs no comma —
`(a = 3)` can only be a tuple, since a bare `a = 3` is not an expression and so
has no grouping reading to clash with. The named label must be a bare identifier;
`(a.b = 3)` is not a named field. Distinguishing a label from an expression that
merely starts with an identifier costs **one token of lookahead** (peek for `=`,
which is a distinct token from `==`), keeping the grammar LL(2).

### Arguments are tuple-shaped, kept distinct by the `fn` marker

A call's argument list and a tuple have the **same shape** — an ordered,
partly-named, heterogeneous group — and Tel deliberately leans into that, so a
tuple *behaves like* an argument list (see
[Tuples as argument bundles](#tuples-as-argument-bundles)). What keeps this from
becoming Swift's deepest tuple regret is the **`fn` marker on function types**:
a function type is always written `fn(...) : T`, never a bare parenthesised
group, so a tuple type and a function's parameter list never collide at the
surface.

Swift modelled argument lists *as* tuples in the type system with no such
marker, so `(Int64, Text) -> R` (two parameters) and `((Int64, Text)) -> R` (one
tuple parameter) were a constant source of compiler pain (Jordan Rose,
["Tuples and Argument Lists"](https://belkadan.com/blog/2021/08/Swift-Regret-Tuples-and-Argument-Lists/),
["Labeled Tuple Elements"](https://belkadan.com/blog/2021/08/Swift-Regret-Labeled-Tuple-Elements/)).
Tel avoids it because the leading `fn` opens a *parameter-list* context that is
never mistaken for a tuple:

- `fn(Int64, Text) : R` — **two** parameters.
- `fn((Int64, Text)) : R` — **one** parameter, a 2-tuple (the inner parens make
  the tuple; see [grouping vs tuple](#literal-forms-grouping-vs-tuple)).

So the two grammars *rhyme by design* (one row machinery, one set of named-field
rules) without being **confusable** — the `fn` keyword is the disambiguator, and
the [function-type spelling rationale](05-function-types.md) records the same
point from the function side. The two grammars share their row rules:

- named tuple fields and named arguments **share resolution rules** — they are
  the same row;
- reordering / casting between differently-labelled tuples is governed by
  [Identity, equality, and narrowing](#identity-equality-and-narrowing):
  explicit narrowing only, never an implicit cast.


### Why tuples exist at all

Real Java experience makes the case: data-transformation code
constantly produces intermediate pairings like `List[(Expiry, Result[T])]` or
`Map[Key, (Count, Sum)]`. Forcing a `struct` declaration for every such pairing
is heavy. Tuples make the everyday "two things together" case free at the call
site, without giving up static typing — the shape is still checked.

A tuple is the *anonymous* form. The moment the pairing has meaning the script
will reuse, prefer a named [record](../10-data-modelling/01-records.md): the
name carries the intent, and `point.x` reads better than `point.0`.

### Destructuring and accessing

A tuple's positions are addressable by index and by destructuring:

```tel
let (q, r) = divmod(17, 5)             # q = 3, r = 2
let line   = (now(), "boot")
let when   = line.0
let what   = line.1
```

Destructuring is the encouraged form — naming the parts at the binding site is
clearer than reaching for `.0`, `.1`. Tuples should be small (two or three
positions); a long tuple is a sign a record would read better.

TODO(open): final spelling for tuple position access — `t.0` (Rust),
`t[0]` (Python-ish), or destructure-only. Undecided; lean
`t.0` for familiarity with Rust/Scala and to avoid clashing with `[ ]` indexing
into arrays and collections.

### Operating over a tuple generically

A tuple is **heterogeneous**: each position can hold a different type. Tel offers
**no generic operation over an arbitrary tuple** — there is no single `impl` that
prints *any* tuple whose positions are each `ToString`, regardless of arity:

```tel
(3, "apples", Severity.Warn).to_string()   # not provided generically
```

Work with a tuple at its concrete shape — destructure it and act on the parts,
or reach for a [record](../10-data-modelling/01-records.md) when the shape
recurs. Tel has no HList-style type-level recursion over tuple structure, which
keeps the language's complexity floor low.

### Tuples and unions in the same expression

Untagged unions inside tuples behave the obvious way:
`(Int64, Text | None)` is a tuple whose second slot may be absent. Combined with
union flattening this gives short signatures for "value with metadata where the
metadata is optional" without ad-hoc records. They sidestep the Java pain of
shuffling between `List[(Expiry, Result[T])]` and `List[Result[(Expiry, T)]]`;
Tel makes both spellings type-check directly, and the user picks the one that
reads best for the transform.

TODO(open): convenience conversions between `(A, Result[B])` and
`Result[(A, B)]` etc. Likely standard-library helpers (`zip_result`, `traverse`),
not language sugar — see [`11-conversions-and-coercions.md`](11-conversions-and-coercions.md).

### Tuples as argument bundles

A call's argument list and a tuple have the same shape: an ordered, partly-named,
heterogeneous group. `min(1, -2, -3, by = abs)` and a literal
`(1, -2, -3, by = abs)` would be the *same* object — a tuple with positional keys
`0, 1, 2` and a named key `by`. This is the same unification the rest of the
language already leans on: "a struct initialiser *is* a function call"
(see [parameters](../09-functions/02-parameters-and-arguments.md)) and the
function-type spelling `fn(Int64, Int64) : Int64`, whose parameter list is itself a
tuple-shaped, partly-named group (see [function types](05-function-types.md)). If
adopted, calls, construction, and function types become three views of one
labelled tuple.

The structure falls straight out of the existing **binary parameter sections**
(positional prefix, then named tail): a labelled tuple is a dense positional
prefix (keys `0..n`, never named, always present) followed by a sparse named tail
(string keys, optional). So **a named member can never precede a positional one** —
the same ordering rule, re-read as a tuple-literal rule.

What this buys:

- tuple positions can carry **meaningful names** (`line.when`, not `line.0`);
- a call's arguments can be **captured as a value** (a typed args bundle);
- a bundle of **statically-known shape** can be **splatted** into another call,
  written `f(...b)` — checked member-by-member and desugared to an ordinary call
  (see [splatting a bundle into a call](../07-expressions/06-function-application.md#splatting-a-bundle-into-a-call));
- a bundle can be **forwarded** through a generic wrapper when its row matches the
  target *exactly* — a function exposes its argument row as `f::Args` (and result
  as `f::Return`), and `f(...args)` type-checks when `args: f::Args`. Under
  monomorphisation this is just the splat above.

This is all **exact-shape**: a tuple with named fields *is* an anonymous
structural record, and the rules compare such records by exact shape. Tel does
**not** add row *subtyping* (width, permutation, or optional-field subtyping
between differently-shaped rows). Where rows differ, narrow explicitly or write a
lambda. See [TIP-0006](../tips/0006-tuples-as-argument-bundles.md) for the full
resolution.

**Named tuple fields are adopted** — a tuple may carry named members
(`(1, 2, a = 3, b = 4)`), so the "keep tuples strictly unnamed and positional"
option is off the table. The argument-bundle story (splat, exact-match
forwarding, calls-as-tuples) works on exact shapes; the broader row subtyping it
could have pulled in is not adopted.

### Identity, equality, and narrowing

A labelled tuple has **two kinds of key**, and they obey **two different
identity rules**:

- **Positional keys** (`0..n`) are **ordinal** — their order *is* meaning.
  `(1, 2)` and `(2, 1)` are different values.
- **Named keys** are **nominal** — an unordered *set* of `name → value`
  bindings. Write-order carries no meaning: `(a = 1, b = 2)` and
  `(b = 2, a = 1)` are the **same type and the same value**.

Everything below falls out of that one distinction. A tuple's shape is
therefore *(ordered position list) + (unordered name set)*.

**Type identity.** Two tuple types are the same iff they have the same
positional arity (with matching position types) *and* the same set of named
keys (with matching field types). Reordering names does not change the type;
reordering positions does.

**Equality.** Differently-shaped tuples are different *types*, and Tel has no
cross-type equality (see
[equality](../10-data-modelling/07-equality-and-hashing.md)). So `(a = 1) == (1,)`
does not return `false` — it **does not type-check**, the same way `Cat == Dog`
doesn't; the question "are they equal?" never arises. Within one shape, equality
is structural: equal at every position and at every named field, regardless of
the order the names were written.

**Hashing and ordering** need a deterministic traversal of the unordered named
tail. The canonical order is **by name**, so `(a = 1, b = 2)` and `(b = 2, a = 1)`
hash and compare identically — exactly what the set-based equality above demands
of the Eq–Hash contract, and what keeps a named-field tuple usable as a `Map`
key or `Set` member (see
[hashing](../10-data-modelling/07-equality-and-hashing.md#hashing) and
[ordering](../10-data-modelling/08-ordering.md)).

**No implicit conversion.** There is **no coercion between differently-shaped
tuples**, and in particular none between a positional slot and a named one. A
name has no canonical position — the named tail is unordered, so nothing fixes
which index `a` would become — and a position has no canonical name, so neither
direction is well-defined. `(a = 1)` and `(1,)` are simply unrelated types.

**Narrowing is explicit, never implicit.** Dropping named fields to fit a
narrower shape — using `(x = 1, y = 2)` where `(x = 1)` is wanted — *is* sometimes
useful, but it is **opt-in**, written at the use site, not an implicit
width-subtyping rule:

```tel
let full   = (x = 1, y = 2)
let just_x = full.narrow      # -> (x = 1); the extra named field is dropped
```

Making it explicit is what keeps Swift's "casting matrix" from forming: by
default a shape fits only itself, and the narrowing call is the single, visible
escape.

**No runtime reflection over labels.** Field names are **static type information
only**. Nothing can enumerate a tuple's labels at run time, so whether a host
erases them or keeps them in its representation is an implementation choice with
**no user-visible effect** — consistent with Tel's no-reflection stance (see
[antifeatures](../02-philosophy/04-antifeatures.md)). This disposes of the
"runtime label questions" Swift faced: there is no runtime label surface at all.

The rules above are the row story in full — named fields exist and are compared
by exact shape, with explicit narrowing the only width operation. Tel does **not**
layer *implicit* row polymorphism (width / permutation / optional-field subtyping)
on top: splat and exact-match forwarding work on shapes that already match, and
any other reshaping is the explicit `.narrow` step or a lambda.

There is **one** blessed, closed exception that *does* transform record shapes at
the type level — adding, dropping, merging, and retyping fields — the
[Record-Shape Calculus](15-record-shape-calculus.md). It is a fixed set of
compiler-recognised primitives (not user row polymorphism), used monomorphically;
it is what the [dataframe](../10a-dataframes/01-overview.md) is built on.

TODO(open): final spelling of the narrowing operator — `.narrow`, `.squeeze`,
or `.ignore_unknown`. The point is that it is *written*, not inferred.

**Defaults live on the signature, not the tuple.** A free-standing
`(1, -2, -3, by = abs)` is the *pre-match* call-site shape, carrying only what was
written; defaults are filled when the bundle is matched against a callee. A
captured bundle therefore preserves "what the caller said," and the inner function
applies its own defaults. Positional slots (no default, always present) and named
slots (may default, optional) map cleanly onto dense-prefix / optional-tail.

**A bundle is always the pre-match *spread* shape.** The call-site view is the
spread form (`(1, -2, -3, by = abs)`, keys `0, 1, 2`); the collected form
(`([1, -2, -3], by = abs)`, key `0` a list) is what the *parameter* becomes after
a [vararg](../09-functions/05-variadic-functions.md) collects. Collection is part
of signature-matching, not part of the tuple — so splatting a bundle into a vararg
spreads its trailing positionals and the vararg collects them at the call.

## Arrays — fixed-length, single-type

An **array** is a fixed-length sequence of values of a *single* element type.
The length is part of the type, and an array may have more than one dimension:

```tel
let rgb:  Array[Int64, 3]     = [255, 128, 0]    # 1-D, length 3
# A 2-D array is rectangular and its shape is part of the type.
# (Exact multi-dimensional spelling is open — shown loosely here.)
let grid: Array[Real64, 3, 4] = ...              # rank 2, a 3 x 4 grid
```

Use an array when the length is known and fixed by the problem (a 3-vector, a
4×4 transform matrix, a fixed-size buffer). Use a [list](../10-data-modelling/09-collection-types.md)
when the length grows and shrinks.

### Rank — N-dimensional, rectangular, not generic over dimension count

An array has a **rank** N ≥ 1 — its number of dimensions. Multi-dimensional
arrays are **rectangular**: within a dimension every row has the same length, so
there are no jagged shapes. The rank is **fixed in the type and never generic**:
a function may be generic over the element type and over the *lengths*, but not
over *how many dimensions* an array has. A native multi-dimensional array is
therefore a distinct thing from a nested `Array[Array[T, m], n]` (an array of
arrays); the rectangular form exists so grid-shaped data needs no nesting.

TODO(open): the multi-dimensional type spelling — `Array[T, 3, 4]`,
`Array[T, [3, 4]]`, or another form — and whether nested arrays remain a
separate allowed construct alongside the native rectangular one.

Indexing is **0-based** (the first element is `arr[0]`, valid indices `0..n-1`) —
settled; see
[Field and Index Access](../07-expressions/07-field-and-index-access.md#indexing-is-0-based).
Indexing is bounds-checked at the boundary; out-of-bounds is a loud error, never
silent corruption. Where the compiler can prove an index in range — typically
through a [branded index or refined int](12-refined-types.md) — the check is
elided.

TODO(open): final element-access spelling — `arr[i]` is the obvious choice and
matches every familiar language; confirm. Also: whether `arr[i]` returns `T`
(bounds-check, abort on miss) or `Option[T]` (caller handles absence) — the
notes prefer a *both* story: a default abort-on-miss accessor *and* a
total-but-`Option`-returning one (`.get(i)`), letting the author pick.

### Arrays as a const generic

Array length is the canonical case for [const generics](07-generics.md): the
length is a *value* that parameterises a *type*. This makes the array type
the simplest demo of const generics; advanced array layouts and length-relating
guarantees (e.g. matrix multiplication preserving compatible dimensions) are an
open question tied to how far const generics go.

TODO(open): commit to or reject value-level length tracking for array-returning
operations — `add(a: Array[T, n], b: Array[T, n]) -> Array[T, n]` is desirable
but bumps into the same dependent-ish concerns as
[refined-type constraint propagation](12-refined-types.md).

### Length and size

Every array exposes its shape through accessors:

- **`.len`** — the length of a **1-D** array.
- **`.len1`, `.len2`, …** — the length of each dimension of a higher-rank array
  (`.len1` is the first dimension, `.len2` the second, and so on).
- **`.size`** — the **total number of elements**, for an array of any rank
  (`grid.len1 * grid.len2` for a 2-D array, and equal to `.len` for a 1-D one).

TODO(open): whether a 1-D array *also* answers `.len1` (an alias of `.len`) for
uniformity, or `.len` is 1-D-only. Lean: `.len` for the common 1-D case,
`.lenK` for multi-dimensional, `.size` always.

### Storage and slices

An array's elements **may or may not be stored contiguously** — the layout is
the runtime's choice, and Tel makes **no guarantee about fast slicing**. A slice
is not promised to be a zero-copy view onto contiguous memory; treat slicing as
a possibly-copying operation unless a specific host documents otherwise.

Slices are still wanted as a general construct — a window into an array or list.
Their ownership story is open: a slice could *always* borrow (never own its
backing storage), or borrowing could be restricted to the immutable case, where
aliasing a shared read-only view is provably safe.

TODO(open): slice ownership — always-borrow, or borrow-only-when-immutable? A
mutable slice that aliases its parent is the dangerous case; an immutable one
never is. Tied to the reference/borrowing model in
[TIP-0001](../tips/0001-mutability-and-borrowing.md).

### Fixed vs runtime-chosen length

Two notions of "fixed length" need separating:

- **Compile-time fixed** — the length(s) are constants in the type
  (`Array[Int64, 3]`). The size is known at compile time, so the array can live
  inline / on the stack.
- **Runtime-chosen but then fixed** — the length is decided **once at
  construction** and never changes after, but is not known until run time. Such
  an array must live **on the heap** (or behind a slice-like handle, as in
  Rust), because its size is not a compile-time constant.

The runtime-chosen-size array is a **generalization** of the compile-time-fixed
one: a compile-time length is just the special case where the chosen size
happens to be a constant. Both are *fixed after construction* — neither grows;
growth is a [list](../10-data-modelling/09-collection-types.md).

TODO(open): the surface for a runtime-chosen-but-fixed array — a distinct type,
a slice handle, or an erased-length `Array[T, ?]` — and how the length appears
in the type. Interacts with const generics and the slice model above.

## Tuples and arrays are values

Like every Tel composite, tuples and arrays are **values**: immutable by
default, compared by content, hashable when their parts are hashable. "Changing"
an element produces a new value (copy-update; see
[records](../10-data-modelling/01-records.md)). A mutable, growable counterpart
lives as a separate type — a list — see
[`../10-data-modelling/09-collection-types.md`](../10-data-modelling/09-collection-types.md).

TODO(open): the mutable/immutable story for arrays specifically — whether an
element can be updated in place through a mutable binding, or whether "changing"
always means copy-update. Best settled *after* the
[record](../10-data-modelling/01-records.md) mutability model, since arrays and
records should treat in-place update the same way. Tracked with the broader
mutability model in [TIP-0001](../tips/0001-mutability-and-borrowing.md).

TODO(open): transposed / struct-of-arrays storage as a possible
*type-level* feature — an `Array[(Real64, Real64)]` that the runtime stores as
`(Array[Real64], Array[Real64])` for SIMD/locality. This is squarely an
implementation-detail concern (see
[`03-strings-and-text.md`](03-strings-and-text.md) on hidden representation) and
should not leak into the type-system surface; record in `impl-notes/` if at all.

## See also

- [Records](../10-data-modelling/01-records.md) — when the grouping has a name.
- [Collection Types](../10-data-modelling/09-collection-types.md) — variable
  length and other shapes.
- [Generics](07-generics.md) — including const generics.
- [Refined Types](12-refined-types.md) — branded indices for bounds-free access.

TODO: review
