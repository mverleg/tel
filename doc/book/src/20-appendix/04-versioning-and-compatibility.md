# Versioning and Compatibility

Tel makes a strong promise at the **source** level and an equally strong
*non*-promise at the **binary** level. Keeping these apart is what lets the
language be frozen-stable for scripts while leaving every implementation free to
change how it compiles them.

## Source compatibility — the strong promise

A Tel script that compiles today keeps compiling, and keeps producing the same
result, for the life of the language. The working title is **Tel1**; a change
that would break a valid Tel1 script ships as a *separate* language (Tel2), not
as a new version of Tel1. There are no editions and no opt-in language flags
that change the meaning of existing code (see
[priorities](../02-philosophy/01-priorities.md) and
[antifeatures](../02-philosophy/04-antifeatures.md)). This is the promise
script authors and host maintainers rely on.

## Binary / ABI compatibility — deliberately none

**Tel guarantees no binary or ABI compatibility of any kind, except between
artifacts produced by the *exact same compiler* compiling the program *together*.**

Concretely:

- There is **no stable ABI**. Two different compiler versions (or two different
  implementations) may lay out values, mangle symbols, order fields, and call
  functions completely differently. Object files or compiled modules from one
  must not be linked with those from another.
- A program is compiled **as a whole** by one compiler. Separate compilation is
  an *internal* optimisation of a single compiler invocation/toolchain, not a
  promise that independently-compiled pieces interoperate.
- Distribution is by **source** (crates ship Tel source; see
  [crates](../11-modules-and-packages/04-packages.md)), not by compiled
  binaries intended for cross-version linking.

### Why no ABI

A stable ABI is one of the heaviest long-term constraints a language can take
on — it freezes name mangling, memory layout, and calling conventions, and (as
Swift's *mangling regrets* document) bakes early mistakes permanently into the
binary interface. Tel does not need it: it is an **embedded, whole-program**
language whose artifacts are produced by one compiler for one host at a time. By
refusing ABI stability up front, every implementation stays free to change its
internals — layout, monomorphisation, symbol naming, the IR — without breaking
anyone, because nobody was promised those internals were stable.

This also removes a whole class of design hazards by construction. For example,
the *private-name discriminator* problem (Swift hashed a module name plus a file
*basename* into mangled symbols, which then could not change without breaking
archived data, and made two same-named files collide) simply cannot bite Tel:
mangled names are never a compatibility surface, so the compiler is free to use
any scheme — including a collision-free one (see the implementation note on
using a project-root-relative path when hashing). Nothing a script can observe
depends on it.

## What a host *can* rely on

- **Behaviour**, not representation: the same script gives the same observable
  results on every conforming implementation (the
  [determinism guarantee](../02-philosophy/03-features.md)), even though those
  implementations share no ABI.
- **Source** portability across hosts and compiler versions, within Tel1.

What a host must **not** rely on: linking compiled Tel artifacts across compiler
versions or implementations, the layout of any value, or any symbol name.
