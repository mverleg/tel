# TIP-0013: A Machine-Facing Toolchain Surface

**Status:** Draft
**Touches:** `18-tooling/09-editor-integration.md`, `18-tooling/07-linter.md`, `18-tooling/08-debugger.md`, `18-tooling/01-compiler.md`, `17-standard-library/14-observability-and-logging.md`, `01-overview/06-tel-for-ai-assisted-development.md`

<!-- TODO: review -->

## Summary

The toolchain already computes almost everything a non-human reader could want
— resolved signatures, capability sets, diagnostics, the symbol graph, the
dependency graph, a full-step trace of a run — but every one of those is
currently specified as something a **person** consumes through an **editor**.
There is no stated way for a *program* (a script, a CI job, a coding assistant)
to ask the same questions and get an answer it can parse.

This TIP records that gap as a **goal** and deliberately **defers the design**.
It is written now so the chapters stop implying "editor" every time they mean
"reader", and so the AI-assisted-development page has something to point at.

## Why this is a question at all

Two observations push in the same direction.

**Everything is already there, once.** The
[incremental compiler](../18-tooling/01-compiler.md) resolves the API;
the [linter](../18-tooling/07-linter.md) produces diagnostics and fix-its;
`tel graph` produces the [dependency graph and its
diffs](../11-modules-and-packages/08-dependency-graph-and-locking.md#visualising-the-graph-and-its-diffs);
`tel trace` produces [what a run actually did](../18-tooling/08-debugger.md#tracing-a-run-to-a-log).
The [editor integration](../18-tooling/09-editor-integration.md) chapter even
specifies *adjustable-detail projections* — public API only, business logic
only — and notes that a context-budgeted assistant wants exactly that. What is
missing is not the data; it is a way in that is not an editor.

**Other ecosystems found this valuable.** Elixir's Tidewave exposes a live
application's processes and documentation to a coding agent over MCP, and the
Elixir community credits that introspection — alongside fast feedback and
verified docs — for how well agents do in that ecosystem. Rust and Cargo make a
weaker version of the same move with `--message-format=json`. The pattern is
consistent: the tools that already know the answer publish it in a form a
machine can read, and the agent stops guessing from source text.

## The goal, stated

> A program should be able to ask the Tel toolchain the questions an editor
> asks, over a documented, parseable interface, without driving an editor.

Bounds that seem settled enough to write down:

- **Reader-side, not writer-side.** This is about *interrogating* the
  toolchain, not about the toolchain calling out to a model. The editor
  chapter's open question about
  [LLM hooks](../18-tooling/09-editor-integration.md#open-questions) stays
  separate, and its maxim holds: *the IDE is a first-class reader*, not a
  first-class writer.
- **One source of truth.** Whatever the surface is, it reports what the
  compiler and linter already decided. No machine-only lint set, no
  machine-only rendering of a type — the same rule that keeps editor-specific
  lints out of the IDE.
- **Headless.** It must work in CI and in a plain shell, because that is where
  an automated reader usually runs.
- **The embedding constraint is real.** Tel is a **guest** inside a host
  program, so "introspect the running system" cannot mean what it means for a
  BEAM node: there is no Tel-owned process tree to attach to, and the host —
  not the toolchain — owns the process, its lifetime, and its security
  boundary. Anything live is at the host's discretion; anything *offline* (a
  recorded trace, a resolved-API query, a diagnostic run) is not, and is the
  obviously safe half to design first.

## Deliberately open

`TODO(open): transport. A CLI with a stable "--json" output, an extension to
the LSP the compiler already ships, or a small separate server (MCP-shaped or
otherwise)? Lean: start with the CLI, because it is headless by construction
and costs no new protocol; promote to a server only if the query pattern turns
out to be chatty. Explicitly do not invent a Tel-specific protocol where LSP
already carries the same payload.`

`TODO(open): the query set. Candidates, roughly in decreasing confidence:
resolved public API of a module at a given detail level (the editor's
projections, addressable by name); diagnostics for a build; the fix-its and
deprecation rewrites available on a file; a dependency-graph diff for a change;
a recorded trace of one run; documentation lookup for a name at a pinned
version (see the docgen note on ecosystem-wide search in
[deferred features](../20-appendix/06-deferred-features.md#documentation-generator-tel-doc)).`

`TODO(open): schema stability. Tel promises source compatibility for the life
of the language; does a machine-readable output schema get a promise of its own,
a version field, or neither? Lean: a version field and additive-only changes —
weaker than the language promise, stronger than "whatever the tool prints".`

`TODO(open): live introspection at all? Given the embedding constraint above,
does the toolchain ever attach to a running guest, or does the offline half
(traces, replay, resolved API) cover the real uses? Lean: offline only for
Tel1; leave live inspection to whatever the host already exposes.`

`TODO(open): who consumes this besides an assistant? If the answer is "only an
assistant", the whole thing is suspicious — a surface worth having should also
serve CI checks, review bots, and ordinary scripts. Look for the second and
third consumer before committing to a design.`

## Prior art to read before deciding

- **Tidewave (Elixir)** — MCP server exposing a running application and its
  documentation to coding agents.
- **`cargo --message-format=json` / rust-analyzer** — the "the build already
  knows, so print it structured" end of the spectrum.
- **LSP** — the protocol that already carries most of these payloads, at the
  cost of assuming an editor-shaped client.
- **HexDocs search API** — documentation lookup pinned to a version, which is
  the piece Tel's [documentation generator](../18-tooling/10-documentation-generator.md)
  would have to grow.
