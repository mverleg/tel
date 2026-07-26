# Introduction

**Tel** (Typed Embedded Language) is a statically-typed language designed to be
**embedded inside other applications**. A host program — a game engine, a
financial model, a message broker, an IDE — supplies the runtime, the data, and
the I/O; a Tel script supplies behaviour.

> **Status:** Tel (formerly *Steel*, and earlier *Mango*) is under
> construction and not ready for use. This repository is its design
> documentation, not a released language.

## What Tel is for

Tel is a **scripting language for end users of a larger application**. The
typical script is small to medium — tens to a few thousand lines — and runs as
one part of a bigger system. Think of a modder writing a gameplay hook, an
analyst writing a custom valuation in a financial tool, or a per-message
transform in a data pipeline.

It is designed around three commitments:

- **Easy and secure to use from other languages.** A host embeds the Tel
  runtime and stays in control: a script can only touch what the host hands it.
- **Simple, but statically typed.** Tel stays small and approachable, but every
  script is checked by a strict static type system — mistakes surface while the
  script is written, not when a user triggers an unhappy path.
- **Feature-complete and frozen.** The language does not grow new versions.
  A script written today runs unchanged in any conforming runtime, so a host
  never has to chase runtime updates to keep old scripts working.

## Why Tel, given the alternatives

There are [many embeddable scripting languages](https://github.com/dbohdan/embedded-scripting-languages).
Two things set Tel apart:

- **Strict static typing.** Most embeddable languages are dynamically typed.
  Tel tells users about their mistakes as they write the script, and it can be
  compiled ahead of time as well as interpreted.
- **One script, many hosts.** Tel works hard to be embeddable in *many* host
  languages and to behave identically in each. Because the language is stable
  and has no ambient I/O, the same script runs the same way in a backend, a
  browser, and a phone app — useful for offline modes, lower hardware costs, and
  cross-system tools. Tel was originally created for one such tool, the schema
  evolution system [Apivolve](https://github.com/mverleg/apivolve).

## What Tel is not for

Tel is deliberately narrow. It is **not** meant for writing standalone
applications or services, for scaling to large multi-team codebases, or for
competing with C on raw performance. There is no file or network access unless
the host application chooses to expose it.

## How it looks

Syntax is not yet pinned down; examples in this documentation are pseudocode
that shows intent rather than settled spelling.

```tel
# A scoring rule the host hands to Tel. The host injects `clock`; the
# script has no other access to the outside world.
fn score(an_order: Order, a_clock: Clock) -> Result[Score, Reject] {
    if an_order.total <= EuroAmt(0) {
        return Err(Reject.NonPositiveTotal)
    }
    let my_age_days = a_clock.now().days_since(an_order.placed_at)
    Ok(Score.from(an_order, my_age_days))
}
```

## Where to go next

- [When to Use Tel](02-when-to-use-tel.md) — deciding between Tel and other
  embeddable languages.
- [Goals and Non-Goals](03-goals-and-non-goals.md) — the design objectives in
  full.
- [A Tour of Tel](04-tour.md) — a guided walk through the language.
- [Priorities and Trade-offs](../02-philosophy/01-priorities.md) — the ranked
  principles that decide design calls.
