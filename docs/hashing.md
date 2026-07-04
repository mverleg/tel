# Hashing Strategy

## Summary

The codebase uses two non-cryptographic hash libraries for different purposes, and they are
not interchangeable:

| Library | Output | Use for | Never use for |
|---|---|---|---|
| `ahash` | 64-bit, unstable across runs/versions | In-memory `HashMap`/`HashSet` | Stored fingerprints, dedup, disk cache keys |
| `xxhash-rust` (xxh3) | 128-bit for content keys, 64-bit for result fingerprints; stable & specified | Content-addressed cache keys (128), answer-changed fingerprints (64) | (Adversarial inputs — see below) |

## The Two Kinds of Hash Use

### Hashmap bucketing (ahash)

In a hashmap, the hash only selects a bucket; the map then compares the actual key with `Eq`
before declaring a match. A collision is therefore a *performance* event (one extra key
comparison), never a *correctness* event. Hashmaps guarantee collisions by design anyway —
a 1024-bucket map uses only ~10 bits of the hash. So the priorities are raw speed and decent
bit dispersion, which is ahash's design target.

### Hash as identity (xxh3-128)

The query compiler (`qcompiler`) compares a step's new answer against its previous answer by
hash instead of full equality, so it can decide "output unchanged, dependents can reuse their
cache" without keeping or re-loading the full old value. Here the hash *replaces* the `Eq`
check: there is no second line of defense. A collision means silently concluding "same",
skipping a rebuild, and shipping stale output with no error anywhere.

The same applies to content-addressed cache keys (`telc-cache`, parse cache): the hash *is*
the identity of the content.

## Why 128 Bits

The failure modes are asymmetric:

* **False "changed"** (equal values, different hashes): cannot come from the hash itself —
  same bytes always hash the same. It *can* come from hashing a non-canonical representation
  (e.g. iterating a `HashMap` in nondeterministic order before hashing). Safe direction
  (redundant rebuild), but it silently destroys incrementality, so canonical serialization
  matters more than the hash choice — see [deterministic-hashing.md](deterministic-hashing.md)
  for how this is enforced.
* **False "same"** (collision): the dangerous direction — stale build, no error.

Collision math — note that 64 bits would suffice for the first case; the 128 bits are only
needed for the second:

* **Pairwise old-vs-new for the same query** (a pure "did the result change?" check): each
  comparison is an independent 2⁻⁶⁴ (or 2⁻¹²⁸) shot. 64 bits is fine here: a billion
  comparisons ≈ 10⁻¹¹ total risk.
* **Hash as lookup key in a hash or sorted structure** (dedup, content-addressed keys —
  any place where the fingerprint identifies a value among all values ever seen): birthday
  territory. With n distinct values, collision probability ≈ n²/2^(b+1) for b-bit hashes.
  At 64 bits: ~10⁻⁶ for 10⁷ values, ~10⁻⁴ for 10⁸ — not "cosmic ray" territory, and cache
  keys accumulate. At 128 bits it is negligible for any n we can store.

Concretely: **content keys are 128-bit** (they identify a value among all values ever seen),
while **result fingerprints are 64-bit** — their collision domain is only the historical
outputs of one logical query (bounded by edit count, Σ n²/2⁶⁵, negligible), because results
are stored *under* content keys and a fingerprint can only collide meaningfully within the
same dependency slot of the same dependent. This holds only while result fingerprints are
never themselves used as storage/lookup keys — see the invariants in
[keys-and-invalidation.md](keys-and-invalidation.md). rustc's incremental system uses 128-bit
fingerprints throughout; we split widths because the 64-bit domain argument is documented and
enforced.

## Why These Libraries

* **ahash** is among the fastest hashers for hash tables (AES-NI accelerated), but only
  produces 64-bit output (it implements `std::hash::Hasher`, whose interface is `u64`),
  seeds randomly per process by default, and explicitly does not guarantee stable output
  across its own versions. All fine for maps, disqualifying for stored fingerprints.
* **xxh3-128** is about as fast as ahash, has a native 128-bit variant, and its output is a
  stable, specified format across versions and platforms — safe to persist to disk.
* **Not a cryptographic hash** (e.g. blake3): none of the hashed inputs are adversarial.
  If a shared/remote cache is ever introduced where untrusted input could be crafted to
  collide (cache poisoning), switch fingerprinting to blake3 — it is cryptographic yet still
  runs at multiple GB/s.
