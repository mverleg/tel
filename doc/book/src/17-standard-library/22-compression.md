# Compression

<!-- TODO: review -->

## What

`std` provides byte-stream compression with two codecs in distinct roles:

- **zstd** (RFC 8478) — the **preferred** codec. Best ratio and
  decompression speed of the mainstream formats, and the one most likely
  to still be current a decade or two out. A single codec covers every
  mode Tel needs: **one-shot**, **streaming**, and **dictionary**
  (including trained dictionaries) — see "Dictionaries" below.
- **gzip** / DEFLATE (RFC 1951 / 1952) — the **interop floor**. Not the
  default, but the format everything else speaks: HTTP `Content-Encoding`,
  zip, PNG, and countless file formats. Always present (see
  "Implementation reality").

Both expose one-shot and streaming APIs, and both compose with the
iteration / stream surface in
[`05-iteration-and-streams.md`](05-iteration-and-streams.md) — a
compressor is just a stream transform from bytes to bytes.

**gzip needs no capability** — it is pure Tel, a plain data transform you
can always call. **zstd is reached through a host-granted capability**,
because its implementation is host-supplied (see "Implementation
reality"). This is deliberate: capabilities are *already* how Tel models
platform-dependence, so an absent zstd is simply a capability the host did
not grant — the same situation as any other capability — rather than a
special "feature missing" error mode.

## Why

**Why two and not one.** The split is not "dictionary vs. no dictionary"
— zstd does both. It is **best codec vs. universal interop**. zstd wins on
ratio and speed but you cannot assume a peer decodes it: as of 2026 zstd
`Content-Encoding` is in Chrome and Firefox but not Safari, while gzip is
spoken by *everything*. So Tel keeps zstd as the codec you reach for and
gzip as the one you fall back to when talking to the outside world.

**Why not Brotli.** Brotli is attractive for static web *text* (it has a
built-in web dictionary and broad `Content-Encoding: br` support), but it
is a third native dependency that earns its place only for that one niche.
`TODO(open): expose Brotli as a host-supplied capability if/when web-text
interop demands it; do not bundle it in core.`

**Why not archive formats.** zip / tar are deliberately *out* of core —
see the exclusion note in
[`13-data-formats.md`](13-data-formats.md). Compression here is about byte
streams, not file-tree archives.

## Dictionaries

A dictionary primes the compressor with sample data so that **many small,
similar payloads** — RPC frames, package manifests, log lines, rows —
compress well despite individually being too short to build up context.
This is a zstd feature, not a separate algorithm:

- pass a fixed dictionary buffer to the one-shot or streaming API, or
- **train** one from a corpus of representative samples and reuse it on
  both ends.

gzip has no real dictionary story; if you need dictionaries, you are on
zstd by definition.

## Implementation reality

The two codecs are very different to implement, and that drives where they
live:

- **gzip is pure Tel and guaranteed.** Inflate is a few hundred lines
  (bit reader, canonical Huffman, a 32 KB LZ77 window, CRC32); a decent
  deflate is a couple of weeks more. It needs nothing beyond the bitwise
  ops in [`21-bitwise-and-binary.md`](21-bitwise-and-binary.md), so `std`
  simply **carries** it. On any host, with or without native libraries,
  gzip is available — it is the portability floor.
- **zstd is host-provided and optional.** Its entropy stage (FSE / tANS)
  plus the Huffman, sequence, frame, and dictionary machinery make a
  correct decoder a serious project and a *competitive* compressor a
  multi-month one that would likely be slow in pure Tel. So `std` ships
  the **interface** and binds to the host's libzstd (or a host-granted
  capability) for the implementation, rather than reimplementing it.
  Platforms are **expected but not required** to supply zstd: most should,
  but a minimal or unusual host may legitimately omit it.

The consequence to keep in mind: zstd is not guaranteed. Because it
arrives as a capability, "is zstd available here?" is answered the same
way as for any other platform-dependent capability — the host either hands
you the zstd capability or it does not. A program that must run everywhere
takes the capability as *optional*, uses it when present, and falls back
to gzip (or uncompressed) when it is absent. That is why gzip, not zstd,
is the always-present baseline even though zstd is the preferred format.

`TODO(open): pin the exact API surface — e.g. `compress.gzip` as a plain
module and zstd reached through a host-granted capability value. Settle
how an optional capability is taken and tested for presence (the general
mechanism is in [`10-os-and-process.md`](10-os-and-process.md)); the zstd
case should fall straight out of that, not invent its own availability
API.`
