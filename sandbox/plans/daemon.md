# Daemon, CLI, and processes — design

Status: **direction decided 2026-07-05; implemented 2026-07-06** as the
`sandbox-daemon` crate (binary `telsb`) plus the file monitor in
`sandbox/src/monitor.rs`: one daemon per `tel.toml` root, ad-file discovery
with token auth, spawn-on-demand, tonic/gRPC
(Handshake/Compile/Watch/Shutdown), exact-version self-replacement,
`--no-daemon` and no-marker fallbacks running in-process. Still future: IDE
point queries, broadcast diagnostics, client-spawned execution, persistence
(roadmap Phase 3). Decisions: one daemon per workspace root (marker file,
innermost wins); tonic/gRPC transport with per-request diagnostic streams;
exact-version handshake, client wins; `--no-daemon` stays as a cold
in-process path; daemon compiles, spawned children execute. Related:
[fast-mode.md](fast-mode.md) (the IDE demand policy assumes this long-lived
session), [roadmap.md](roadmap.md) Phase 3 (persistence makes daemon
restarts warm).

## Process model — one daemon per workspace root

A language server is a background process; exactly one covers a directory
tree. The engine only pays off inside a persistent process (the whole
incremental design assumes a warm `Compiler`), and the in-memory layer —
dependency graph, reverse edges, dirty marks — is only coherent in one
process. So: one daemon = one cache domain = one file monitor = one root.

- **The root is defined by a marker file** (a minimal `tel.toml`; the
  language has no crate/workspace concept yet, so the marker introduces one
  and defines *nothing else* for now). Clients find the root by walking up
  from the working directory; **innermost marker wins**, which also
  guarantees no two servers' trees overlap.
- **Discovery** (watchman/Bazel/Gradle pattern): the daemon writes an
  advertisement file `{port, pid, version, auth_token}` under
  `$XDG_RUNTIME_DIR/tel/<hash-of-root-path>/`, held under an OS file lock so
  a crashed daemon leaves a detectably stale ad. Not inside the workspace:
  keeps repos clean, works on read-only checkouts, inherits per-user
  permissions.
- **Spawning**: client reads the ad and connects; no ad or dead lock → the
  client re-execs itself as `tel server --daemon` and waits for the ad.
- **Security**: bind localhost only; every connection presents the token
  from the ad file. The trust boundary is "can read the owner-only ad
  file", *not* "can reach the port" — the daemon executes code, so this is
  load-bearing.

## Transport — tonic/gRPC (committed)

Protobuf via prost/tonic, sharing the `.proto` toolchain already used by
telir. Committed 2026-07-05 after weighing websocket.

- **Rejected: protobuf over websocket.** Everything the standing-connection
  requirement actually needs — persistent multiplexed connection, message
  framing, request/response correlation, server→client streaming,
  cancellation propagation — is what HTTP/2 + gRPC provide and what a raw
  websocket would require hand-rolling. Websocket's genuine advantage is
  browser clients (a playground); if that materializes it becomes a second
  listener, not the CLI protocol.
- **Hedge**: the protocol is defined in transport-neutral `.proto` files,
  and tonic types stay out of the engine's public API (the server crate
  translates at its boundary) — a transport swap survives the messages.
- **Dependency note**: prost/tonic approved and added 2026-07-06 with the
  `sandbox-daemon` crate (same versions as telir).

### Connection and API shape

One persistent HTTP/2 connection per client; each request is a multiplexed
stream over it. Initial service surface:

- `Handshake` — version/token exchange (below).
- `Compile(entry)` — *server-streaming*: `Diagnostic` / `Progress` events,
  then `Done{result}`. The CLI blocks reading the stream; blocking is a
  client choice, not a protocol property. Client dropping the stream **is**
  the cancellation signal.
- `Watch(entry)` — same shape, never-ending: one diagnostic batch per
  invalidation wave. The daemon-hosted `run_watch` surface.
- `Shutdown` — drain and exit (used by version upgrades).
- Later: unary IDE point queries (`Hover`, `GotoDef`, …), issued
  concurrently over the same connection.

**Rejected: one bidirectional über-stream** carrying all message types —
that reimplements LSP's JSON-RPC correlation inside gRPC, the exact work
gRPC exists to avoid. An eventual LSP adapter instead translates LSP
requests onto these same RPCs, with `publishDiagnostics` fed from `Watch`;
the JetBrains plugin can use either surface.

**Diagnostics are owned by the request that provoked them** (each
`Compile`/`Watch` stream carries its own), not broadcast on a separate
subscription channel. Stateless and trivially correct with multiple
clients. The one thing this can't do — an editor showing squiggles for a
compile the CLI triggered — is named here deliberately; add a broadcast
`Subscribe` stream if and when that's wanted.

## Versioning — exact match, client wins

Client and daemon must be the exact same build for now, which makes
protobuf a serialization convenience rather than a compatibility contract —
messages can change freely until version skew is deliberately supported.

