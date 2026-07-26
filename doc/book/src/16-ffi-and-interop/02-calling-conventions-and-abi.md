# Calling Conventions and ABI

TODO: review

Tel has **no ABI**. There is no stable binary interface, no calling convention
exposed to scripts, no notion of linking a pre-compiled Tel artifact against a
host built separately. **Interoperability is source-level only.**

This is a deliberate choice, not an omission, and it follows from Tel's
priorities (see [`../02-philosophy/01-priorities.md`](../02-philosophy/01-priorities.md)).

## Source-only, never binary

A host integrates a Tel script by having the Tel toolchain process the
**source**, in one of two modes (see
[Interpreted vs compiled crossing](#interpreted-vs-compiled-crossing) below).
There is no third option where a Tel script is shipped as a compiled blob with
a frozen ABI that some other Tel-or-host build links against later.

Why no ABI:

- **No ABI compatibility to maintain.** An ABI is a second compatibility
  surface, separate from the language, that drifts and breaks in subtle ways —
  changing a read-only field to a property, adding an optional parameter,
  reordering a struct. Tel already commits to decades of *source* stability
  (see [`../02-philosophy/04-antifeatures.md`](../02-philosophy/04-antifeatures.md));
  it does not also sign up to freeze a binary layout.
- **Source is the readable artifact.** Shipping source keeps scripts
  inspectable and reviewable — important when scripts come from users or third
  parties.
- **One frontend, many backends.** Parsing and type-checking happen once, in
  the shared frontend, producing the [Xolir IR](#the-ir-is-an-internal-contract).
  Codegen is per-target. An ABI would have to span every target language at
  once; source plus a per-target backend does not.

A practical consequence: the script and the host are expected to be built with
**matching toolchain versions**. There is no "script compiled for version X
runs against host version Y" story, because there is no ABI to make that work.
Tel's stability commitment means a *source* script keeps compiling for decades
— but it is recompiled, not relinked.

## Interpreted vs compiled crossing

A host runs Tel in one of two modes, and the boundary crossing — calling host
operations, passing values in and out — looks different in each:

- **Interpreted.** The host embeds a small Tel interpreter. Cheap to ship,
  fast cold start. A boundary crossing is a call from interpreter code into a
  host function, marshalling values through the interpreter's runtime
  representation.
- **Compiled (AOT).** The Tel source is compiled ahead of time, via the IR, to
  the host language (or wasm). Peak throughput. A boundary crossing becomes an
  ordinary call in the generated target code.

**Observable behaviour is identical in both modes** — that is a hard guarantee
(see [`../01-overview/03-goals-and-non-goals.md`](../01-overview/03-goals-and-non-goals.md)).
What differs is performance and how the crossing is *implemented*, never what
the script sees.

The crossing also depends on *which* values cross: only immutable types may
cross the boundary, in either mode — see
[Embedding Tel in a Host](04-embedding-tel-in-a-host.md).

TODO(open): one genuinely open case — if the host does not
provide a facility (say, a sorted-tree collection) and the script uses one
implemented in Tel itself, *how does that Tel-implemented type cross the API
boundary* back to the host? The answer plausibly differs between interpreted
and compiled mode and needs to be worked out.

## The IR is an internal contract

Tel does have a precise interface between its frontend and its backends — the
**Xolir IR**, a serializable cross-language intermediate representation. But
this is an *implementation* contract between Tel's own components, not an ABI
exposed to scripts or hosts. A script author never sees it; a host integrator
never links against it. Its shape belongs in `impl-notes/`, not in this
chapter.

## See also

- [Embedding Tel in a Host Application](04-embedding-tel-in-a-host.md) — the
  host boundary in full.
- [Binding Other Languages](03-binding-other-languages.md)
- [`../01-overview/03-goals-and-non-goals.md`](../01-overview/03-goals-and-non-goals.md)
  — interpretation and AOT as equal, behaviour-identical modes.
