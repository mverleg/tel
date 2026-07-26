# Formatter

<!-- TODO: review -->

## What

The formatter (`tel fmt`) rewrites Tel source to fix layout that is
*unambiguously* bad — broken indentation, inconsistent spacing, stray
punctuation — and leaves everything subjective alone. It is **deterministic**
and **idempotent**: re-running it on its own output is a no-op. It is
deliberately **less strict** than the rest of Tel: where a layout choice is a
matter of taste, a human still does a better job than any automated rule, so
the formatter does not touch it.

> A formatter that produces *stable* output is far more valuable than one that
> produces *pretty* output.

That is the whole design. The formatter exists to remove noise from diffs and
tidy obviously-wrong layout — not to impose a single house style on every line.

Two flags:

- `tel fmt` — fix files in place.
- `tel fmt --check` — print the diff and exit non-zero if anything is
  unambiguously wrong. The CI mode.

The minimal formatter is **built in**, shipped *with* the compiler, not a
separately versioned side project — so it can never drift from the grammar.
There is no plugin system and no configuration.

## What it fixes, and what it leaves

It **fixes** (none of these is a matter of taste):

- Indentation that does not match block nesting.
- Inconsistent or misleading spacing around tokens.
- Trailing whitespace, a missing final newline, pathological runs of blank
  lines.
- Import lists — sorting, grouping, dedup.
- Parentheses around the *rarer* operator combinations where
  [precedence](../04-syntax/04-precedence-and-associativity.md) would otherwise
  leave a reader guessing.

It **leaves alone** (these carry intent a tool cannot judge):

- Intentional vertical alignment of related bindings.
- Hand-grouped `match` arms or argument lists.
- How a long expression is broken across lines. **The formatter does not
  reflow or wrap at a column.** Whether a line runs long is the author's call;
  chopping every line over 100 columns destroys more intent than it saves.
- Blank lines the author placed to separate logical groups (beyond collapsing
  pathological runs).

The rule of thumb: if two readers could reasonably disagree about a layout, the
formatter keeps the author's choice. It changes what is *wrong*, never what is
merely *different*.

## Why built-in but minimal

- **Code is read more often than written.** Unambiguous layout noise — bad
  indentation, ragged spacing — is a tax on every reader, so the formatter
  removes it. Subjective layout is *not* noise, so it stays.
- **No drift.** The compiler is the whole toolchain; a separate formatter
  project would inevitably lag the compiler's grammar, exactly the churn the
  [stability priority](../02-philosophy/01-priorities.md) avoids.
- **Humans format the subjective part better than rules do.** A *total*
  canonicaliser (gofmt-style: exactly one output per input, all equivalent
  programs mapping to the same text) was considered and **rejected**. Every
  reasonable-but-different layout it would flatten is information lost, and the
  maxim *if it looks correct, it probably is correct*
  ([`../02-philosophy/02-maxims.md`](../02-philosophy/02-maxims.md)) cuts
  toward trusting a deliberate layout. Tel keeps the formatter narrow so it
  never fights a human on a judgement call — that is why it is *less* strict
  than the rest of the language, not more.

## Configuration: none

The formatter takes **no configuration**. No `.telfmt`, no tab-vs-space
preference, no per-file pragmas. There is no line-width knob either, because
the formatter does not wrap lines at all — so the question is moot rather than
suppressed.

## What the formatter does *not* do

- **Change meaning.** It never renames, reorders declarations, or touches a
  call site. Output is observationally identical to input.
- **Reflow or wrap lines.** Line breaks inside an expression are the author's.
- **Run lints or apply fix-its.** Those live in the [linter](07-linter.md);
  the formatter and linter are siblings, not the same tool.
- **Rewrite comments.** Content is untouched; only leading indentation is
  normalised.
- **Refactor.** The semantic-tree refactor surface (Go-style `gofix`,
  IntelliJ structural replace) is a distinct tool — see
  [open questions](#open-questions).

## Integration with editors and CI

The formatter is built to be called from many surfaces:

- **On save** in the editor, via the LSP
  ([`09-editor-integration.md`](09-editor-integration.md)). The round-trip is
  required to be fast — low milliseconds for a typical file — because it runs
  on every keystroke save.
- **On commit**, as a pre-commit hook.
- **In CI**, via `tel fmt --check`. A PR with unambiguously-wrong layout does
  not merge.

## Open questions

- TODO(open): **Source-level refactor tool.** `gofix` / IntelliJ structural
  replace are strong models for cross-cutting rewrites and deprecation
  migrations. The deprecation-migration story sits with the
  [linter](07-linter.md) today; decide whether a general-purpose `tel rewrite`
  should also ship. Lean: yes, as a separate subcommand, with declarative rules
  — never user code that runs at compile time.

## See also

- [Compiler](01-compiler.md) — the formatter reuses the compiler's parser and
  source-span machinery.
- [Linter](07-linter.md) — lint fix-its and deprecation rewrites live there.
- [Editor Integration](09-editor-integration.md) — format-on-save.
- [Build System](03-build-system.md) — `tel fmt --check` in CI.
- [Maxims](../02-philosophy/02-maxims.md) — *if it looks correct, it probably
  is correct*.
</content>
