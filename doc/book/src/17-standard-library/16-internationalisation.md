# Internationalisation and Formatting

<!-- TODO: review -->

## What

`std` covers the workhorse internationalisation needs of an embedded
script: locale-aware **number**, **currency**, **date / time**, and
**unit** formatting, **collation** (locale-aware sorting), text
**translation**, and the small set of refined types that travel with
them (`Locale`, `Currency`, `TimeZone`, `Address`, `PersonName`).

### What is `std` and what is a library

A full localization stack is more than
the language should freeze. `std` ships the **core** — locale-aware number,
currency, date/time formatting, and collation, plus the `Locale` capability and
the `Show` / `Display` traits. The **heavier pieces** below — the
[translation](#translation) tables, the [template language](#template-language),
and the opinionated structured types
([`PersonName`/`PostalAddress`/`PhoneNumber`](#person-names-addresses-and-other-structured-fields))
— are **library** concerns, not committed `std` surface. The sections that
describe them are kept as design notes for *what such a library needs from the
language*; the worked example of building that library (and the proof the
language is expressive enough to build it safely) is
[A Localization Library](../19-use-cases/10-localization-library.md).

Like every other piece of `std`, the **locale is a capability**, not an
ambient global. A script that needs to render in the user's locale
receives a `Locale` value from the host; one that wasn't given a
locale renders in a documented invariant form, never silently in the
runtime's process locale.

## Why: i18n is *easy to get wrong*, hard to retrofit

Programmers believe many falsehoods — about
names, dates, time zones, addresses, character sets. The library is
shaped to make those falsehoods **type errors**, not runtime bugs:

- Display functions take a `Locale` argument (or the locale comes
  through the implicit context — see
  [`08-io-and-filesystem.md`](08-io-and-filesystem.md)); there is no
  ambient `toString` that picks one for you.
- `PersonName` is a structured type, not a `(first, last)` pair. The
  Falsehoods Programmers Believe About Names list is the design brief.
- A `DateTime` is timezone-qualified at the type level (see
  [`09-time.md`](09-time.md)); rendering picks the locale's
  conventions for separators, ordering, era names.
- `Currency` is its own type with its own arithmetic; mixing two
  currencies without an exchange step is a type error.

## The `Locale` capability

A `Locale` carries the locale's identifier (`en-US`, `nl-NL`,
`ar-EG-u-nu-arab`) and the data tables to render values according to
it. It is *not* a global setting; it is a value that flows through the
script.

```tel
fn render_total(total: EuroAmt, lc: Locale) -> Text {
    "${currency_format(total, lc)}"
}

# In nl-NL: "€ 1.234,56"
# In en-US: "€1,234.56"
```

The host supplies the locale, and *may* supply several — a user-facing
UI script may take both the *display locale* (what the user reads)
and an *invariant locale* (what the data is stored in). The library
exposes a small `Locale.invariant` constant for the latter when no
host grant is needed.

`TODO(open): which locale data the runtime ships vs which it expects
the host to supply. The CLDR is large; bundling it everywhere conflicts
with "cheap to embed". Lean: ship a minimal default tier (a handful of
major locales), and let the host extend with a *locale data*
capability for richer needs.`

## Number formatting

The library exposes:

- **`number_format(x, lc, opts)`** — locale-aware separators, decimal
  point, grouping size.
- **`percent_format(x, lc, opts)`** — separately because the symbol
  position and rounding conventions differ.
- **`scientific_format`**, **`engineering_format`**, **`compact_format`**
  — k/M/B in the locale's own abbreviations (`1.2K` vs `1,2 mil.`).

Every formatter takes an explicit *options* record (precision, sign
display, rounding mode, padding) so the call site says exactly what it
wants. There is no global "default format" to fight with.

## Currency

`Currency` is a refined type (`Currency.EUR`, `Currency.JPY`,
`Currency.BTC`); `Money` (or `MonetaryAmount`) pairs an exact decimal
value with a currency. Arithmetic obeys the obvious rules:

- `EUR + EUR -> EUR` is allowed.
- `EUR + JPY` is a compile error; the user must call `convert(rate)`
  first, taking an exchange-rate capability.
- Rounding to the currency's *minor unit* (JPY: 0, EUR: 2, BHD: 3)
  is built in; the user picks the rounding mode.

```tel
let price: Money = 19.99.eur()
let tax    = price * 0.21
let total  = price + tax            # exact: 24.1879 EUR
let charge = total.round_to_minor()  # 24.19 EUR
```

`TODO(open): exact spelling of money / amount / currency types and
their interaction with the refined-numeric story in
[`07-numerics-and-math.md`](07-numerics-and-math.md). Coordinate so we
don't ship `EuroAmt`, `Money`, *and* `Currency.EUR` as three
overlapping things.`

## Dates and times

Locale-sensitive date rendering takes the same shape: a `DateTime` plus
a `Locale` plus an *options* record (long / short / numeric format,
era display, time-zone display). The reverse direction — *parsing* in
a locale — uses the named-format-group machinery from
[`09-time.md`](09-time.md), with locale-specific groups available.

The library also exposes `time_since(t, lc)` and `time_until(t, lc)`
that produce a locale-aware *human* duration ("3 days ago",
"in 5 minutes"), the common "X minutes ago" UI need.

## Units of measurement

Even a `Real64` cannot be displayed without
context — joules vs kilowatt-hours vs MeV all carry the same number
with different meanings. `std` exposes:

- **`Quantity[U]`** — a value with a unit type, e.g.
  `Quantity[Joule]`, `Quantity[Meter]`. Arithmetic respects units
  (`Meter + Second` is a type error; `Meter / Second` produces
  `Quantity[Velocity]`).
- **Locale-aware unit display** — picking metric vs imperial by
  locale, formatting the unit symbol in the locale's script.
- **Conversion functions** — `to(other_unit)` returns a new
  `Quantity` in the requested unit; conversion factors are baked into
  the type system.

`TODO(open): units-of-measurement is a substantial sublibrary. Decide
whether the *machinery* (dimensional analysis at the type level) lives
in the language or in `std`. The lean is `std`, but the type
machinery is heavier than usual library code.`

## Translation

*Library concern* (see the [framing](#what-is-std-and-what-is-a-library) above
and the [worked example](../19-use-cases/10-localization-library.md#2-translation-keys-checked-at-compile-time)).
These notes describe what such a library needs.

The library exposes a translation surface:

- **`tr(key, lc, args = {})`** — look up a key in the active
  translation table for the locale, substituting named arguments.
- **Pluralisation** — keys can declare plural forms (`zero`, `one`,
  `few`, `many`, `other`) matching the locale's CLDR plural rules;
  the matching form is chosen by the count argument.
- **Compile-time validation** — a `tr("greeting", lc, {name: ...})`
  call is checked against the declared keys and arguments of the
  translation file, so a renamed or missing key is a compile error,
  not a runtime mojibake.

Translation files are themselves a *separate format* (probably the
Tel-data sublanguage hinted at in
[`13-data-formats.md`](13-data-formats.md)); the build step generates
type-safe accessors from them.

`TODO(open): translation key validation requires the build to see the
translation files; this is a *codegen* workflow, not a runtime one.
Coordinate with the data-formats and build chapters.`

## Template language

*Library concern* (see the [framing](#what-is-std-and-what-is-a-library) above
and the [worked example](../19-use-cases/10-localization-library.md#4-escape-or-localise-output-slots)).

A localization library should offer a small **template language** for generating
text output (HTML, emails, reports), derived from established open-source engines
rather than invented from scratch. The desirable properties, none yet pinned
down:

- **Optionally disallow raw `Text`.** A template can be configured to refuse a
  plain `Text` value in an output slot, forcing the author to go through a typed
  step that (1) *escapes* for the target — HTML-escaping by default for an HTML
  template — and (2) *localizes* any human-facing string. This turns "forgot to
  escape" and "shipped an untranslated string" into compile-time errors instead
  of production bugs.
- **Compatible with ordinary string templates.** The slot/format syntax should
  match the language's
  [string interpolation and format specifiers](../07-expressions/05-string-operations.md)
  so there is one mental model, not two.
- **Pluralisation** — reuse the CLDR plural-form machinery from
  [Translation](#translation) above, so a template pluralises through the same
  locale rules as `tr`.

`TODO(open): which open-source engine to derive from, the exact opt-in for
"no raw Text", and how a template file is checked at build time (it is a
codegen workflow like translation files).`

## Person names, addresses, and other "structured" fields

*Library concern* (see the [framing](#what-is-std-and-what-is-a-library) above
and the [worked example](../19-use-cases/10-localization-library.md#why-this-is-a-library-not-std)).
`std`'s stability rule is exactly why these cannot be `std` types: a name shape
frozen forever will be wrong for someone, and a library can rev where `std`
cannot.

The library ships refined types for the fields most likely to be
abused by `(first, last)`-style hand-rolling:

- **`PersonName`** — a *named* set of optional parts (given, family,
  honorific, middle, generational suffix, locale-specific particles)
  plus a `display(lc)` that knows how to render them per locale. There
  is no `PersonName.full_name` field — display is computed.
- **`PostalAddress`** — country, region, locality, street, postcode,
  with country-specific shape. Validation is locale-aware.
- **`PhoneNumber`** — E.164 form with a country-code wrapper.

These are *opinionated* refined types in the spirit of `EuroAmt`. A
script that wants free-form text can still use `Text`; the refined
types are there for the cases where structure pays off.

`TODO(open): person-name handling is famously hard (the Falsehoods
list). Designing a type the library is willing to *stand behind for
decades* is a real commitment — the stability rule means we cannot
silently fix bad assumptions later. Decide whether `PersonName` is in
`std` or a separate crate.`

## Display traits

Two complementary traits underpin the chapter:

- **`Show`** — the locale-free debug/log form. Always available;
  rendered the same in every host, every locale, every time. Used by
  default logging, error messages, and stack traces.
- **`Display`** — locale-aware presentation. Takes a `Locale` and an
  options record. Used by user-facing output.

The split is deliberate: a `Show` rendering is for *programmers*
(stable, comparable across runs, no locale), a `Display` rendering is
for *users* (locale-aware, allowed to drift across runtime updates).
The compiler rejects calling `Display` where it has no locale and
rejects logging a `Display`-only value.

`TODO(open): exact trait names — `Show` overlaps with Haskell's
existing usage; `Debug` / `Display` is Rust-flavoured but conflates
"debug" with "locale-free", which should be kept separate.
Pick names that read right in Tel.`

## Falsehoods as design briefs

The chapter is shaped by the *Falsehoods Programmers Believe About …*
articles (names, addresses, time, character sets, time zones,
phone numbers). Each is treated as a design brief, not a curiosity:
the API has to refuse the falsehood, not just document against it.

`TODO(open): a list of links to the source articles belongs in the
`impl-notes/` scratchpad, not this user-facing topic. Keep this topic
to the design rules; the literature trail lives in implementation
notes.`

## See also

- [Strings and Text](06-strings-and-text.md) — codepoints vs graphemes
- [Numerics and Math](07-numerics-and-math.md) — decimals and
  refined numerics behind `Money`
- [Time](09-time.md) — datetime types
- [Data Formats and Serialization](13-data-formats.md) — translation
  file format
- [Antifeatures](../02-philosophy/04-antifeatures.md) — no ambient
  locale
