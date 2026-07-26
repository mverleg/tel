# When to Use Tel

Tel is a embedded language for **other programs**, for example to parametrize a financial model, to describe schema evolutions, or to script a game engine. What sets it apart from other such languages are type safety and running the same script in multiple languages.

A flexible host application supplies the runtime and the I/O, and the data; a Tel script supplies behaviour. This page is for someone deciding between Tel and one of the many [alternative embeddable languages](https://github.com/dbohdan/embedded-scripting-languages). The deeper rationale for *why* Tel is shaped this way lives in [`02-philosophy/`](../02-philosophy/01-priorities.md).

## The sweet spot

Tel is a good fit when most of the following hold:

- A larger application — game engine, financial model, messaging tool, IDE plugin, modding host, scientific pipeline — wants to expose scripting to its users.
- Scripts are **small to medium**: tens to a few thousand lines, not multi-team services.
- Mistakes should be caught **before the script runs**, not when a user hits an unhappy path in production.
- The host wants to keep tight control over what scripts can do: no surprise filesystem access, no hidden network calls, no ambient time or randomness.
- Scripts must keep working **for years** with no editions, no migration guides, no "this used to work in Tel 1.4."

If most of these feel irrelevant, jump to [When to use something else](#when-to-use-something-else).

## Where Tel really shines: one script, many hosts

The standout case for Tel is a script that has to run inside **more than one host language** and produce identical results in each.

Examples:

- **Backend + browser + mobile.** A pricing rule, validation routine, or piece of game logic that runs in a Java or Go backend, again in a JavaScript front-end for offline mode, and again in a Swift or Kotlin mobile app. Writing it three times invites drift; writing it once in Tel and embedding the Tel runtime in each host keeps behaviour bit-identical and saves on hardware costs.
- **Cross-system tools.** A message broker, schema migration tool (e.g. [Apivolve](https://github.com/mverleg/apivolve), Tel's original home), encoding format, or workflow engine that exposes scripting hooks. The same hook should mean the same thing whether the host is Python, Rust, or the JVM.
- **Long-lived plugin formats.** A modding API, plugin SDK, or saved-rule format that must stay readable a decade from now without forcing every host to upgrade in lockstep.

Tel optimises hard for this scenario:

- **Stable surface.** Tel is effectively frozen at 1.0 — see [stability priority](../02-philosophy/01-priorities.md). No editions, no breaking releases. The next breaking change would have to be a separate language called Tel2.
- **No ambient I/O.** Anything outside the program (files, clock, network, randomness, env vars) arrives as a *capability* the host passes in. A browser host without a filesystem simply does not hand out that capability, and every script must cope with the absence up front.
- **Two execution modes from one source.** A host can interpret Tel (cheap to embed, fast cold start) or compile ahead of time (peak throughput). Observable behaviour is identical either way.
- **Strict static typing.** Most script mistakes are caught when the script is loaded, not when the user clicks the button that triggers the broken branch.

## Typical good fits

- **Modding and user scripts in games.** Untrusted authors, long-lived save files, no acceptable crash, restricted I/O. Capabilities make sandboxing the default rather than an afterthought.
- **Custom calculations in scientific or financial tools.** A user writes a formula, valuation, or model. Strict types and refined numerics (`EuroAmt`, bounded ranges, `Id[Person]`) prevent quiet errors; injected clock and RNG keep results reproducible and auditable.
- **Configuration with logic.** When a static config file is not enough but Turing-complete YAML is a horror, a small Tel script with capability-gated I/O is a better answer.
- **Plugin and extension points in desktop or IDE apps.** Stable language means a plugin written today still loads in next year's release.
- **Internal DSLs.** Tel's small, composable core is enough to host a DSL for markup, query, build rules, or workflow definitions without inventing a language from scratch. A host can give the DSL its own flavour — a declarative configuration surface, a game-engine scripting style — while reusing Tel's parser, types, and capability sandbox underneath. `TODO(open): how hard should the docs lean into DSLs as *the* headline use case? Embedding hosts often want to expose their own dialect/style on top of Tel; decide whether that is a first-class, documented authoring path (host-defined preludes, restricted surfaces, tagged literals) or just one use case among several.`
- **Embedded rules in messaging and data pipelines.** Per-message routing or transformation logic that must be cheap to evaluate, safe on untrusted input, and identical across every consumer of the pipeline.

## A taste

Pseudocode — Tel's syntax is not yet pinned down, but the shape is settled:

```tel
# A scoring rule a host hands to Tel.
# The host injects `clock` and `log`; the script has no other I/O.
fn score(an_order: Order, a_clock: Clock, a_log: Log) -> Result[Score, Reject] {
    if an_order.total <= EuroAmt(0) {
        return Err(Reject.NonPositiveTotal)
    }
    let my_age_days = a_clock.now().days_since(an_order.placed_at)
    a_log.info("scoring order", an_order.id, my_age_days)
    Ok(Score.from(an_order, my_age_days))
}
```

The same source compiles and runs identically inside a Java backend, a JS browser bundle, and an iOS app — none of them gives the script ambient access to files, the network, or the system clock.

## When to use something else

Tel is deliberately narrow. Pick another tool when:

- **You are writing a standalone application or service.** Tel does not own the build, the deployment, the process lifecycle, or the OS. A standalone backend, CLI, or batch job is a better fit for a language whose ecosystem ships those things — Go, Rust, Python, TypeScript.
- **You need raw performance or low-level control.** Tel sits closer to Python than to C. No SIMD intrinsics, no GPU sub-language, no `unsafe`, no manual memory layout. If a hot inner loop must saturate cache lines, write it in the host language and expose it as a capability.
- **You need cutting-edge language features.** Tel is allowed to look conservative; new tricks land slowly or not at all. If part of the appeal is the latest macro system, effect system, or dependent types, Tel is not it.
- **You target microcontrollers or `no_std` environments.** "Embedded" in Tel means *guest in a normal program on normal hardware*, not *running on a 32 KB MCU*. Pick a language designed for that constraint.
- **The script is the whole project.** A large monolith with its own deployment, build system, crate ecosystem, and CI story will outgrow Tel's intentionally small surface. Tel scales to medium projects, not to enterprise codebases.
- **You want ambient power.** If a script *should* be able to open arbitrary files or fire HTTP requests without the host's say-so, Tel will frustrate you — that capability gate is load-bearing, not friction to remove.

## Quick checklist

Tel is likely the right call if you can answer "yes" to most of these:

- Will Tel run inside a larger host program?
- Do you want script errors caught at load time rather than at runtime?
- Will scripts come from users, customers, or third parties whose code you do not fully trust?
- Do you need the same script to run in more than one host language, or to keep working unchanged for years?
- Are you happy for the host to gate I/O, time, and randomness?

If most answers are "no", a general-purpose language is a better fit — and that is exactly what Tel is *not* trying to be.

## See also

- [Goals and Non-Goals](03-goals-and-non-goals.md) — the deeper design objectives Tel commits to.
- [Priorities and Trade-offs](../02-philosophy/01-priorities.md) — the ranking that decides design calls.
- [Features Tel Embraces](../02-philosophy/03-features.md) and [Antifeatures (and Why)](../02-philosophy/04-antifeatures.md).