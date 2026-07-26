# Priorities and Trade-offs

When two designs are both reasonable, Tel picks the one that aligns with the priorities below. They are ranked from most to least important — higher priorities trump lower ones.

## The ranking

1. **Stability over novelty.** A Tel script written today should keep compiling and producing the same result for decades. The language is effectively frozen at 1.0 — the working title is **Tel1**, because the next breaking change would be a separate language called Tel2. New features land conservatively; backwards-incompatible change is a last resort. This is the *defining* commitment: many people will script Tel, and many host implementations will be maintained by people who are not the language authors. Both need a target that doesn't move.

2. **Safety and validation over flexibility.** Strict static typing is non-negotiable. Tel actively encourages *additional* validation on top — pre- and post-conditions, invariants, refined types (e.g. `Id[Person]`, `EuroAmt`, bounded numerics), capability-gated I/O, exhaustive matching. The compiler should catch mistakes before the script ever runs; runtime checks are the fallback, not the default. A little extra typing in the source pays back many times over when the script is read, reviewed, debugged, or ported to another host.

3. **Embedded scripts over standalone projects.** Tel is *primarily* a guest language inside a host application — a game engine, a Python/JS/JVM/Rust program, a message broker, a scientific tool — and is aimed at the small-to-medium scripts that live there, not at sprawling standalone codebases. "Embedded" here means *guest inside another program*, **not** *microcontroller-style embedded systems*. Tel is expression-oriented and tuned for small scripts; features that scale up to larger projects (modules, visibility, crate management) exist but are **opt-in** and stay out of the way for a 30-line modding hook. When a feature would be nice for standalone or enterprise work but hostile to embedding (ambient I/O, hidden global state, runtime version churn, OS-coupled assumptions, fat runtimes), embedding wins. Tel aims to be a *great* embedding language and a *decent* enterprise language — in that order.

4. **Readability over writability.** A line of Tel is read many times and written once — often by a different person, possibly years later, possibly an AI reviewing a diff. Clarity wins over terseness. Names, types, and asserts carry the explanation; comments are a last resort. Where this conflicts with brevity, prefer the form where *what looks correct is correct*.

5. **Familiarity over a "better" but novel surface.** Tel is allowed to look conservative. When a choice is between a syntactic form that already exists in mainstream languages and one that is theoretically nicer but unfamiliar, Tel picks the familiar one. The aim is that a reader from Python, Java, C#, Rust, JS, or Kotlin can read a Tel script with little surprise.

6. **High abstraction over low-level control.** Tel sits closer to Python than to C. Programmers should rarely have to think about heap vs stack, allocators, cache lines, or SIMD. This both keeps small scripts approachable *and* lets the same script run across very different host runtimes (interpreter, JIT, AOT-to-many-targets) without leaking implementation choices into user code.

7. **One good way over many clever ones.** Tel prefers a single, obvious way to do a thing over two clever ways or a long list of single-purpose constructs. Latest tricks are not a goal — features earn their place by composing well with what already exists, not by being novel. The standard library follows the same principle: it is curated and batteries-included so a script can do real work without pulling in a constellation of third-party crates. Cleverness that costs the priorities above doesn't pay.

"X over Y" isn't meant to suggest Y is useless, just that if we have to choose, X is more so. For example, larger scale programs are possible, but encapsulation is opt-in because embedded scripts are the priority.