- The handshake carries a build fingerprint; reuse the cache **schema
  hash** machinery (same concept: "do these builds agree on data layout").
- On mismatch the **client is the authority**: it calls `Shutdown` on the
  old daemon, waits, and spawns its own version — upgrades self-heal
  (Bazel/Gradle behavior). This strengthens the case for Phase 3
  persistence: without it every upgrade is a cold cache.

## `--no-daemon` — cold, hermetic, always available

The engine stays a library; the CLI keeps an in-process path (today's
`run_file`). CI and tests want hermetic runs, and the path keeps the daemon
an honest thin host rather than the only way in.

- Default: **no persistent cache** — cold per invocation, warm within it.
- If a warm-but-daemonless mode ever matters: sharing the future LMDB store
  is safe by construction — content-store entries are valid forever (keys
  chain through content fingerprints + schema hash), so concurrent writers
  can only duplicate work, never corrupt; LMDB itself is single-writer/
  MVCC-readers across processes. Read-only open is the conservative first
  step. No design work needed now.
- Corollary: the daemon's exclusivity is *not* protecting the store (the
  store protects itself); it is the single warm home of the in-memory
  layer, the file monitor, and IDE sessions.

## Execution — the daemon compiles, spawned children execute

Guidance adopted: `run` and `test` are the same thing — *execution* — split
by who owns the I/O, and policy decides which process spawns the child:

- **`tel test`** → daemon-spawned children: output captured per test, runs
  parallelizable, and — the payoff — **test results become cacheable**
  (Bazel-style: a test whose input closure is unchanged doesn't rerun).
  "Exec results stay uncached" (TODO.md non-goals) is about
  exec-as-side-effect; a test is exec-as-assertion and its pass/fail is a
  value.
- **`tel run`** → eventually client-spawned: the invocation's context —
  real TTY semantics (`isatty`, raw mode, colors), signal delivery
  (Ctrl-C hits the program), exit codes, cwd/env — is inherited for free
  by a client child and is accidental complexity to proxy over RPC. Note
  this is *not* about insulating the daemon from side effects; a child
  process does that whoever spawns it. Rule: whoever owns the I/O
  contract spawns the child. Low priority: the common path is likely
  compile-an-artifact-then-run-it-yourself, which converges on the cargo
  model (client tool spawns the binary, the compiler never runs your
  program) — client-spawn arrives naturally with the AOT backend.
- **Near term**: the language's only I/O is `print` (no stdin), so both can
  execute daemon-side with output streamed as `Compile`/`Watch` events —
  which also defers serializing the monomorphised AST for handover. The
  client-spawned path becomes real work when the language grows
  stdin/env/exit codes; the streamed-event API already doesn't preclude it.

## File monitor — implemented (`src/monitor.rs`)

The daemon-side event source for `invalidate` + `run_watch`, usable today
without the daemon via `Compiler::run_watch_loop`.

- **Shape**: backends implement `FileMonitor` (`watch`/`unwatch` roots
  only); delivery — batching, dedup, closed-stream termination — lives once
  in `ChangeStream` (`next_batch()` coalesces everything pending into one
  wave). Backends: `DiskMonitor` (the `notify` crate — dependency approved
  2026-07-05 with this work) and `MockMonitor` (tests inject events; a
  cloneable `MockHandle` allows emitting across tasks). Future backends
  (editor buffer overlays, polling fallback, the Phase 3 virtual source
  backend) implement the same trait and feed the same stream.
- **Access events are filtered, load-bearing**: the compiler reads watched
  files while recomputing, so forwarding reads would make every watch run
  schedule the next one forever. Everything else is forwarded — extra
  events are hints (early cutoff absorbs them); missed events are stale
  serves (watch contract), so backends over-report rather than filter.
- **Path identity**: the engine interns paths textually, so events must
  carry the same form the compiler saw. Practice: compile a canonical
  absolute entry path; `DiskMonitor::watch` canonicalizes roots so the OS
  reports canonical paths. The daemon makes this systematic (it controls
  both the entry path and the watch root); a stricter fix (canonicalize at
  intern) is deferred until it bites.

## Open questions

- Mono-AST serialization for client-side execution — deferred until the
  language has real I/O (see Execution above); postcard is the presumptive
  format (already planned for LMDB).
- Broadcast diagnostics (`Subscribe`) — add only when a second client wants
  squiggles for compiles it didn't initiate.
- Test-result caching — design when `tel test` exists; requires
  daemon-spawned execution (decided) and hermetic input tracking (not yet
  designed).
- ~~Marker-file name~~ — decided with the discovery code: `tel.toml`,
  contents ignored for now (it defines the root and nothing else).
- Spawn race on cold start: two clients can each spawn a daemon; the
  loser's ad is overwritten and the orphan idles until killed. Recorded as
  a known simplification in `sandbox-daemon/src/discovery.rs`; a file lock
  can arbitrate later without changing the layout.
