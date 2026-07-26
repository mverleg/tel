# A Localization Library

Rendering text for humans — translated, pluralised, escaped for its target,
formatted to a locale's conventions — is a recurring need, and one that is
*easy to get subtly wrong*. It is also **large**: a full localization stack is
more than `std` should freeze (see
[`../17-standard-library/16-internationalisation.md`](../17-standard-library/16-internationalisation.md)).
The split Tel settles on is:

- **`std` ships the core** — locale-aware number, currency, date/time
  formatting, and collation, all gated on a `Locale` capability.
- **Everything heavier** — translation tables, a template engine, structured
  `PersonName` / `PostalAddress` / `PhoneNumber` types — is a **library**.

This use case is not a language feature of its own. It is a worked example of
*applying* features covered earlier — capabilities, refined types, untagged
unions, traits, and schema-first codegen — to build that library, and a check
that the language is **expressive enough** to build it *well*: the safety
properties (a missing translation key is a compile error, an unescaped value
cannot reach an HTML slot) have to fall out of the type system, not out of
runtime discipline. Where the language is not yet expressive enough, that is
flagged as a `TODO(open):` — those are requirements on the language, not on the
library.

## What — four patterns

### 1. The locale is a capability, never ambient

Nothing here reaches for a process-global locale. A script that renders for a
user receives a `Locale` value from the host, the same way it receives a
`Clock` or a `File` ([`../02-philosophy/03-features.md`](../02-philosophy/03-features.md)).
A script handed no locale renders in a documented invariant form — never
silently in the runtime's process locale.

```tel
fn greeting(lc: Locale, who: PersonName) -> Text {
    tr.hello(lc, name = who.display(lc))
}
```

The library's whole surface threads `Locale` explicitly, so "which locale did
this render in?" is always answerable by reading the call, and a test pins it
by passing a fixed `Locale`.

### 2. Translation keys checked at compile time

The hazard a translation layer must kill is the *stringly-typed key*: `tr("helo")`
that typos the key and ships a blank string, or `tr("greeting", name=…)` against
a key whose template never had a `name` slot. Tel has [no reflection and no
`eval`](../02-philosophy/04-antifeatures.md), so the library cannot look a
runtime string up in a table and hope. Instead it leans on Tel's **schema-first
codegen** stance ([`../17-standard-library/13-data-formats.md`](../17-standard-library/13-data-formats.md)):
a build step reads the translation files and emits a typed accessor per key,
using the Tel-as-data AST surface
([`../17-standard-library/18-tel-as-data.md`](../17-standard-library/18-tel-as-data.md)).

```text
# translations/en.toml  (the one source of truth)
hello       = "Hello, {name}!"
unread      = { one = "{n} unread message", other = "{n} unread messages" }
```

```tel
# Generated — committed alongside the script, never hand-edited.
# One typed function per key; the arguments match the template's slots.
mod tr {
    fn hello(lc: Locale, name: Text) -> Text { ... }
    fn unread(lc: Locale, n: Int64) -> Text { ... }   # plural-aware, see below
}
```

A renamed or missing key, or a wrong/absent argument, is now an ordinary
*compile error* in the generated module — not a runtime "mojibake" surfacing in
production. This is the same move use case 9 makes for query results: derive the
typed surface from the external source of truth rather than declaring it twice.

`TODO(open): the generator must emit one accessor per key with argument names
and types drawn from the template's slots. Confirm the Tel-as-data surface can
express "emit a function whose parameter list is computed from data" — this is
the load-bearing expressiveness requirement. Coordinate with
[`../17-standard-library/18-tel-as-data.md`](../17-standard-library/18-tel-as-data.md).`

### 3. Pluralisation as a union, not a magic string

CLDR plural categories — `zero`, `one`, `few`, `many`, `other` — are a closed
set, so they are an [untagged union / enum](../10-data-modelling/02-union-types.md),
and the form is chosen by `match` on the count. No string compares the category
name; the compiler checks the arms.

```tel
# Inside the generated `unread`, sketch:
fn unread(lc: Locale, n: Int64) -> Text {
    match lc.plural_category(n) {        # std supplies the locale's rule
        PluralCat.One   => "${n} unread message"
        _               => "${n} unread messages"
    }
}
```

The locale's *rule* (which count maps to which category) is data `std`'s `Locale`
carries; the *selection* is an ordinary match the library writes. A locale with
`few`/`many` forms (Polish, Arabic) gets those arms generated from its file.

### 4. Escape-or-localise output slots

A template engine's job is to make "forgot to escape" and "shipped an
untranslated string" into *type* errors. The library models an output slot as a
refined type ([`../05-types/12-refined-types.md`](../05-types/12-refined-types.md))
parameterised by its target, and *refuses a raw `Text`* in a slot:

```tel
# A value cleared for an HTML slot — produced only by escaping or by
# a localised render, never by an implicit Text -> slot conversion.
type Html = newtype Text

fn esc(t: Text) -> Html { Html(html_escape(t)) }

# The template's slots demand `Html`, so a bare `Text` is a compile error:
let page = html`<p>${esc(user_input)} — ${tr.hello(lc, name)}</p>`
#                  ^ ok: escaped       ^ ok: localised render returns Html
#               ${user_input}  would not type-check
```

