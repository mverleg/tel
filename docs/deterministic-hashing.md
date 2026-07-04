# Deterministic Hashing Guidelines

## Summary

Fingerprints (see [hashing.md](hashing.md)) only work if the same semantic value produces the
same bytes-to-be-hashed on every run, every machine, every compile. A spurious difference is a
silent incrementality killer: everything still *works*, but downstream queries recompute for no
reason. A spurious equality is worse (stale results).

The guardrail is a dedicated trait — call it `StableHash` — that is *deliberately not
implemented* for nondeterministic types. Determinism is then enforced by the compiler: if a
type reaches a fingerprint without a deterministic encoding, it fails to build instead of
silently hashing garbage like an interner index or map iteration order.

## Why Not `std::hash::Hash`

* No encoding stability guarantee: how `str`, integers, enums feed the `Hasher` is
  unspecified and may change between std versions or platforms (`usize` width!).
* Implemented for things we must reject: raw pointers hash their *address*; `Sym` derives
  `Hash` on its process-local index (correct for in-memory maps, wrong for fingerprints).
* No context parameter: resolving `Sym` back to its string requires the `Interner`, and
  `Hash::hash` has nowhere to pass it.
* No guardrail: `#[derive(Hash)]` happily includes any field; nothing flags a
  nondeterministic one.

Keep `std::hash::Hash` for in-memory maps (with ahash); it is good at that. `StableHash` is a
separate trait for a separate job, the same way `CtxDisplay` exists next to `fmt::Display`.

## Trait Shape

Mirror the existing `CtxDisplay` pattern: the context carries the interner (and later anything
else needed to map process-local ids to stable forms).

```rust
pub trait StableHash {
    fn stable_hash(&self, ctx: &StableCtx<'_>, out: &mut Xxh3_128);
}

pub struct StableCtx<'a> {
    pub interner: &'a Interner,
}
```

A `#[derive(StableHash)]` macro makes structs/enums cheap to support and is where the
enforcement lives: a field whose type lacks `StableHash` is a compile error — that error is
the feature. Support `#[stable_hash(skip)]` for fields that must not affect the fingerprint
(e.g. source spans in IDE mode, cached/derived fields).

## Encoding Rules

* **Integers:** fixed-width little-endian bytes. No `usize`/`isize` impl — convert explicitly
  to `u64` at the call site so 32-bit platforms agree.
* **Strings and byte slices:** length prefix (as `u64`), then bytes. Length prefixes prevent
  concatenation ambiguity: `("ab", "c")` must not hash like `("a", "bc")`.
* **Sequences** (`Vec`, slices, arrays): length prefix, then elements in order.
* **Structs and tuples:** fields in declaration order (derive handles this). Note that
  reordering/adding fields changes fingerprints — that is a cold cache, not a bug.
* **Enums:** variant tag (index as `u32`), then payload. Reordering variants likewise
  invalidates caches; acceptable, but know it.
* **`Option`/`Result`:** as enums (tag + payload).
* **Ordered maps/sets** (`BTreeMap`, `BTreeSet`): fine — iterate in key order, length prefix
  first. Only sound because iteration order equals `Ord` order.
* **`HashMap`/`HashSet`: no impl, ever.** Iteration order is nondeterministic; this is the
  single most likely determinism bug in practice. Convert to a `BTreeMap` or collect-and-sort
  at the fingerprint boundary. The missing impl is what makes the mistake impossible.
* **Floats: no direct impl.** `NaN != NaN`, `-0.0 == 0.0`, and bit patterns vary by operation
  order. If a float must be fingerprinted, wrap it in a newtype that canonicalizes (single NaN
  bit pattern, `-0.0` → `0.0`) and hashes the bits — the wrapper documents that someone thought
  about it.
* **References, `Box`, `Rc`, `Arc`:** hash the pointee's content. No impl for raw pointers.

## Interned Ids: the Repo-Specific Trap

`Sym` (and `Name`, `Path`, `FQ` wrapping it) is a `u32` index assigned by interning order,
which differs across runs and even across concurrent schedules within a run. Its derived
`Hash`/`Eq` are correct for in-memory use and must stay cheap — but a fingerprint containing a
raw `Sym` index is meaningless outside the process that made it.

So `StableHash for Sym` resolves through the context and hashes the *string*:

```rust
impl StableHash for Sym {
    fn stable_hash(&self, ctx: &StableCtx<'_>, out: &mut Xxh3_128) {
        ctx.interner.resolve(*self).stable_hash(ctx, out)
    }
}
```

This is the reason the trait takes a context at all. The same pattern applies to any future
process-local id (query ids, `StepId`, db indices): either resolve it to its stable meaning,
or don't implement `StableHash` for it.

## What Must Never Enter a Fingerprint

* Interner indices, arena/db indices, or anything assigned in discovery order (resolve first).
* `HashMap`/`HashSet` iteration order.
* Addresses/pointers, `usize` as-is.
* Timestamps, absolute paths, hostnames, environment values — unless they are *deliberately*
  part of the input state (a file mtime in `input_state` is a versioning signal, fine; an
  mtime accidentally embedded in an AST node is a determinism bug).
* Source spans in outputs that should be span-free (fast mode) — spans shift on every edit
  and destroy early cutoff. Use `#[stable_hash(skip)]` or keep spans in a side table.

## Testing Determinism

* **Order-shuffle test:** build the same value twice with different interning order (intern a
  bunch of dummy strings first in one of the runs) and different `HashMap` insertion order;
  fingerprints must match. This catches both `Sym` leaks and map-order leaks.
* **Cross-process test:** compute fingerprints of fixture values in a subprocess and compare —
  catches per-process seeds and address leaks that in-process tests can miss.
* **Golden fingerprints:** a few checked-in expected hashes for fixture values, so an
  accidental encoding change (which would cold-start every user's cache) shows up as a test
  diff instead of a mystery slowdown.
