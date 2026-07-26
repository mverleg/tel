# Maxims of Tel

A short list of one-liners. Each is a concise answer to a recurring design question. They are not laws — the real ranking lives in [`01-priorities.md`](01-priorities.md) — but they capture the spirit of the language.

## Stability and embedding

- Same code, same results, decades later.
- Tel1 — the next change would be Tel2.
- Embedding is the point: a host runs Tel, not the other way around.
- One Tel script runs anywhere a host runs.
- "Embedded" means *guest in a program*, not *running on a microcontroller*.
- I/O is a capability you receive, not a power you have.
- Compiling untrusted code is safe — the build performs no I/O the source can trigger.
- The host decides what your code can do, never what it means.
- Reproducible by default — randomness, time, and I/O are injected, not ambient.
- Same inputs, same outputs — bit-for-bit, on every host. Determinism is a promise, not an emergent property.
- One script, many hosts — behaviour identical across them, including numerics, time arithmetic, and iteration order.

## Safety and validation

- The compiler tells you about your mistakes before your users do.
- Prevent, don't fix.
- Invalid states should be unrepresentable.
- Make the right thing easy and the wrong thing hard.
- When in doubt, fail — fast and loud.
- An error is never dropped silently — discarding one is explicit.
- Crash by default; recover at the boundary, not in the middle of the work.
- Mutability makes things easier, but its scope should be minimized.
- Compiler errors should teach, not just reject.
- A missing field is a value, not a default.
- Stale data is worse than no data — silence is a signal too.

## Readability and conservatism

- Code is read more often than written.
- Reasoning stays local — a function is understood from its signature and body, not the whole program.
- If it looks correct, it probably is correct.
- Names, types, and asserts carry the explanation; comments are a last resort.
- Surprise is a cost. Prefer the obvious.
- One way to do a thing.
- One sigil, one meaning: `!` marks unique/exclusive ownership (`!T`, `&!T`), never logical "not" — the absence of a capability is the word `not` (`: not Send`, `: not Unpack`).
- Error handling is explicit but terse.
- Pragmatism over purity.

## Scope and composition

- Big-project features are there when you want them, out of your way when you don't.
- The standard library should be enough for small, complete programs.
- Standard library features should be composable, non-overlapping and consistent.
- Performance is the host's to deliver, but the language must not stand in its way.

## Feedback

- Productivity is proportional to iteration speed — keep the edit–run–see loop short.
- The IDE is a first-class reader — prefer features it can amplify, reject features it can't follow.
- The compiler is the whole toolchain — no separate build step, no build scripts.

