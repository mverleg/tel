# Compiler

<!-- TODO: review -->

## What

The Tel compiler turns Tel source into something runnable — either by feeding
an interpreter or by emitting code for a host language. It runs as a pipeline
of well-defined stages, offers **two compile modes** (a fast bulk mode and a
friendly incremental mode), and can run scripts under a **resource-bounded**
execution mode.

A central design point: **fast, clean compilation is a language-design goal,
not just an implementation detail.** Tel scripts are often compiled by the
host at load time, sometimes on every run, so compile speed is part of the
developer's iteration loop — and the maxim *productivity is proportional to
iteration speed* applies directly.

## Why compile speed is a language goal

Compile speed cannot be bought back after the fact by
caching pre-compiled data in a package index — that data is unavailable when
scripts are compiled fresh, which is the common case. Nor can it be sidestepped
by interpreting: Tel insists on full static type safety, so *some* complete
checking pass always happens. An unoptimised compile is therefore about as
cheap as interpreting, and gives type safety on top.

So compile speed is designed into the **language**, not just the compiler:

- **No macros or annotation processing.** Nothing runs user code at compile
  time to produce more code, so there is no expansion phase and no
  macro-driven cache invalidation. See
  [`../02-philosophy/04-antifeatures.md`](../02-philosophy/04-antifeatures.md).
- **A grammar with fixed lookahead.** The grammar is designed to parse with
  bounded lookahead and limited backtracking — no unbounded ambiguity. Priority
  rules for conflict resolution are acceptable; pathological grammars are not.
- **Limited, Java-style type inference.** Full whole-program inference can be
  cubic or worse, and — just as bad for Tel — it is an easy way to break
  backwards compatibility (a new `std` type can silently change what an
  expression infers). Tel deliberately restricts inference: public signatures
  are explicit, inference is local, and type information flows in essentially
  one direction. `TODO(open): the exact inference rules are a language-chapter
  decision; this topic only records that they are bounded for compile-speed and
  stability reasons.`

The implementation can lean on a fast lexer and reuse buffers across files, but
those are notes for `impl-notes/`, not user-facing design.

## Compile pipeline stages

The compiler runs these stages in order:

1. **Lex and parse** — source text to a syntax tree.
2. **Resolve** — resolve all references; identify which code is actually
   reachable/used. Name and type resolution fundamentally need at least two
   conceptual phases to handle mutual recursion.
3. **Type-check and infer** — check and infer all types (possibly fused with
   resolve).
4. **Lower to IR** — translate to a smaller instruction set: the cross-language
   IR, **xolir** (see [`02-compile-targets.md`](02-compile-targets.md)).
5. **Rewrite** — rewrite for `yield` (generators), task/async lowering, and
   tail-call optimisation.

`TODO(open): should the rewrite stage run before
or after lowering to the smaller instruction set? Doing rewrite-then-lower is
fewer stages but may make the rewrites harder to express. Left as an open
implementation-shape question.`

## Two compile modes

One compiler, two modes — the host or tool picks per situation:

- **Clean (bulk) mode.** For compiling the bulk of code. It does not maintain
  fine-grained within-file caches, does not produce richly formatted
  diagnostics, and skips the extra metadata (whitespace, spans) that tooling
  needs. It is tuned purely for throughput. It can also skip work for code that
  is provably unused.

- **Incremental & friendly mode.** Fine-grained, symbol-level caching;
  high-quality error messages; continues past the first error to report as
  many problems as it can; keeps whitespace and span metadata. Used when the
  clean pass fails (to explain why), for incremental recompiles, and to back
  the LSP server.

The two modes **share code but are not the same program** — the LSP and
the compiler reuse components, but are not fully
merged.

Incremental compilation is a **query compiler with content-addressed
caching**. Every compilation step — "parse this file", "signature of `F`",
"type-check `F`" — is a query whose **content key** hashes its stable
arguments plus the answer fingerprints of its *direct* dependencies, and the
cache maps content keys to answers. A cached answer is valid *by
construction*: if any transitive input had changed, the key would differ and
the lookup would miss — so there is no invalidation logic, and results can be
shared across runs and even across machines. **Early cutoff** falls out: an
edit whose intermediate answer is byte-identical (a formatting change → the
same syntax tree; a body edit → the same signature) stops propagating at that
level, so editing one function does not recompile its neighbours or its
callers. Dependencies are tracked at the **symbol level**; core answers carry
no presentation metadata (spans, formatting) — that lives in sidecar queries
demanded only when diagnostics are rendered, which is what keeps
semantically-identical answers byte-identical across the two modes. The
parser side can use a tree-sitter-style incremental parser that updates the
syntax tree as the source is edited. `TODO(open): how far to push
incrementality — symbol-level caching is the committed target; Unison-style
recompilation-free renames are noted as an aspiration only.`

## Interpreter vs AOT

The same source must run two ways
([`../01-overview/03-goals-and-non-goals.md`](../01-overview/03-goals-and-non-goals.md)):

- **Interpreted** — cheap to embed, fast cold start. The interpreter executes
  xolir; see the interpreter naming in
  [`02-compile-targets.md`](02-compile-targets.md).
- **Ahead-of-time compiled** — emit code for a host language for peak
  throughput.

Observable behaviour is identical either way; users do not write different
code for the two. Because Tel demands full type checking regardless, even the
interpreted path runs a complete checking pass first.

## Resource-bound execution mode

For running **untrusted** scripts, the compiler/runtime offers a
resource-bounded mode that caps a script's footprint so a runaway or malicious
script cannot hang or starve the host:

- A **maximum operation count** — the script is stopped if it executes too
  many operations (preventing an infinite loop from eating the CPU).
- A **maximum memory** bound — more conventional, but it must also prevent
  *garbage-collector* overload, not just out-of-memory: a script that churns
  allocations without exceeding peak memory can still drown the host in GC.

`TODO(open): how a single "operation" is counted is unresolved — does a hash
lookup count as one op or as its real cost? Define the accounting before this
mode can be specified.`

This mode pairs with capability-gated I/O
([`../17-standard-library/08-io-and-filesystem.md`](../17-standard-library/08-io-and-filesystem.md)):
capabilities bound *what* a script can touch, resource limits bound *how much*
it can consume.

## Never panic

The compiler and runtime must **never panic** on a host — at most they may
abort on out-of-memory. A guest that can crash its host is not safely
embeddable. Errors in Tel code are surfaced as diagnostics or as values, never
as a runtime crash of the embedding process.

## See also

- [Compile Targets](02-compile-targets.md)
- [Editor Integration](09-editor-integration.md)
- [Antifeatures](../02-philosophy/04-antifeatures.md)
