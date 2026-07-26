# Platform Layer

<!-- TODO: review — new topic separating platform-supplied capabilities
     from the user-facing stdlib. -->

## What

`std` ships as two layers. This topic enumerates the **platform layer** —
the features whose *implementation* must be supplied by each host that
embeds Tel, either as a primitive or by implementing an interface that
`std` defines.

The other layer — the **Tel layer**, pure-compute features and convenience
wrappers written in Tel on top of the platform primitives — is the rest of
this chapter. The user-facing topics ([Strings and Text](06-strings-and-text.md),
[Time](09-time.md), [Networking](11-networking.md), …) describe the *surface*
a script sees regardless of host; this topic describes what a host must
implement to make that surface real.

A script always sees one `std`; the split is about *who supplies the
implementation*, not about a second namespace. The categorisation matters for
language implementers (porting Tel to a new host), not for script authors.

## Why the split

- **Portability.** Anything written in Tel runs on every host out of the
  box. Anything that touches the machine, the network, the clock, or the
  user has to be a platform contract — different hosts genuinely have
  different capabilities (a browser host has no filesystem; an embedded
  controller host has no network).
- **Stability.** Tel's stability maxim
  ([`../02-philosophy/01-priorities.md`](../02-philosophy/01-priorities.md))
  applies to the Tel layer in full. The platform layer's *interfaces* are
  also stable; the *implementations* are the host's, and may move at the
  host's pace (TLS protocol churn, HTTP version updates, OS-specific
  filesystem quirks).
- **Capability discipline.** The platform layer is where Tel's "I/O is a
  capability you receive, not a power you have" maxim is enforced. Every
  platform-supplied feature is reached only through a host-granted value.

Many features are **mixed**: a small platform core, with a Tel-written
shell of utilities around it. Path *manipulation* is pure Tel; *access* is
a capability. The IANA time-zone *data* is pure Tel; "what zone am I in"
is a capability. The platform layer here lists the part the host supplies;
the user-facing topic for the feature documents the Tel-side wrapper.

## What hosts must supply

Each item below names the interface (`std` defines it), the capability type
a host grants, and the user-facing topic where the surface is described.

### Core primitives

These are the foundations the rest of `std` builds on. A host *must* supply
them; without them no Tel script runs.

- **Tasks** — the concurrency primitive that replaces threads, processes,
  and async. The host decides what running a task means (OS thread, fiber,
  sequential continuation). See
  [Concurrency Utilities](12-concurrency-utilities.md) and
  [the task chapter](../14-concurrency-and-parallelism/02-tasks.md).
