# Bitwise and Binary Operations

Bit manipulation lives in `std`, **not in the language**. Tel deliberately has
no bitwise *operators* — the `& | ^ ~ << >>` symbols stay free for other roles
(see [Comparison and Logical Operators](../07-expressions/03-comparison-and-logical.md)
and [Arithmetic and Numeric](../07-expressions/02-arithmetic-and-numeric.md#bit-shifts-are-not-operators)).
Everything here is a named library function on integer and byte values. Naming
them as functions sidesteps the spelling debate that dogged them as operators:
there is no precedence to define and no symbol to reserve.

## Why functions, and why a small set

Tel sits closer to Python than to C ([priorities](../02-philosophy/01-priorities.md)):
ordinary code should not reason about endianness, bit widths, or machine
integers, and a flag set is more honestly a `Set[Flag]` than an `Int64` of
OR-ed bits. But code at the **boundary with external binary data** — parsing a
binary format, unpacking a packed config, decoding a host's protocol, computing
a hash — has nowhere else to go. For that boundary `std` commits to the
inventory below; it ships in 1.0.

Everything in this topic obeys the standard Tel numeric discipline:

- Operands share the **same** fixed-width integer type — no implicit widening,
  no signed/unsigned mixing.
- The result has the operand type.
- A shift or bit index is restricted to `0 <= n < width`; out of range is a
  **defined error**, not undefined behaviour (see
  [antifeatures](../02-philosophy/04-antifeatures.md)).
- As ordinary calls they need no place in the
  [no-precedence rule](../04-syntax/04-precedence-and-associativity.md):
  mixing them with arithmetic reads as nested calls, not operator soup.

## Bit logic and shifts

| Function | Meaning |
|----------|---------|
| `bin_and(a, b)`         | bitwise AND |
| `bin_or(a, b)`          | bitwise OR |
| `bin_xor(a, b)`         | bitwise XOR |
| `bin_not(a)`            | bitwise NOT (ones' complement) |
| `bin_left_shift(x, n)`  | shift bits left by `n`, zero-filling the low bits |
| `bin_right_shift(x, n)` | shift bits right by `n`; **sign-extends** for signed types, **zero-fills** for unsigned types |

The right shift needs no separate "arithmetic vs logical" pair. Because Tel's
signed and unsigned integers are **distinct types**, `bin_right_shift` does the
right thing for each: arithmetic (sign-preserving) on `Int*`, logical
(zero-fill) on `Uint*`. There is no machine-level reinterpretation to get wrong.

```tel
let mask  = bin_or(0x0F, 0xF0)          # 0xFF
let high  = bin_right_shift(0xAB, 4)    # 0x0A  (on a Uint8)
let packed = bin_or(bin_left_shift(r, 16), bin_or(bin_left_shift(g, 8), b))
```

## Boolean exclusive-or

`and` and `or` are short-circuit keywords
([logical operators](../07-expressions/03-comparison-and-logical.md)), but there
is no boolean-xor keyword — so the one boolean bit-op that needs a name is:

| Function | Meaning |
|----------|---------|
| `logical_xor(a, b)` | `Bool` exclusive-or — `true` iff exactly one of `a`, `b` is `true` |

`logical_xor` is distinct from `bin_xor`: it takes and returns `Bool`, whereas
`bin_xor` operates on the bits of an integer. Keeping them separate avoids the C
trap where the same `^` quietly means both.

## Inspecting and poking individual bits

For the "poke individual bits" cases, `std` also ships:

| Function | Meaning |
|----------|---------|
| `count_ones(x)`      | number of set bits (population count) |
| `leading_zeros(x)`   | count of zero bits above the highest set bit |
| `trailing_zeros(x)`  | count of zero bits below the lowest set bit |
| `bit_width(x)`       | position of the highest set bit + 1 (`0` for `0`) |
| `rotate_left(x, n)`  | rotate bits left by `n` (bits wrap around, none lost) |
| `rotate_right(x, n)` | rotate bits right by `n` |
| `test_bit(x, i)`     | `Bool` — is bit `i` set? |
| `set_bit(x, i)`      | `x` with bit `i` set |
| `clear_bit(x, i)`    | `x` with bit `i` cleared |

`set_bit` / `clear_bit` return a **new** value — integers are immutable, there
is no in-place bit mutation.

## Converting between integers, bytes, and text

The boundary with external binary data needs explicit, endianness-aware
conversions — never an implicit reinterpretation of memory. Byte order is an
explicit `Endianness` (`Endianness.Big` / `Endianness.Little`); there is **no
"native" option**, so the same code decodes the same bytes the same way on every
host — which matters for an embedded guest that cannot assume the host's
byte order.

**Integers ⇄ bytes** (one constructor per fixed width — `Int8/16/32/64`,
`Uint8/16/32/64`):

| Function | Meaning |
|----------|---------|
| `to_bytes(x, endian)`          | the fixed-width integer `x` as `Bytes`, in the given order |
| `Int32.from_bytes(b, endian)`  | read a sized integer back from exactly-width `Bytes` |

**Integers ⇄ text** (radix forms; the `from_*` parsers return a
[`Result`](../13-error-handling/02-result-types.md) because text may be
malformed, while the `to_*` directions are total):

| Function | Meaning |
|----------|---------|
| `to_hex_string(x)`             | integer → base-16 text, e.g. `0xFF` → `"ff"` |
| `Int32.from_hex_string(t)`     | base-16 text → sized integer (`Result`; one per width, like `from_bytes`) |
| `to_binary_string(x)`          | integer → base-2 text, e.g. `10` → `"1010"` |
| `Int32.from_binary_string(t)`  | base-2 text → sized integer (`Result`) |

**Bytes ⇄ text**:

| Function | Meaning |
|----------|---------|
| `to_hex_string(b)`         | raw `Bytes` → hex text, two chars per byte |
| `Bytes.from_hex_string(t)` | hex text → `Bytes` (`Result`) |

```tel
# decode a big-endian u32 length prefix from a host protocol
let len = Uint32.from_bytes(header[0..4], Endianness.Big)

# round-trip an integer through hex text
let h = to_hex_string(0xCAFE)            # "cafe"
let n = Uint16.from_hex_string("cafe")?  # 0xCAFE
```

## Prefer sets to bit-OR

Even with these functions available, the *idiomatic* way to express a set of
flags is a `Set[Flag]` over a named enum — reach for the bitwise functions only
at the boundary with external binary data:

```tel
# preferred
let perms: Set[Permission] = { .Read, .Write }
if perms.contains(.Write) { ... }

# discouraged — only when interoperating with a packed external format
let perms = bin_or(Read, Write)
if bin_and(perms, Write) != 0 { ... }
```

The set form is type-safe, exhaustively checkable, and reads as the *intent*
rather than the encoding.

## See also

- [Numerics and Math](07-numerics-and-math.md) — integer and float arithmetic,
  overflow discipline, the numeric types these functions operate on.
- [Strings and Text](06-strings-and-text.md) — general text handling that the
  hex/binary string converters bridge into.
- [Arithmetic and Numeric § Bit-shifts are not operators](../07-expressions/02-arithmetic-and-numeric.md#bit-shifts-are-not-operators)
  — why the shift symbols are absent from the grammar.
