# Layout Rules

Tel has **no semantic indentation**. Whitespace and newlines never change the
meaning of a program — they only separate tokens. Indentation is purely for
human readers and may be reformatted freely.

Block structure is given by **explicit `{}` delimiters** (see
[Blocks](03-blocks.md)), not by how code lines up. Two programs that differ only
in indentation are the same program — with one exception: the contents of a
**multi-line string** (`"""..."""`). There the line breaks, and any indentation
the margin rule keeps, are part of the string's value, so they are not free to
reformat (see [Literals](../03-lexical-structure/05-literals.md#multi-line-strings)).

## Newlines still matter

"No semantic indentation" does not mean newlines are ignored. A **newline
terminates a statement** — that is the one layout-related token event Tel keeps.
Statements written on the same line are divided by an explicit separator. The
lexical detail, including the two continuation cases (a leading `.` in a method
chain, and a trailing lambda), is in
[Whitespace and Newlines](../03-lexical-structure/08-whitespace-and-newlines.md).

The distinction is deliberate: line *breaks* survive copy-paste, so relying on
them is safe; leading *whitespace* does not, so Tel never relies on it.

The most-recent notes are explicit: *"statements must end with `;` and/or a
newline."* So the on-one-line separator is **`;`**, and a trailing `;` before
`}` is permitted (it ends a statement followed by no further statement) but
not required. An empty statement (a `;` with nothing before it) is not a
statement and is rejected.

{{#spec SEMICOLON_STATEMENT_SEPARATOR}}

`TODO(open): trailing `;` style.` Decide whether the formatter inserts a
trailing `;` before `}`, omits it, or leaves the choice to the author.

## Why no significant whitespace

Tel should be editable in poor editors — host configuration UIs, in-browser text
boxes — and pasted into and out of config files, where leading whitespace is
easily mangled, normalised, or stripped. Python/Haskell-style significant
indentation is fragile in those contexts: a reindent or a paste can silently
change behaviour. Explicit delimiters let code survive copy-paste and
reformatting unharmed.