- **Channels** — closeable queues for task-to-task communication. Every
  host must provide at least an **SPSC closeable queue**; MPMC and the
  cloneable `Sender` shape are built on top, backed by the host's
  concurrent primitives where available. Single-threaded hosts implement
  channels as deterministic queues within the one thread. See
  [Channels and Message Passing](../14-concurrency-and-parallelism/06-channels-and-message-passing.md#portability--channels-work-on-every-host).
- **Allocation and raw arrays** — implementation backbone the collection
  types compile down to. Not directly visible to scripts; see
  [Core Collections](04-core-collections.md).
- **Monotonic timer for benchmarks** — `bench.now()`, low-overhead, never
  steps back. Listed separately from the wall clock because a host may
  offer one without the other. See
  [Observability and Logging](14-observability-and-logging.md).

### Time

- **`Clock`** — wall-clock "now". Capability. See [Time](09-time.md).
- **`MonotonicClock`** — monotonic instant for elapsed-time measurement.
  Capability, separate from `Clock`. See [Time](09-time.md).
- **Time-zone resolution** — "what zone is the host running in". The IANA
  data tables are pure Tel; this single piece is a capability.

### Randomness

- **`Random`** — PRNG capability; always available, seedable for
  reproducibility. The algorithm (xoshiro / PCG / similar) is platform
  choice. See [Randomness, Hashing and Crypto](15-randomness-hashing-and-crypto.md).
- **`CryptoRandom`** — CSPRNG capability; optional. A host without system
  entropy simply does not grant it. See
  [Randomness, Hashing and Crypto](15-randomness-hashing-and-crypto.md).

### Filesystem and process

- **`ReadDir` / `ReadWriteDir`** — narrowly-scoped filesystem capabilities.
  See [I/O and Filesystem](08-io-and-filesystem.md).
- **Temp files / dirs** — capability; the host may own the temp location.
  See [I/O and Filesystem](08-io-and-filesystem.md).
- **`Env`** — read-only environment-variable capability. See
  [OS and Process](10-os-and-process.md).
- **`Shell`** — subprocess execution capability. Optional; many hosts grant
  none. See [OS and Process](10-os-and-process.md).
- **Standard streams (`stdin`, `stdout`, `stderr`)** — capabilities passed
  to the script's entry point, not ambient. See
  [OS and Process](10-os-and-process.md).

### Networking

`std` defines the interfaces; hosts supply the implementations. No
Tel-side reference impl ships for these — protocol churn (TLS, HTTP/2/3)
moves faster than the stability maxim allows.

- **TCP / UDP sockets** — interface in `std`, host-provided impl. See
  [Networking](11-networking.md).
- **DNS resolution** — interface, host-provided.
- **TLS** — interface only. Host owns the protocol stack.
- **HTTP client** — interface only. Host wires its native HTTP stack
  (libcurl, JDK `HttpClient`, browser `fetch`, etc.).
- **WebSocket** — interface only, if the host supplies one at all.

### Logging and observability

- **Logging sink** — `std` defines the logger *interface*; one default
  in-process implementation ships, but the host may replace the sink (file,
  syslog, OpenTelemetry exporter, `/dev/null`). The interface is stable;
  the impl is not. See [Observability and Logging](14-observability-and-logging.md).
- **Profiler** — optional `profiler.flame(...)`, `profiler.heap_dump()`
  capabilities; supported where the host runtime can deliver them.

## Co-existence with the Tel layer

Several features have both halves visible in the same topic. The pattern is
always the same: the *manipulation* is pure Tel; the *interaction* is a
capability.

| Topic | Tel-side (pure) | Platform-side (capability) |
|---|---|---|
| Filesystem | Path parsing, join, normalise | Read, write, walk, lock |
| Time | `Duration`, calendar arithmetic, IANA tables | `Clock`, `MonotonicClock`, current zone |
| Networking | URL parsing, headers, request/response types | Sockets, DNS, HTTP transport, TLS |
| Randomness | UUID formatting, distribution helpers | `Random`, `CryptoRandom` entropy source |
| Logging | Log record shape, severity, structured tree | Sink that writes records somewhere |
| Process | `Command` builder (argv as data) | `Shell.run(cmd)` that actually spawns |

A script that uses only the Tel-side half of a topic does not need the
matching capability — generating a UUID *string* needs no randomness;
parsing a URL needs no network. Capabilities are required only at the
point where the script *acts* on the outside world.

## What hosts must *not* supply

The host fills in the items above and nothing else. It does **not** add
ambient globals, alternative numeric types, or competing collection types
on the side. The library's "one good way" rule
([`../02-philosophy/01-priorities.md`](../02-philosophy/01-priorities.md))
extends to platform bindings: a host that wants its native HTTP client
exposes it *as* the `HttpClient` capability `std` defines, not as a
parallel `host.http` module.

`TODO(open): host extensions for genuinely host-specific power (a game
engine's scene graph, an IDE's editor model) sit outside this list. They
are the host's, not Tel's, and reach the script through capabilities the
host names — see [FFI and Interop](../16-ffi-and-interop/04-embedding-tel-in-a-host.md).
The boundary between "platform-layer interface in std" and "host-specific
extension" needs a crisp rule.`

## See also

- [Standard Library Organisation](01-stdlib-organisation.md) — the
  two-layer split at a glance.
- [Antifeatures](../02-philosophy/04-antifeatures.md) — why I/O is always
  a capability.
- [Embedding Tel in a Host Application](../16-ffi-and-interop/04-embedding-tel-in-a-host.md) —
  how a host actually supplies the platform layer.
