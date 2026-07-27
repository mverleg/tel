# Hashing

<!-- TODO: review -->

The cache is built out of hashes, and two different jobs are being done with
them. Using one hash for both is the easiest way to get a silently wrong
compiler.

| Job | Property needed | Reference implementation | Never use for |
|---|---|---|---|
| Hashmap bucketing | speed, bit dispersion | `ahash` (64-bit, unstable across runs and versions) | stored fingerprints, dedup, disk cache keys |
| Identity | stability, collision resistance | xxh3 (128-bit content keys, 64-bit result fingerprints) | adversarial input — see below |

## Two kinds of hash use

### Bucketing

In a hashmap the hash only selects a bucket; the map then compares the actual
key with equality before declaring a match. A collision is therefore a
*performance* event — one extra key comparison — never a *correctness* event.
Hashmaps guarantee collisions by design anyway: a 1024-bucket map uses about
ten bits of the hash. The priorities are raw speed and decent dispersion.

### Identity

The query engine decides "this answer is unchanged, dependents can reuse their
cache" by comparing hashes, without keeping or reloading the full previous
value. Here the hash **replaces** the equality check; there is no second line
of defence. A collision means silently concluding "same", skipping a rebuild,
and shipping stale output with no error anywhere.

The same applies to content-addressed cache keys: the hash *is* the identity of
the content.

> Rejected alternative: keep the full previous answer and compare real equality,
> using hashes only for map lookup (Salsa's approach). Zero false positives, at
> the cost of holding every answer in memory. For a cache whose answers can be
> large and whose store is shared on disk, fingerprints are the better trade.

## Why the widths differ

The two failure modes are asymmetric:

- **False "changed"** — equal values, different hashes. This cannot come from
  the hash itself; identical bytes always hash identically. It comes from
  hashing a non-canonical representation, e.g. iterating a hash map in
  nondeterministic order. The direction is safe (a redundant rebuild) but it
  silently destroys incrementality, so canonical encoding matters more than the
  hash choice — see [Deterministic hashing](06-deterministic-hashing.md).
- **False "same"** — a genuine collision. This is the dangerous direction: a
  stale build with no error.

Sixty-four bits would suffice for the first case. The extra bits are only for
the second, and only where the collision domain is global:

- **Pairwise old-vs-new for one query** ("did this result change?") — each
  comparison is an independent 2⁻⁶⁴ shot. A billion comparisons is about 10⁻¹¹
  total risk. 64 bits is plenty.
- **Hash as a lookup key** — any place a hash identifies a value among all
  values ever seen. This is birthday territory: with *n* distinct values,
  collision probability is roughly `n² / 2^(b+1)`. At 64 bits that is ~10⁻⁶ for
  10⁷ values and ~10⁻⁴ for 10⁸ — not cosmic-ray territory, and cache keys
  accumulate. At 128 bits it is negligible for any *n* that can be stored.

Concretely:

- **Content keys are 128-bit** — they identify a value among all values ever
  seen.
- **A source file's content digest is 128-bit** for the same reason: it is the
  entire variable part of a parse key's preimage, so it is compared in that same
  global keyspace of all distinct file contents. A 64-bit digest would cap the
  parse key's entropy no matter how wide the key itself is.
- **Result fingerprints are 64-bit** — their collision domain is only the
  historical outputs of one logical query, because results are stored *under*
  content keys and a fingerprint can only collide meaningfully within the same
  dependency slot of the same dependent.

That last argument holds **only** while result fingerprints are never used as
storage or lookup keys ([invariant 2](09-invariants.md)). rustc uses 128-bit
fingerprints throughout; splitting the widths is only safe because the 64-bit
domain argument is written down and enforced.

## Why these hashes

- **A fast table hasher** (`ahash` in the reference implementation) is among the
  fastest available, but produces only 64-bit output, seeds randomly per process
  by default, and explicitly does not promise stable output across its own
  versions. All fine for maps, disqualifying for anything stored.
- **xxh3-128** is about as fast, has a native 128-bit variant, and its output is
  a stable, specified format across versions and platforms — safe to persist.
- **Not a cryptographic hash.** None of the hashed inputs are adversarial. If a
  remote or multi-tenant cache is ever introduced, where untrusted input could
  be crafted to collide (cache poisoning), switch fingerprinting to a
  cryptographic hash such as blake3 — still multiple GB/s.

## The digest algorithm is part of the schema version

Swapping the hash is not a drop-in change: it changes every key in every
existing cache. So **the algorithm is an ingredient of the persisted schema
version**, alongside the encoding rules — the schema version is folded into
every content key, and the persisted cache carries it as a stamp.

That makes an algorithm swap a *cold cache* rather than a corruption. Old
entries live under old keys and are simply never looked up again; a store
stamped with a different schema version is rejected wholesale on open, rather
than being read with the wrong interpretation. They are garbage to be collected,
not hazards — the same property that lets the store be append-only in the first
place.

The rule follows: **any change to what goes into a hash, or to how it is
hashed, requires a schema version bump.** That covers the algorithm, the
encoding rules of [Deterministic hashing](06-deterministic-hashing.md), and the
shape of any answer type that gets fingerprinted.
