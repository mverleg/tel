# Goals and Non-Goals

A concise statement of what Tel is trying to be — and what it is deliberately *not* trying to be. Use this page as the elevator pitch; the underlying ranking lives in [`02-philosophy/01-priorities.md`](../02-philosophy/01-priorities.md), the user-facing fit guide lives in [`02-when-to-use-tel.md`](02-when-to-use-tel.md).

## Goals

### Be a great guest

Tel is a **guest language inside a host program**. The host owns the OS, the build, the process, the data, and what the script is allowed to touch. Tel exists to make that arrangement pleasant and safe for both sides.

### Run the same script in many hosts

The standout case Tel optimises for: one script, embedded in a Java backend, a JavaScript browser bundle, and a Swift mobile app, producing **identical observable behaviour** in all three. This drives capability-based I/O, a fixed feature set, and the choice to specify behaviour rather than implementation.

### Stay stable for decades

Tel is effectively frozen at 1.0 — internally **Tel1**, because the next breaking change would have to be a separate language called Tel2. A script written today should still compile, run, and produce the same result in a runtime written ten years from now. There are no editions, no migration releases, no language flags.

### Catch mistakes before the user does

Strict static typing is non-negotiable. Refined types (`Id[Person]`, `EuroAmt`, bounded numerics), pre- and post-conditions, exhaustive matching, and capability-checked I/O move errors from runtime to compile time. When the compiler cannot prove something, contracts run as runtime checks — but the goal is always *prevent, don't fix*.

### Make I/O explicit and sandboxable

Every effect outside the program — files, network, time, randomness, environment — arrives as a **capability** the host passes in. There is no ambient `stdout`, no global clock, no implicit filesystem. A script that was not given a capability cannot use it; tests and reproducible runs fall out of this for free.

### Support both interpretation and AOT compilation

The same source must run on a small embeddable interpreter (cheap to ship, fast cold start) and through an ahead-of-time compiler (peak throughput). Hosts choose what fits their constraints; users do not write different code for the two modes.

### Stay small and readable

Tel is expression-oriented and aimed at **small to medium scripts**. The core is a small, composable set of orthogonal features. Surface syntax leans conservative and familiar — a Python, Java, C#, Rust, JS, or Kotlin reader should find Tel unsurprising. Readability beats writability; *what looks correct is correct*.

### Scale to medium projects when needed

Modules, visibility, packaging, and a package manager exist and are well-supported, but they are **opt-in**. A 30-line modding hook never has to mention any of them; a multi-file embedded library can lean on them without leaving the language.

### Ship a curated standard library

A small, dependable `std` covering data structures, iteration, numerics, text, and serialization — so a typical Tel script does not need to pull in a constellation of third-party crates just to be useful. Capabilities for I/O, time, and networking are part of the surface that `std` exposes through the host's gates.

## Non-goals

### Standalone applications and services

Tel does not own the build, deployment, process lifecycle, or operating system. There is no shipped web framework, ORM, CI tool, shebang runner, or venv manager. If a project's centre of gravity is "an application I deploy and operate," pick a general-purpose language.

### Competing with C on performance

Tel sits closer to Python than to C. No SIMD intrinsics, no GPU sub-language, no PGO hints in source, no `unsafe` blocks, no manual memory layout. Hot paths belong in the host language behind a capability boundary.

### Microcontroller-style "embedded"

"Embedded" in Tel means *guest in a normal host on normal hardware*, never *running on a 32 KB MCU*. Memory model, runtime, and stdlib all assume a normal heap. `no_std`-shaped constraints are out of scope.

### Cutting-edge or fashionable features

Tel is allowed to look old-fashioned. Features earn their place by serving the priorities, not by being trendy. No proc-macro systems, no runtime reflection, no `eval`, no operator-precedence customisation, no language-level effect rows.

### Implicit conversions and "do what I mean"

No truthy/falsy coercion, no silent numeric widening, no automatic string ↔ number conversion, no quiet integer overflow, no `null`. The compiler refuses ambiguity; the programmer writes the conversion. See [antifeatures](../02-philosophy/04-antifeatures.md).

### Exceptions and inheritance

Errors are values returned through `Result`-shaped types, never thrown. Polymorphism is via traits, never via class extension. Both rules buy readability and stability at the cost of some familiarity from Java/Python/C# users.

### Direct exposure of threads, locks, and the memory model

User code does not see threads, mutexes, atomics, or cache lines. Concurrency is expressed as **tasks**; the host's runtime decides what running a task actually means (fiber, thread pool, JS microtask, sequential continuation). A host with no concurrency at all still runs single-task code.

### Runtime version churn

No editions, no "Tel 1.4 features," no language flags. A new feature lands rarely and backwards-compatibly, or it does not land. The cost is conservatism; the payoff is that any Tel script runs in any conforming runtime.

### Ambient power

If a script *should* be able to open arbitrary files or hit the network without the host's say-so, Tel is the wrong tool. The capability gate is the point, not friction to remove.

## See also

- [When to Use Tel](02-when-to-use-tel.md) — the reader-decision framing of these same trade-offs.
- [Priorities and Trade-offs](../02-philosophy/01-priorities.md) — the ranked tie-breaker used when goals collide.
- [Features Tel Embraces](../02-philosophy/03-features.md) and [Antifeatures (and Why)](../02-philosophy/04-antifeatures.md) — the concrete inventory behind each goal and non-goal.
