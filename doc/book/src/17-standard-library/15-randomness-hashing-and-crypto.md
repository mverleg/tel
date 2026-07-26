# Randomness, Hashing and Crypto

<!-- TODO: review -->

## What

`std` bundles the everyday machinery for **random number generation**,
**hashing** (both crypto and non-crypto), small-scale **cryptography**
(hashes, key stretching, primes), and the encoding helpers that
typically travel with them — **base64**, **hex**, and friends. Every
operation that reaches outside the
program (a hardware RNG, a system entropy pool) goes through a host-
granted capability; the pure-data parts (hashing a buffer, base64-
encoding bytes) are ordinary functions.

## Randomness

### A capability, never ambient

The rule from [`10-os-and-process.md`](10-os-and-process.md): there is
no global RNG. A script that needs randomness receives a `Random`
capability and uses it.

```tel
fn pick_winner(entries: List[Entry], rng: Random) -> Entry {
    entries.at(rng.below(entries.size()))
}
```

A test or replay run hands the script a *seeded* `Random` and gets
deterministic output every time. There is no separate "test-only RNG"
type — the capability is the same; only its provenance differs.

### The `Random` surface

A `Random` exposes the small set of operations a script actually wants,
not a wrapper over a single underlying generator API:

- **`Bool()`** — uniform 0/1.
- **`below(n: Int64)`** — uniform `0..n`, free of modulo bias.
- **`range(lo, hi)`** — uniform `lo..hi` for integers, floats, or
  `Duration`s.
- **`pick(xs: List[T])`** — uniform element of a non-empty list.
- **`shuffle(xs: List[T])`** — returns a shuffled copy.
- **`weighted([{value, weight}, ...])`** — weighted pick.
- **`chance(p: Real64) -> Bool`** — `true` with probability `p`. Mirrored
  in the prelude (see [`03-prelude.md`](03-prelude.md)) for the common
  case where the RNG is on the context.

The capability also exposes the underlying byte stream
(`rng.bytes(n)`) for callers that need raw entropy.

### Crypto-grade vs ordinary RNGs

A host can grant either a *fast* (xoshiro / SFC-style) RNG or a
*crypto* one (system entropy). The split is visible in the *type*:

- **`Random`** — any random source; ordinary scripts depend on this.
- **`CryptoRandom`** — guaranteed to be cryptographically suitable.

A `CryptoRandom` *is* a `Random`, so code that wants either can ask
for the broader one; security-sensitive code that requires
unpredictability asks for `CryptoRandom`. `TODO(open): name —
`SecureRandom`? `RandomSecret`? The Java precedent is `SecureRandom`,
which reads well.`

## Hashing

### Crypto vs non-crypto, named separately

The library exposes two clearly-named families. It
should be **impossible to confuse them**:

- **`crypto.hash.sha256(bytes)`**, `sha512`, `blake3`, … — slow,
  collision-resistant against adversaries, suited for content-
  addressing and integrity. Salts and key-stretching helpers
  (`crypto.kdf.argon2(...)`, `pbkdf2(...)`) ship alongside.
- **`fast_hash.xxh3(bytes)`**, `fnv1a`, … — fast, collision-resistant
  under non-adversarial input. Suited for in-memory hash tables and
  cache keys. The name carries the warning.

`TODO(open): exact set of crypto primitives. The library should provide
the *current* recommended ones, not the historical alphabet soup
(MD5, SHA-1). Decide whether deprecated primitives ship at all — the
stability priority says yes (old data still has to be read); the
safety priority says hide them behind an explicit `crypto.legacy`
namespace.`

### Default hash for keyed collections

The `Map` / `Set` types use a randomised non-crypto hash with a
per-table salt by default — the safeguard against the *iteration-order
leaks salt* failure mode described in
[`04-core-collections.md`](04-core-collections.md). A caller that
needs cross-run determinism opts into a fixed-salt hash via the key
wrapper mechanism.

## UUID

`std` provides UUID generation and parsing. Generating a UUID is pure
*given* a randomness capability; the type and string representations are
ordinary values.

```tel
let id = Uuid.v7(rng)                    # time-ordered UUIDv7
let s  = id.to_string()                  # "018f2b…"
let p  = Uuid.parse("018f2b...")?        # back to a Uuid
```

The library exposes UUIDv4 (random) and UUIDv7 (time-ordered, finalised in
RFC 9562 / 2024) as the two recommended forms; older versions (v1, v3, v5)
are available for parsing legacy data but not surfaced as defaults.
Prior-art design point: RFC 9562 + UUIDv7.

`TODO(open): exact module name (`std.uuid` vs in `std.id`); whether parsing
accepts the "with braces" Microsoft form by default.`

## Cryptography

`std` deliberately ships a *small* cryptographic surface and stops
there:

- **Hashes** — see above.
- **Symmetric encryption** — AEAD (e.g. ChaCha20-Poly1305, AES-GCM),
  with the key as a refined type (`Key[Aead]`).
- **Asymmetric** — signing and verification (Ed25519), agreement
  (X25519). Heavy machinery (TLS, PKI, X.509) lives in the crate
  ecosystem.
- **Prime generation** — `crypto.prime(bits, rng)` returning a
  cryptographically suitable prime, taking a `CryptoRandom`. This
  belongs in `std` so a script does not roll its own Miller-Rabin.

The rule: the things every embedded script may legitimately want are
in `std`; protocol-shaped surfaces (TLS, JWT, OAuth) are not. A host
that wants a TLS-protected socket is expected to expose it as a
capability — the TLS stack is the *host's*, not Tel's.

`TODO(open): pre-pivot — re-justify against embedding. A non-trivial
cryptographic surface in a stable library is a maintenance commitment
("decade-old script still runs unchanged" applies to the crypto too).
Decide how much to ship versus delegate to the host.`

## Encoding helpers

The encoding family lives here because it travels with the crypto and
network workflows, not because it is conceptually related:

- **`base64.encode` / `base64.decode`** — with explicit URL-safe vs
  standard alphabets, no silent padding heuristics.
- **`base32` / `base16` (hex)** — same shape.
- **`hex.bytes(s)`** — byte arrays from hex literals at compile time.
- **`percent_encode` / `percent_decode`** — URL component escaping;
  also exposed as `escape_url` in
  [`06-strings-and-text.md`](06-strings-and-text.md).

The encoding functions are pure data transforms; no capability needed.

Compression now lives in its own topic,
[`22-compression.md`](22-compression.md).

## Storage rules

A few cross-cutting rules:

- **Secrets are typed.** Keys, passwords, and tokens are refined types
  with no `Display` / `toString` implementation — printing one is a
  compile error, so they cannot leak into a log line. To intentionally
  serialise one (e.g. to encrypted storage) the caller calls
  `.expose()` explicitly. `TODO(open): exact name and how the type
  interacts with the structured-logging serialiser — coordinate with
  [`14-observability-and-logging.md`](14-observability-and-logging.md).`
- **Constant-time comparison.** `crypto.constant_time_eq(a, b)` is a
  named function; ordinary `==` on a key or hash is *not* constant-
  time. The linter warns when `==` is used on a key-typed value.

## See also

- [Prelude](03-prelude.md) — `chance()` shortcut
- [Core Collections](04-core-collections.md) — hashing of keys
- [OS and Process](10-os-and-process.md) — capabilities including
  randomness
- [Observability and Logging](14-observability-and-logging.md) — why
  secrets do not render
- [Antifeatures](../02-philosophy/04-antifeatures.md) — no ambient RNG
