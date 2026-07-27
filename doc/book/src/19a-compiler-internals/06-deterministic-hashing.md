# Deterministic Hashing

<!-- TODO: review -->

## Why

Keys and fingerprints only work if the same semantic value produces the same
bytes-to-be-hashed on every run, on every machine, in every compile.

- A spurious **difference** is a silent incrementality killer: everything still
  works, but downstream queries recompute for no reason and nobody notices
  except as a mystery slowdown.
- A spurious **equality** is worse: stale results.

The most likely source of both is not the hash function but *what gets fed to
it*: an interner index, a map's iteration order, a pointer, a timestamp.

## The guardrail

Determinism is enforced by the type system rather than by review. A dedicated
trait — call it `StableHash` — is **deliberately not implemented** for
nondeterministic types. A type that reaches a fingerprint without a
deterministic encoding then fails to compile, instead of silently hashing
garbage.

```rust
pub trait StableHash {
    fn stable_hash(&self, ctx: &StableCtx<'_>, out: &mut Xxh3_128);
}

pub struct StableCtx<'a> {
    pub interner: &'a Interner,
}
```

A `#[derive(StableHash)]` macro makes structs and enums cheap to support, and is
where the enforcement lives: a field whose type lacks `StableHash` is a compile
error, and **that error is the feature**. A `#[stable_hash(skip)]` escape hatch
covers fields that must not affect the fingerprint — source spans in fast mode,
cached or derived fields.

### Why not the language's ordinary hash trait

Rust's `std::hash::Hash` (and its equivalents elsewhere) is the wrong tool:

- **No encoding stability.** How strings, integers, and enums feed the hasher is
  unspecified and may change between standard-library versions or platforms —
  pointer-width integers are the obvious trap.
- **Implemented for things that must be rejected.** Raw pointers hash their
  *address*. An interned symbol derives its hash from a process-local index —
  correct for in-memory maps, meaningless in a fingerprint.
- **No context parameter.** Resolving an interned symbol back to its string
  needs the interner, and the standard trait has nowhere to pass it.
- **No guardrail.** A derive happily includes any field; nothing flags a
  nondeterministic one.

Keep the ordinary hash trait for in-memory maps — it is good at that.
`StableHash` is a separate trait for a separate job.

## Encoding rules

- **Integers** — fixed-width little-endian bytes. No implementation for
  pointer-width integers; convert explicitly at the call site so 32-bit and
  64-bit platforms agree.
- **Strings and byte slices** — length prefix, then bytes. The prefix prevents
  concatenation ambiguity: `("ab", "c")` must not hash like `("a", "bc")`.
- **Sequences** — length prefix, then elements in order.
- **Structs and tuples** — fields in declaration order. Reordering or adding a
  field changes fingerprints; that is a cold cache, not a bug.
- **Enums** — variant tag, then payload. Reordering variants likewise
  invalidates caches. Acceptable, but know it.
- **Optionals and results** — as enums: tag plus payload.
- **Ordered maps and sets** — fine. Iterate in key order with a length prefix.
  Sound only because iteration order equals the ordering relation.
- **Hash maps and hash sets — no implementation, ever.** Iteration order is
  nondeterministic; this is the single most likely determinism bug in practice.
  Convert to an ordered map, or collect and sort, at the fingerprint boundary.
  The *missing* implementation is what makes the mistake impossible.
- **Floats — no direct implementation.** `NaN != NaN`, `-0.0 == 0.0`, and bit
  patterns vary with operation order. If a float must be fingerprinted, wrap it
  in a newtype that canonicalises (a single NaN bit pattern, `-0.0` → `0.0`) and
  hashes the bits. The wrapper documents that somebody thought about it.
- **References and smart pointers** — hash the pointee's content. No
  implementation for raw pointers.

## Interned ids: the trap worth naming

An interned symbol is an integer index assigned in interning order, which
differs across runs and even across concurrent schedules within one run. Its
derived hash and equality are correct for in-memory use and must stay cheap —
but a fingerprint containing a raw index is meaningless outside the process that
made it.

So the stable implementation resolves through the context and hashes the
*string*:

```rust
impl StableHash for Sym {
    fn stable_hash(&self, ctx: &StableCtx<'_>, out: &mut Xxh3_128) {
        ctx.interner.resolve(*self).stable_hash(ctx, out)
    }
}
```

This is the entire reason the trait takes a context. The same pattern applies to
any other process-local id — query ids, step ids, arena indices: either resolve
it to its stable meaning, or do not implement `StableHash` for it.

## What must never enter a fingerprint

- Interner, arena, or database indices — anything assigned in discovery order.
- Hash map or hash set iteration order.
- Addresses, pointers, pointer-width integers taken as-is.
- Timestamps, absolute paths, hostnames, environment values — *unless* they are
  deliberately part of the captured input state. A file mtime used as a
  change-detection signal is fine; an mtime accidentally embedded in an AST node
  is a determinism bug.
- Source spans, in outputs that are supposed to be span-free. Spans shift on
  every edit and destroy early cutoff. Skip them, or keep them in a side table.

## Testing determinism

Three tests catch nearly everything:

- **Order shuffle.** Build the same value twice with different interning order
  (intern a pile of dummy strings first in one run) and different map insertion
  order. The fingerprints must match. This catches both interned-id leaks and
  map-order leaks.
- **Cross-process.** Compute fingerprints of fixture values in a subprocess and
  compare. This catches per-process seeds and address leaks that in-process
  tests cannot see.
- **Golden fingerprints.** A few checked-in expected hashes for fixture values,
  so an accidental encoding change — which would cold-start every user's cache —
  shows up as a test diff instead of a mystery slowdown.
