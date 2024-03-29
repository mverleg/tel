
![Test & lint](https://github.com/mverleg/tel/actions/workflows/test-lint.yml/badge.svg)
![fuzz & perf](https://github.com/mverleg/tel/actions/workflows/fuzz-perf.yml/badge.svg)

**Tel** (Typed Embedded Language) is a statically-typed language that can be embedded in other applications.

**Tel (formerly Steel) is under construction and not ready for use.**

It is designed to be:

* Easy and secure to use from other languages.
* Simple to use, but with the safety of strict static types.
* Feature-complete, no need to update runtimes to run new scripts.

## Goals

The goal is to provide a simple language that end users can write small or medium scripts in, to run as part of a larger application. It is stable, so no mismatch between script and runtime versions, and no constant updates.

The simplicity, safety and the ease of use from other languages make Tel convenient as a scripting langauge. For example to provide modding capabilities in a video game, or to allow users to provide custom calculations in a financial or scientific model.

These are nice use cases, but there are [many alternatives](https://github.com/dbohdan/embedded-scripting-languages), why pick Tel?

You may choose Tel over a lot of other embeddable scripting languages if you prefer strict static typing, to tell users about their mistakes when they are writing the script, rather than when running. It also allows ahead-of-time compiling, in addition to interpreted mode.

But where Tel really shines is when scripts have to run embedded in more than one language. Tel tries hard to be embeddable in many languages by its design choices, most importantly by providing a stable langauge, so that any Tel script runs in any embedding environment.

An example of such a multi-language case are user scripts that can run in the backend, but also in a browser, or on a phone app. Support offline mode and save on hardware costs.

Another example is for scripts in a cross-system tool, like a message broker or message encoding that provides scripting. Tel was originally created for such a tool, [Apivolve](https://github.com/mverleg/apivolve).

## Non-goals

The goal is **not** to provide all the latest tricks, scale to thousands of lines, or write independent services. There is no file or network access unless the host application exposes it.