There is no implicit conversion in Tel ([`../02-philosophy/04-antifeatures.md`](../02-philosophy/04-antifeatures.md)),
so the slot type does the work: the only ways to fill an `Html` slot are to
escape a `Text` or to use a value that is already locale-rendered. The same
slot syntax reuses the language's
[string-interpolation and format specifiers](../07-expressions/05-string-operations.md),
so there is one mental model, not two.

`TODO(open): the "template demands Html" check wants the interpolation slots of
a template literal to carry a *type* the author must satisfy. Confirm whether
this is expressible with the planned string-template surface or whether
templates are a build-time-checked external file (the translation-file path
above). Lean: external template files, checked by the same codegen step as
translation keys.`

## Why this is a library, not `std`

Three of Tel's commitments push the heavy parts out of `std` and into a crate:

- **Stability.** A type `std` ships is frozen for the life of the language.
  `PersonName` is the cautionary case: the *Falsehoods Programmers Believe About
  Names* list means any structured name type will be *wrong* for someone, and
  the [stability priority](../02-philosophy/01-priorities.md) forbids silently
  fixing it later. A library can rev; `std` cannot. So `PersonName` lives in the
  library, as an ordinary record with an explicit `display(lc)`:

  ```tel
  struct PersonName {
      given:       Option[Text],
      family:      Option[Text],
      honorific:   Option[Text],
      particles:   List[Text],      # locale-specific (van, de, bint, …)
  }
  # No `full_name` field — display is computed, per locale.
  fn display(self, lc: Locale) -> Text { ... }
  ```

- **One good way over many clever ones.** A template engine and a translation
  framework shape how *every* string in a program is produced. That is bigger
  than a library should be when bundled into the language, and different hosts
  (a game's dialogue system, a web backend's HTML, a CLI's messages) want
  different engines. `std` blessing one would force it on all.

- **The host owns deployment.** Translation files and templates are *codegen*
  inputs read at build time; the build is the host's
  ([`../17-standard-library/13-data-formats.md`](../17-standard-library/13-data-formats.md)).
  `std` ships the data model and the AST surface a generator emits into; the
  generator itself is tooling, not language.

The thesis of this use case is the converse of that boundary: keeping these out
of `std` is only acceptable *because the language is expressive enough to build
them as a library with the same safety* — compile-checked keys, escape-safe
slots, locale-as-capability. The `TODO(open):`s above are exactly the places
where that expressiveness still has to be confirmed.

## The one piece that stays in `std`: the `Show` / `Display` split

Two traits underpin the boundary, and they belong in `std` because logging and
debugging are universal:

- **`Show`** — the locale-free debug form, rendered identically in every host
  and run. Used by logging, errors, traces
  ([`../17-standard-library/14-observability-and-logging.md`](../17-standard-library/14-observability-and-logging.md)).
- **`Display`** — locale-aware presentation. Takes a `Locale`; used for
  user-facing output.

The library's rendered values implement `Display`; the compiler rejects logging
a `Display`-only value or calling `Display` with no locale in scope. That guard
is a language/`std` concern (it protects everyone), while *what* a localised
render produces is the library's.

`TODO(open): final trait names — `Show` clashes with Haskell's, `Debug`/`Display`
is Rust-flavoured but conflates "debug" with "locale-free". Tracked in
[`../17-standard-library/16-internationalisation.md`](../17-standard-library/16-internationalisation.md#display-traits).`

## How it looks — putting the pieces together

```tel
# Host hands in the display locale; nothing is ambient.
fn render_inbox(lc: Locale, user: PersonName, unread: Int64) -> Html {
    html`
      <h1>${esc(tr.hello(lc, name = user.display(lc)))}</h1>
      <p>${tr.unread(lc, n = unread)}</p>
      <footer>${esc(number_format(unread, lc))}</footer>
    `
}
```

Every human-facing string flows through a locale; every slot is either escaped
or already a localised render; every translation key is a generated function the
compiler checks. The library is a few hundred lines on top of features that
already exist — which is the point: Tel does not need an i18n framework baked in,
only the expressiveness to host one.

## See also

- [Internationalisation and Formatting](../17-standard-library/16-internationalisation.md)
  — the `std` core (formatting, collation) and the design notes for the
  library parts this use case builds.
- [Refined and Newtype Types](../05-types/12-refined-types.md) — the `Html`
  slot type and `Currency`-style wrappers.
- [Union Types](../10-data-modelling/02-union-types.md) — plural categories as a
  closed enum.
- [Data Formats and Serialization](../17-standard-library/13-data-formats.md)
  and [Tel-as-data](../17-standard-library/18-tel-as-data.md) — the schema-first
  codegen that makes translation keys compile-checked.
- [Entity Identity, Queries, and Projections](09-entity-identity-and-projections.md)
  — the sibling "derive the typed surface from the source of truth" use case.
- [Antifeatures](../02-philosophy/04-antifeatures.md) — no ambient locale, no
  implicit conversion, no reflection.

TODO: review
