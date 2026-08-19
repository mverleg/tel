# Field and Index Access

<!-- TODO: review -->

Reading a field of a record, or an element of a collection, are ordinary
expressions in Tel.

## What

Field access uses a dot:

```tel
an_order.id
point.x
config.retry.limit      # chained
```

Index access uses brackets:

```tel
scores[0]
table[row]
```

Both are *read* expressions. There is **no implicit setter**: `point.x = 3` is
not a way to mutate a field of an arbitrary value. Mutation happens only
through a `uniq` binding and is always explicit (see
[Mutability](../06-bindings-and-scope/02-mutability.md)); the immutable way to
"change" a field is a [`with`-copy](../06-bindings-and-scope/06-destructuring.md).

## Field access versus function call

Tel lets a zero-argument function be called without `()`
([Function Application](06-function-application.md)). That makes `point.x`
ambiguous between *reading field `x`* and *calling zero-arg function `x`*. The
deliberate consequence is that a stored field can be replaced by a computed
function of the same name without breaking callers.

TODO(open): the field-vs-getter resolution is the same open question raised in
[Function Application](06-function-application.md#the-field-vs-getter-ambiguity)
— resolve in one place. Until then, treat `value.name` as "access member
`name`", whether that member is stored or computed.

## Indexing is 0-based

Indexing is **0-based**: the first element of an array, list, or any indexable
collection is at index `0`, and an `n`-element collection has valid indices
`0..n-1`. This is settled, not open.

```tel
let xs = [10, 20, 30]
xs[0]            # 10 — the first element
xs[2]            # 30 — the last element of a 3-element list
xs[xs.len - 1]   # the idiomatic "last element"
```

Why 0-based:

- **Familiarity** — C, Rust, Python, Java, JavaScript, Go, and effectively every
  language Tel's audience reaches for index from `0`. Tel's
  [familiarity priority](../02-philosophy/01-priorities.md) makes matching them
  the default unless there is a strong reason not to, and there is not.
- **Half-open ranges fall out cleanly** — a range `0..len` (start inclusive, end
  exclusive) covers exactly the valid indices, slices compose without `±1`
  fudging, and an empty range is simply `i..i`.
- **Index is an offset** — `xs[i]` is "skip `i` elements," so the first element
  is offset `0`. The same mental model gives pointer-free bounds arithmetic.
- **Consistency with tuple positions** — tuple positions are numbered from `0`
  (`t.0` is the first; see
  [Tuples and Arrays](../05-types/04-tuples-and-arrays.md)), so collection
  indices and tuple positions agree.

There is no per-collection or host-configurable index base; a Tel program reads
the same on every host (see [reproducibility](../02-philosophy/02-maxims.md)).

## Indexing and bounds

Index access can fail when the index is out of range. Tel's safety priority
([priorities](../02-philosophy/01-priorities.md)) says such a failure must not
be silent.

TODO(open): whether `coll[i]` returns an `Option`, aborts on an out-of-range
index, or is restricted by a refined index type is not settled
for this chapter. There *is* a sketch of *branded* / generative index types
(an index that statically belongs to one specific collection, so bounds need
no runtime check) — promising but speculative; defer to
[`05-types`](../05-types/) and [collections](../10-data-modelling/09-collection-types.md).

## Multi-dimensional indexing

`TODO(open): `arr[x, y]` vs `arr[x][y]`.` How do these relate? Two
readings:

- `arr[x, y]` is the *primitive* form, used by multi-dimensional collections
  (matrices, tensors), and `arr[x][y]` is the *iterated* form, used by
  nested collections of collections (a `List[List[T]]`). They are not
  interchangeable in general — a `Matrix[Real64]` accepts `[i, j]` but does not
  accept `[i][j]`.
- Or: `arr[x][y]` is the only form, and a 2D collection's `[i, j]` syntax is
  syntactic sugar for it.

Lean: the first reading. A 2D collection is a distinct type from a list of
lists, and indexing should reflect that. Defer to
[Collection Types](../10-data-modelling/09-collection-types.md).

## See also

- [Function Application](06-function-application.md) — the zero-arg call form.
- [Mutability](../06-bindings-and-scope/02-mutability.md) — no setters.
- [Destructuring and Object Construction](../06-bindings-and-scope/06-destructuring.md) —
  `with`-copies as the immutable field update.
