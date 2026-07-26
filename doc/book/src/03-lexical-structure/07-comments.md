# Comments

A comment is source text the compiler discards before parsing. Tel has exactly
two comment forms — a normal line comment and a documentation comment — and **no
block comments**.

## Line comments with `#`

Tel uses **`#` to start a normal line comment**, running to the end of the line.
This matches the convention scripting users already know from Python, shell,
Ruby, and many config formats — *familiarity over a novel surface*.

```tel
# A scoring rule the host hands to Tel.
fn score(order: Order) -> Score {
    let age = order.age_days   # inline comment, runs to end of line
    Score.from(order, age)
}
```

The example blocks throughout this documentation already use `#` this way.

## Documentation comments with `##`

A comment that starts with **`##`** is a **teldoc (documentation) comment**: the
same line-comment lexing, but the text is retained as documentation metadata
feeding tooling and API summaries rather than discarded. The design notes
describe documentation as *"documentation from comments"*; `##` is the lexical
marker that distinguishes it from an ordinary `#` comment.

```tel
## Scores an order against the active ruleset.
## Returns a `Score`; never panics.
fn score(order: Order) -> Score {
    # ordinary comment — discarded, not part of the docs
    Score.from(order, order.age_days)
}
```

Because `##` is just a longer comment opener, the rule stays simple: `#` runs to
end of line, and a leading `##` on that comment promotes it to documentation.
There is no separate doc-comment lexer state. See [Tooling](../18-tooling/) for
how teldoc comments are surfaced.

A `##` comment *describes* a declaration; it does not make a checkable claim.
A separate construct — the **review invariant** — states a prose obligation
that review tooling re-checks whenever the annotated code (or its callers)
changes (*"every path does an auth check first"*). That is a tooling concern,
not a third comment form; see
[`../18-tooling/07-linter.md`](../18-tooling/07-linter.md#review-invariants--unprovable-re-checked-on-change).

## No block comments

Tel deliberately has **no `/* ... */` block or nestable comment form**. The
reasons:

- *One obvious way.* A single line-comment form (plus its `##` doc variant) is
  the whole story; there is no second mechanism to learn or to choose between.
- *Block comments invite trouble.* They force a decision about nesting and a
  lexer state with a non-local terminator — an unterminated `/*` can silently
  swallow the rest of a file, and "do block comments nest?" has no answer that
  satisfies everyone.
- *Embedding context.* Tel source is frequently pasted into host config UIs,
  in-browser boxes, and string literals carrying other languages; a stray `*/`
  inside such a snippet is a real hazard. Line comments have no multi-line
  failure mode.

To comment out a span, comment each line — editors toggle `#` prefixes over a
selection trivially, which covers the one job block comments are reached for.

## Shebang handling

Choosing `#` as the comment marker has a convenient side effect: a file may
begin with a `#!`-style shebang line and the compiler can simply treat it as an
ordinary `#` comment, with no special case. The design notes weigh exactly this
— *"either use `#` for comment, or have special handling to ignore shebang"* —
and picking `#` collapses the two into one rule.

That said, Tel is a **guest** language: a host embeds the runtime and feeds it
source, so a shebang is only meaningful in the rare case where an OS tool runs a
`.tel` file directly. The comment rule makes a leading `#!` *harmless*; whether
Tel actively endorses shebang-run scripts is a separate, lower-priority
question.

`TODO(open): does Tel endorse shebang-run scripts at all?` It conflicts mildly
with the "host owns the process lifecycle" non-goal. Lean: `#!` parses as a
plain comment so nothing breaks, but Tel does not advertise a shebang runner.
Confirm with [Source Encoding](01-source-encoding.md).

TODO: review
