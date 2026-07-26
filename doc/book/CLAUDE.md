# Tel — Programming Language Design

This repository contains the design documentation for **Tel** (Typed Embedded Language).

> **What "embedded" means here:** Tel is designed to be **embedded in host applications** — game engines, Python/JS/JVM/Rust programs, message brokers, scientific tools, IDE plugins, modding hosts. It is **not** a language for embedded *systems* (microcontrollers, constrained hardware). Assume the host is a normal program running on normal hardware; the constraints come from being a *guest* inside another program, not from tiny memory or no-OS targets.

> **Naming note:** Older material may refer to this language as *Mango*. Treat every such occurrence as *Tel*; do not preserve the old name in new documentation.

> **Pre-pivot material:** Many earlier ideas (in `inputs/`, in design notes, sometimes carried into chapter stubs) date from before Tel was scoped to embedding. When integrating such material, **actively push back on anything that conflicts with the embedding philosophy** — ambient I/O, OS-style assumptions, runtime version churn, features that only pay off for standalone projects, etc. Do not silently preserve a feature because an input mentions it. If unsure whether something fits, mark it `TODO(open):` with a note that says *"pre-pivot — re-justify against embedding."*

## Goal

Produce documentation that is detailed enough that a reader can:

- **Understand the language well** — know what features exist, how they behave, and why they were designed that way. The reader does not need experience with large/complex projects.
- **Implement the language** — have enough detail to build a working compiler or interpreter. The reader may make reasonable assumptions about common hardware (byte-addressed memory, two's-complement integers, IEEE 754 floats, typical OS facilities) and standard tooling. Such common assumptions do not need to be re-stated in the docs.

The documentation is **clear, not formal**. It uses prose, worked examples, and diagrams instead of formal grammars or operational semantics. Where precision matters (e.g. operator precedence, integer overflow rules) be precise; elsewhere prefer readability.

Every topic should answer three questions:

1. **What** — what the feature is and how it behaves.
2. **Why** — the reasoning behind the design, including alternatives considered and rejected.
3. **How it looks** — small, illustrative code examples.

## Structure

The documentation is organized as **chapters** (folders) containing **topics** (single markdown files).

```
.
├── CLAUDE.md              ← this file
├── book.toml              ← mdBook config (generation, not content)
├── theme/                 ← site theme overrides (generation)
├── preprocessors/         ← mdBook preprocessors (generation)
└── src/                   ← all documentation content
    ├── README.md          ← table of contents / entry point
    ├── inputs/            ← reference materials (see below)
    ├── impl-notes/        ← implementation scratchpad (see below)
    ├── 01-overview/
    │   ├── 01-introduction.md
    │   └── ...
    ├── 02-philosophy/
    │   └── ...
    └── ...
```

Documentation content lives under `src/`; generation files (`book.toml`,
`theme/`, `preprocessors/`) stay at the book root.

- One topic per file. If a topic grows past ~300 lines or starts covering two unrelated concerns, split it.
- Number prefixes (`01-`, `02-`, …) give a reading order but are not load-bearing — they can be renumbered.
- See `README.md` for the live table of contents.

Two chapters carry special weight:

- **`02-philosophy/`** is the **tie-breaker** for design decisions. It records the priorities, principles, and antifeatures that define Tel. When two pieces of input conflict, or two options seem equally reasonable, this chapter decides. Keep it crisp and opinionated — a short ranking of competing concerns (e.g. *readability over writability*), a list of one-line maxims, a list of features Tel embraces, and a list of features Tel deliberately excludes.
- **Use-cases / showcases** (near the end of the book): a series of small and medium worked examples that demonstrate Tel end-to-end. Each showcase is one topic file and should exercise multiple features in a realistic way, with commentary on *why* Tel is a good fit. This chapter sits late so it can reference earlier topics.

## Input materials

The `inputs/` directory holds reference materials — older notes, prototypes, partial specs, prior attempts. These are **source material, not authority**. They are expected to be:

- **Incomplete** — most topics are not covered, and many decisions are open.
- **Conflicting** — different inputs disagree with each other, with newer thinking, and sometimes internally. Resolving these conflicts *is* part of the design process; do not paper over them.

When integrating an input:

1. **Read it, do not copy it.** Extract the underlying intent and re-express it in the right topic file.
2. **Read top-down, but interpret newest-first.** Most input files are built by *prepending* new snippets at the top — the most recent thinking is near the start of the file and older material is further down. When two passages in the same file disagree, **prefer the earlier (higher-up) one**. Reading the file bottom-up can help reconstruct the chronological evolution of an idea.
3. **On conflict, look to philosophy.** When two inputs disagree, or an input disagrees with existing docs, the call is made by aligning with `02-philosophy/` (priorities, antifeatures, the Tel maxims). If the philosophy doesn't yet cover the question, *that* is a philosophy-chapter gap — flag it with `TODO(open):` and continue.
4. **Don't silently discard.** If you reject an input's approach, note the rejection (with reason) in a `TODO(open):` so the user can sanity-check the decision.
5. Inputs are never authoritative on naming — if an input contradicts a name already chosen (e.g. *Mango* → *Tel*), the chosen name wins.

## Implementation notes (`impl-notes/`)

The `impl-notes/` directory holds notes about *how* Tel could be implemented — IR shape, codegen strategies, bootstrap plans, runtime sketches, performance ideas, target-platform quirks. These are **not part of the documentation**. They exist so that implementation thinking is not lost between sessions, but they are not curated or guaranteed consistent with the docs.

Conventions:

- Treat `impl-notes/` as a scratchpad: short, free-form, no numbering required.
- `README.md` and chapter files **do not link to** `impl-notes/`. Readers of the documentation should never need to open it.
- When a note matures into a real design decision, lift it into the appropriate chapter. Optionally leave a one-line stub in `impl-notes/` pointing at the new location.
- If you find yourself writing user-facing material in `impl-notes/`, move it to a chapter.

## Workflow

When adding or changing content:

1. **Place it in the right topic.** Look for an existing topic that covers the area before creating a new file. Extending existing content is preferred over duplication.
2. **Align with existing decisions.** Read related topics first. If new content overlaps, integrate rather than restate.
3. **Never stop to ask.** When something is unclear, conflicting, or under-specified, do not pause the work. Instead:
   - Make a reasonable assumption and proceed.
   - Mark the spot inline with a `TODO(open): <question or concern>` comment in the markdown.
   - If you add a new document or a large section, add a `TODO: review` comment.
   - At the end of the work session, collect every open question into a summary message to the user. These are resolved in the next iteration.
4. **Cross-reference instead of duplicating.** Link to other topics with relative paths.
5. **Commit after each change.** One logical change = one commit. Commit titles ≤72 chars, body lines ≤72 chars, no signature, no JIRA prefix.

## Per-input integration loop

The main authoring pattern is to convert raw snippets in `inputs/` into polished topic files, one snippet at a time. The standard loop:

1. **Pick an input.** A file in `inputs/`, or a section of one (snippets are usually small — one snippet per pass is fine).
2. **Spawn an agent** to do the integration. Pass it: the snippet, the target chapter/topic, and a reminder of the conventions below. Keep the main conversation lean — only the user-visible decisions need to surface here.
3. **The agent's job:**
   - Identify the right topic file (or propose a new one if none fits, surfacing the choice as a `TODO(open):`).
   - **Rewrite, don't transcribe.** The snippet is raw thinking; the topic file is clear, structured prose with a *What / Why / How it looks* shape.
   - **Use pseudocode for examples.** Tel's syntax is not yet pinned down. Examples should look Tel-ish but stay loose — readers should understand the intent, knowing details will be fixed later. Do not invent syntax that pretends to be settled.
   - **Leave `TODO(open):` markers** wherever the snippet leaves something unresolved, where it conflicts with existing docs, or where a design call is being deferred.
   - On conflict, defer to `02-philosophy/` (see "Input materials" above).
4. **Review and commit.** The agent reports its open questions; the user resolves them in a later iteration. One commit per integration.

`02-philosophy/` is normally authored directly by the user, not via the integration loop — its content is the policy that the loop appeals to.

## Terminology: ownership vs mutability (push back when conflated)

Tel has **two orthogonal axes**, and the docs must never collapse one into the other:

- **Ownership / aliasing** — `Alias` (shareable; many paths may reach the value) vs **affine / unique** (one owner, moved not copied). This is what the `!` *type* sigil and the `uniq` *binding* keyword mark. `!T` is affine; bare `T` is `Alias`.
- **Mutability** — immutable vs mutable. A *separate* property.

All four combinations exist, so **"not `!`" does not mean "immutable"**:

| | immutable | mutable |
| --- | --- | --- |
| **shareable** (`Alias`, no `!`) | `Person`, `5` | **`ConcHashMap`, `Mutex`, atomics** — synchronised stdlib types |
| **affine** (`!`) | a linear file handle | `!List` builder |

The shareable-**and**-mutable cell (top-right) is the load-bearing counterexample: a `ConcHashMap` mutates through shared `&self` and is deliberately **not** a `!` type. So `!` / `uniq` track **alias-vs-unique, never immutable-vs-mutable**; mutability is a correlated-but-distinct property.

The "no `!` ⟹ immutable" shorthand holds **only for user-defined types** (user code has no interior-mutability escape hatch); the stdlib synchronised types are the sealed exception. **Push back whenever this is mixed up**: if a passage says "immutable" for a non-`!` value, stop and check whether it means "shareable / `Alias`" — and fix it if the synchronised-stdlib case would make the statement false. Likewise describe `!` / `uniq` / affine as *ownership*, not as *mutation*.

## Style

- **Markdown**, GitHub-flavored.
- **Diagrams** are encouraged. Prefer `mermaid` fenced blocks for anything non-trivial (syntax trees, type relationships, memory layouts, control flow). Small ASCII diagrams are fine when a mermaid block would be overkill.
- **Code examples** use fenced blocks tagged ` ```tel ` for Tel source. Use the relevant tag (`text`, `bash`, etc.) for non-Tel content.
- **Headings**: each topic file starts with a single `# Topic Title`. Use `##` and below for sections.
- **Tone**: explanatory and direct. Short paragraphs. Bullet lists where it helps scanning. Avoid filler.
- **Length**: terse where the design is obvious; longer where rationale matters. No padding.

## What this repo is *not*

- Not a formal language specification.
- Not a tutorial — the audience is someone studying or implementing the language, not learning to program.
- Not a standard library reference (that may come later, but the core design comes first).
