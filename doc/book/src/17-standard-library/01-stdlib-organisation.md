# Standard Library Organisation

<!-- TODO: review -->

## What

Tel ships a single, curated standard library — `std` — that covers the data
structures, iteration, text, numerics, and serialization a typical script
needs. Anything that touches the outside world (files, network, time,
randomness) is *also* described by `std`, but reached only through a
host-granted capability rather than an ambient global. See
[`08-io-and-filesystem.md`](08-io-and-filesystem.md) for the capability model.

The library is organised into the topic areas listed in this chapter:

- [Platform Layer](02-platform-layer.md) — what each host must implement
  for the rest of `std` to work.
- [Prelude](03-prelude.md) — names available without an import.
- [Core collections](04-core-collections.md) — lists, maps, sets, tables.
- [Iteration and streams](05-iteration-and-streams.md) — lazy pipelines.
- [Strings and text](06-strings-and-text.md) — text, formatting, interpolation.
- [Numerics and math](07-numerics-and-math.md) — integers, reals, decimals.
- [I/O and filesystem](08-io-and-filesystem.md) — capability-gated file access.
- [Time](09-time.md) — capability-gated clocks.
- [OS and process](10-os-and-process.md) — capability-gated environment access.
- [Networking](11-networking.md) — capability-gated sockets and HTTP.
- [Concurrency utilities](12-concurrency-utilities.md) — channels, tasks, platform-conditional shared-mutable primitives.
- [Data formats](13-data-formats.md) — JSON and friends.
- [Observability and logging](14-observability-and-logging.md) — logs, traces,
  metrics, benchmarks.
- [Randomness, hashing and crypto](15-randomness-hashing-and-crypto.md) —
  PRNGs, hashes, base64.
- [Internationalisation and formatting](16-internationalisation.md) — locale,
  currency, dates, translations.
- [Scheduling and timed operations](17-scheduling-and-timed-ops.md) — cron,
  retries, debounce, dependency graphs.
- [Tel-as-data](18-tel-as-data.md) — types representing Tel syntax, for code
  generation (not reflection).
- [Testing utilities](19-testing-utilities.md) — assertion helpers, property
  generators, deterministic capability stubs.
- [Data access and ORMs](20-data-access-and-orms.md) — why `std` ships no
  ORM, and the language pieces a third-party one can stand on.
- [Compression](22-compression.md) — zstd (preferred) and gzip (interop
  floor); streaming, one-shot, and dictionary modes.

## Why

### Batteries included, by design

A core priority is *one good way over many clever ones*
([`../02-philosophy/01-priorities.md`](../02-philosophy/01-priorities.md)), and
that extends to the library: a script should be able to do real work without
pulling in a constellation of third-party crates. The maxim is blunt — *the
standard library should be enough for small, complete programs*.

This matters more for Tel than for a standalone language. Tel scripts are
often shipped as **source** and compiled by the host at load time (Apivolve,
for instance, ships source, not class files). Every dependency a script pulls
in is a dependency every host must be able to resolve. A fat, dependable `std`
keeps the common case dependency-free.

### Composable, non-overlapping, consistent

The library follows the same maxims as the language:

- **Composable.** Pieces combine cleanly — an iterator feeds a collection
  builder feeds a serializer — rather than each module being an island.
- **Non-overlapping.** There is one list-type story, one map-type story, one
  way to format a string. Two modules do not solve the same problem two ways.
- **Consistent.** Naming and argument order are uniform across modules, so a
  reader who knows one module can guess the next.

### No ambient power, even from `std`

`std` does not break the capability rule. There is no `std.io.print`, no
`std.fs.open`, no `std.time.now` that works without a host grant. The I/O,
time, networking, and OS modules describe *types* (a `File`, a `Clock`, a
`Socket`) and the operations on them — but an instance only exists because the
host handed the script a capability that produced it. A browser host that
grants no filesystem capability simply makes that part of `std` unreachable,
and the script must cope. See
[`../02-philosophy/04-antifeatures.md`](../02-philosophy/04-antifeatures.md).

### Magic where it earns its place

Most of `std` is implementable in Tel itself, and the long-term plan is to do
exactly that — the standard library is written in Tel and embedded into the
compiler/runtime. This keeps it portable across every compile target. But it
is **not a hard requirement** that *all* of `std` be expressible in plain Tel:
a few primitives (low-level collection internals, numeric intrinsics) may have
direct compiler support where a pure-Tel implementation would be slow or
impossible.

