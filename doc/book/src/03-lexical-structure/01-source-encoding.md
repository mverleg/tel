# Source Encoding

Tel source is **UTF-8 text**. This is the one encoding every modern host,
editor, and version-control system handles without configuration, which matters
because Tel scripts are routinely pasted into host configuration UIs, in-browser
text boxes, and config files.

## What is settled

- Source files are UTF-8.
- Line endings are insignificant: `\n` and `\r\n` are both accepted, and a
  trailing newline is not required. See
  [Whitespace and Newlines](08-whitespace-and-newlines.md).
- Indentation carries no meaning — two files that differ only in indentation are
  the same program. See [Layout Rules](../04-syntax/05-layout-rules.md).

{{#spec LINE_ENDINGS_INSIGNIFICANT}}

## Open questions

`TODO(open): the surface alphabet.` The lean is toward an English-only,
mostly-ASCII surface for identifiers and keywords on readability grounds, while
string literals and comments are obviously free to contain any UTF-8. See
[Identifiers](03-identifiers.md), where this decision is tracked.

`TODO(open): shebang lines.` A file may begin with a `#!`-style line so a host
or OS tool can route it. Tel must either treat a leading `#!` line as ignorable
or fold it into the comment rule. This interacts with the choice of `#` as the
comment marker — see [Comments](07-comments.md). Note that Tel is a *guest*
language: a shebang only matters if a host runs Tel files directly, which is an
edge case, not the main use. Decide whether to support it at all.

`TODO(open): a source-version declaration.` Should source carry a version, so
that syntax or semantics can change for newer versions without breaking old
code? This conflicts directly with the
[stability priority](../02-philosophy/01-priorities.md) and the antifeature
"no runtime version churn or language editions" — Tel is frozen at 1.0, and
the *next* breaking change would be a separate language (Tel2), not a new
edition of Tel. Lean: no version pragma in source. A future Tel2 file would
look different enough (different file extension, or a host setting) without
asking every Tel1 script to declare itself. Re-justify against embedding
before adding any version marker.

TODO: review
