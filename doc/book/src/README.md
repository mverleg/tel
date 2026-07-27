# Tel

**Tel** (Typed Embedded Language) is a statically-typed, expression-oriented language designed for embedding inside host applications. Embedding — running Tel scripts safely inside a host app — is its core selling point; the expression-oriented surface is the *how*, not the *what*. This repository contains its design documentation.

---

## Where to start

Use the **sidebar** for the full table of contents — it is generated from `SUMMARY.md`, the single source of truth for the chapter list and reading order. A few orientation points:

- **[Overview](01-overview/01-introduction.md)** — what Tel is, when to use it, and a quick tour.
- **[Philosophy](02-philosophy/01-priorities.md)** — the priorities, maxims, and antifeatures that act as the tie-breaker for every design decision. Start here to understand *why* Tel is shaped the way it is.
- The middle chapters (lexical structure → standard library) are the language reference proper.
- **[Use Cases](19-use-cases/01-hello-world.md)** — worked end-to-end examples chosen to suit embedding (DSLs, formats, numeric tasks, concurrent data structures).
- **[Compiler Internals](19a-compiler-internals/01-overview.md)** — how the incremental query engine works: content-addressed caching, invalidation, hashing, concurrency. For readers implementing Tel rather than using it.
- **[Appendix](20-appendix/01-keywords.md)** — keyword and operator references, style guide, versioning, and design history.
