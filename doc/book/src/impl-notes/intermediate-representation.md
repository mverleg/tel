# Intermediate Representation (notes)

Scratchpad for **xolir** (cross-language IR; older name *TelIR*). User-facing
material lives in [`../18-tooling/02-compile-targets.md`](../18-tooling/02-compile-targets.md).

## Name

- Current: **xolir** (cross-language IR; pronounced *xo-l-ir*).
- Older inputs use **TelIR**. Treat any TelIR reference as xolir.

## Schema format

- Working choice: **Protobuf3**, with generated client libraries per host
  language.
- Reasons: mature codegen across Rust / Python / JVM / TS / Go; compact wire
  format; backends can be in different processes.
- Alternatives considered (not yet written up): Cap'n Proto, FlatBuffers,
  custom binary, JSON Schema.
- JSON Schema is more self-descriptive but slow and bulky; the IR's audience
  is compiler backends, not casual readers.

## Packaging (working coordinates)

These are pre-pivot artifact names from the input; record them here so they
do not get lost, but do **not** quote them in user-facing docs (the docs
should say "your host language has a xolir client" without pinning version
numbers).

- Rust (cargo): `xolir` — `cargo add xolir`
- Python (PyPI): `xolir` — `pip install xolir`
- Java (Maven Central): `com.apivolve:xolir` — group historically tied to
  Apivolve; revisit before publishing under the Tel name.
- TypeScript (npm): `xolir` — `npm install xolir`

Local build path (per input): `bash run.sh test` with `protoc`.

`TODO: confirm whether the Apivolve groupId is the long-term home or whether
xolir should move under a tel-* namespace.`

## Backend independence

- A backend may be implemented in any language, regardless of the host
  language it targets (Python backend can be written in Rust, etc.).
- Backends consume xolir; they do not amend and re-emit it. Front-end passes
  belong upstream.
- Interpreter is a backend over xolir. It may need a tighter execution form
  (bytecode / threaded code) below xolir for speed — undecided whether that
  is a separate "vm-ir" or just a final pass on xolir.

## Open questions

- **Generics representation in xolir.** Monomorphisation gives simple,
  fast backends but inflates code; dictionary passing keeps code small but
  pushes work onto the host runtime. Likely a hybrid: monomorphise where
  cheap, dictionary-pass at trait-object boundaries. Document once decided.
- **Rewrite stage before or after lowering to xolir?** Tracked in
  [`../18-tooling/01-compiler.md`](../18-tooling/01-compiler.md) as well.
- **Xolir versioning vs Tel1's "no editions" rule.** Tel itself is frozen,
  but xolir may need its own forward/backward-compat story for old IR files
  vs new backends.
- **Capability slots in xolir.** Capabilities must be explicit in the IR so
  backends cannot accidentally introduce ambient I/O. Concrete encoding
  unresolved.
- **Source-position carry-through.** Backends need enough span info for
  runtime errors to point back to Tel source. Trade-off against IR size.