### Two layers: platform-supplied vs Tel-written

`std` divides into two layers. Both ship as part of the language contract;
the split is about *who supplies the implementation*.

- **Platform layer** — features each host must implement, either directly
  or by satisfying an interface `std` defines. Roughly: everything I/O,
  everything that touches the machine, and the few features whose
  prior-art designs move too fast for the stability maxim (TLS, HTTP).
  Listed in full in [Platform Layer](02-platform-layer.md).
- **Tel layer** — everything else: regex, the wider collections, iteration
  adapters, date arithmetic, JSON, hashing, gzip. Written *in Tel*
  on top of the platform layer, so it does not have to be re-implemented
  per target. (zstd is the exception — it binds to the host's libzstd
  rather than being reimplemented; see [Compression](22-compression.md).)

Many topics span both halves: path *manipulation* is pure Tel; path
*access* is a platform capability. The IANA time-zone *data* ships as Tel;
"what zone am I in" is a host call. Each topic in this chapter documents
both halves where they apply; the [Platform Layer](02-platform-layer.md)
topic gives the consolidated host-facing list.

A consequence of the Tel layer being written in Tel: the *language* and
the *library* are more tightly coupled than usual. Some surface syntax may
compile down to library calls. This is an implementation detail to the
script author, but it is the reason `std` is treated as part of the
language contract, not as a separable add-on.

### Treating most of `std` as "in scope"

A small modding hook should not need a stack of imports for everyday names. A
generous subset of `std` is automatically in scope for scripts (see
[`03-prelude.md`](03-prelude.md)); larger or more specialised areas are
imported on demand. The library favours **batteries-included** over
**configurable** — easier to get started, more homogeneous, a coherent base to
build on. The trade is fewer plug-points; the library decides this is worth it
for an embedded scripting language where consistency across hosts is itself a
feature. Only the parts of `std` a script actually uses are compiled into the
output, so "wide" does not mean "fat at runtime".

### Deprecation, evolution and migration

Tel itself is effectively frozen ([`stability priority`](../02-philosophy/01-priorities.md)),
but the library is allowed to *grow* and, very rarely, **shed** a piece. The
expected pattern: a candidate is published as a separate crate, exercised in
the wild, then folded into `std`. A piece may also leave `std`: this is rare,
goes through a deprecation period, and ships with **automated migration**
tooling that rewrites old call sites to the new form. The mechanism doubles as
upgrade help for third-party crates — see
[`../18-tooling/07-linter.md`](../18-tooling/07-linter.md) and
[`../18-tooling/04-package-manager.md`](../18-tooling/04-package-manager.md).
`TODO(open): "shed from std" sits uneasily with the stability priority — confirm
that a long deprecation plus automated rewrite is considered enough.`

### Conservative defaults

`std` does not hand out a default `equals`, `hashCode`, or `toString` unless
the obvious implementation is correct ~99% of the time. Where the right
behaviour is genuinely ambiguous, the library asks the programmer to be
explicit rather than guessing — consistent with *no implicit DWIM*.

## Expanding the standard library

A recurring question: how does a script reach functionality `std` does not
have?

- **Importing from URLs / GitHub gists** was considered and is **rejected**.
  Code fetched from a URL rots when the URL dies, and running online code —
  even heavily sandboxed — sits badly with Tel's stability and supply-chain
  goals. `TODO(open): confirm rejection — the lean is against URL imports,
  but the door is not fully closed.`
- **The package manager** is the supported answer for reusable Tel code that
  does not belong in `std`. See
  [`../18-tooling/04-package-manager.md`](../18-tooling/04-package-manager.md).
  Crates declare the capabilities they need, so a dependency cannot quietly
  acquire network or filesystem access.
- **Host-provided functionality** is the answer for anything `std` genuinely
  should not do (native acceleration, app-specific resources): the host
  exposes it as a capability.

`TODO(open): the boundary between "belongs in std", "belongs in a crate",
and "belongs to the host" needs a crisp rule, not just three examples.`

## See also

- [Platform Layer](02-platform-layer.md)
- [Prelude](03-prelude.md)
- [Priorities and Trade-offs](../02-philosophy/01-priorities.md)
- [Package Manager](../18-tooling/04-package-manager.md)
