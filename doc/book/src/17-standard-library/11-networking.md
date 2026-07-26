# Networking

<!-- TODO: review -->

## What

`std` describes networking types — sockets, HTTP requests/responses — but, as
with all I/O, **there is no ambient network access**. A script cannot open a
socket or make an HTTP call on its own. Networking is reached only through a
**capability** the host explicitly grants.

## Why: the network is a capability

This is the same rule as files and time
([`../02-philosophy/04-antifeatures.md`](../02-philosophy/04-antifeatures.md)):
a script that was not handed a network capability cannot reach the network —
there is no global client to grab. The benefits are the usual ones:

- **Sandboxing by default.** Untrusted modding or pipeline scripts cannot
  exfiltrate data or call out unless the host allows it.
- **Reproducible tests.** A test injects a fake HTTP capability.
- **One script, many hosts.** A host with no network (or a restricted one)
  grants a narrower capability, or none; the same source still runs.

A network capability can be **narrow** — access to one host or one endpoint —
rather than blanket internet access. The host picks the smallest grant that
still lets the script work.

```tel
# The host injects `http`, scoped to one partner API.
fn fetch_terms(http: HttpGet, query: Query) -> Result[Terms, NetError] {
    let response = http.get(query.as_path())?
    Terms.parse(response.body)
}
```

## Layered surface

Like file I/O, networking is a *small layered family* of capabilities. A
host picks the level that matches what it is willing to grant — and the
script gets a type that lets it do only that:

- **`Socket`** — raw TCP/UDP. The lowest level; rarely granted to
  untrusted scripts.
- **`Http`** — HTTP client (`get`, `post`, …). Often the only network
  capability a host wants to expose. Can be narrowed to a single host or
  endpoint (see below).
- **`WebSocket`** — bidirectional connection over WS / WSS.
- **Higher-level wrappers** built on the above — gRPC, REST clients,
  message-queue clients — live in the crate ecosystem, not `std`
  (the bar for inclusion is "every embedded script needs it").
  `TODO(open): re-examine REST specifically. A typed REST client (request
  type, response type, paths and verbs as data) is a recurring ask
  because so many embedded scripts glue services together. The decision
  is whether the *typed* REST layer (declare an endpoint as a Tel record,
  get a generated client) belongs in std or in a blessed crate. Lean:
  blessed crate — the typing surface is heavy enough that one bundled
  REST library could not satisfy every host. Either way, JSON and HTTP
  capabilities make the manual form short.`

A host that grants only `Http` to a particular domain can still let a
script call that one partner API without exposing it to the rest of the
network.

`TODO(open): the exact layering — and whether `Http` is built *in Tel* on
top of `Socket`, or supplied by the host directly so each runtime can
defer to its native HTTP stack. Lean: both are valid host strategies; the
script-facing types are the same either way.`

## `select` across endpoints

Long-running network code needs to wait on several sources at once — a
request, a timeout, a cancellation signal. The library exposes a `select`
primitive that lives in
[`12-concurrency-utilities.md`](12-concurrency-utilities.md) and works
across any *awaitable* — channels, network reads, timers. Network code
uses the same `select`, not a network-specific variant. `TODO(open): pin
down whether `select` is a language construct, a stdlib function taking a
list of awaitables, or a macro; coordinate with the task model.`

## Networking and the task model

Network calls block. The interaction between an I/O-bound capability call and
Tel's task scheduler — keeping such calls off the CPU pool — is discussed in
[`12-concurrency-utilities.md`](12-concurrency-utilities.md) and worked
through in [`../19-use-cases/01-hello-world.md`](../19-use-cases/01-hello-world.md)
(the concurrent `renderPage` example, which fans out partner-site searches on
I/O and collation on the CPU pool).

A network capability call must respect the **timeout** parameter passed at
construction or at call time, measured against the injected `Clock` (see
[`09-time.md`](09-time.md)). There is no ambient network timeout.

## Retry and backoff

The retry / backoff machinery is generic to any fallible operation, not
networking-specific — see
[`17-scheduling-and-timed-ops.md`](17-scheduling-and-timed-ops.md) for the
retry adapters. Network code is the *most common* place they show up, but
the library does not bundle them with the network types.

## Supply chain

A crate that wants network access must declare the capability it needs (see
[`../18-tooling/04-package-manager.md`](../18-tooling/04-package-manager.md)).
A dependency that starts making network calls it never declared fails to
compile — scope creep becomes visible at review time, not in production.

## LLM API

A typed **LLM capability** — messages, system prompts, tool/function calls,
streaming token deltas, structured-output schemas — is **deferred**. The want is
real, but the provider API surface is still in flux mid-2026 and Tel's stability
commitment makes freezing a shape now a poor bet; a bare `Http` capability
covers it via a crate meanwhile. See
[Deferred Features → Typed LLM capability](../20-appendix/06-deferred-features.md#typed-llm-capability).

## See also

- [I/O and Filesystem](08-io-and-filesystem.md)
- [Concurrency Utilities](12-concurrency-utilities.md)
- [Data Formats and Serialization](13-data-formats.md)
- [Scheduling and Timed Operations](17-scheduling-and-timed-ops.md) —
  retry and backoff
