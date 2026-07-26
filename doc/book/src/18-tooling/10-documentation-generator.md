# Documentation Generator

`tel doc` is the planned toolchain subcommand that turns a project's source —
declarations, doc comments, contracts, and examples — into browsable reference
documentation (HTML, with single-page Markdown and structured JSON from the same
pipeline), rendering the *resolved* API off the
[incremental compiler](01-compiler.md) rather than re-parsing the surface text.

The tool is **deferred**. It is wanted — the maxim *the standard library should
be enough for small, complete programs* implies findable reference docs for every
public name — but it is a substantial, fully-additive pipeline whose payoff only
lands once there is a language and a stdlib to document. Doc comments themselves
stay a committed language feature
([comments](../03-lexical-structure/07-comments.md)); only the generator waits.

The full design — comment shape, what the generator extracts, examples-as-tests,
where business goals and invariants live, versioning, inheritance, and the open
questions — now lives in
[Deferred Features → Documentation Generator](../20-appendix/06-deferred-features.md#documentation-generator-tel-doc).

## See also

- [Deferred Features](../20-appendix/06-deferred-features.md#documentation-generator-tel-doc)
  — the deferred design in full.
- [Testing](../14-testing/01-testing.md) — examples as tests share the test runner.
- [Comments](../03-lexical-structure/07-comments.md) — the `##` doc-comment
  language feature that survives independently of the generator.
