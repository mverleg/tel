# Tel-as-Data

<!-- TODO: review -->

## What

`std` exposes a small set of types that represent **Tel source code as
data** — expressions, statements, type declarations, function
signatures. There are exactly **two** uses, and neither is reflection:

- **Code generation (writer).** A script consumes an *external* schema or
  data model — protobuf, Avro, a translation file — and *writes a new Tel
  module* to disk. Codegen takes the external description as input and emits
  Tel; it never takes Tel as input.
- **Analysis (reader).** A tool reads *structured* Tel and inspects it —
  the home for the declarative [custom lint rules](../18-tooling/07-linter.md#custom-lints)
  a project or crate can declare. This consumes Tel-as-data; it never runs
  it.

The chapter is short because the surface is deliberately narrow.

## Why: codegen earns its place; reflection does not

The distinction is load-bearing:

- **Reflection** is *looking at running code* — the program inspects
  its own functions, types, fields at runtime. Tel deliberately does
  not have it (see [`../02-philosophy/04-antifeatures.md`](../02-philosophy/04-antifeatures.md));
  it defeats capability-based safety, obstructs AOT compilation, and
  invites stringly-typed dispatch.
- **Code generation** is *writing new code* — the program produces
  Tel source files that the compiler then processes like any other
  source. There is no runtime introspection involved; the generated
  code is just text on disk, read by the next compile.

The serialisation story in [`13-data-formats.md`](13-data-formats.md)
lands on schema-first, code-generated data classes; the schema-evolution
machinery there is one of the headline users. The translation files
in [`16-internationalisation.md`](16-internationalisation.md) are
another. A typed Tel-data DSL (also mentioned in
[`13-data-formats.md`](13-data-formats.md)) is a third. The generator
in every case wants to *build a Tel module*, and the cleanest way to
do that is a typed AST, not raw string concatenation.

## The surface

`std.tel_ast` (working name) exposes:

- **Expression nodes** — literals, identifiers, calls, lambdas, `let`,
  `match`, blocks.
- **Type nodes** — concrete types, unions, generics, refined types.
- **Declaration nodes** — `fn`, `type`, `import`, visibility modifiers.
- **A `Module` node** — a complete file: imports plus top-level
  declarations.
- **A printer** — `Module.to_source() -> Text` that emits valid Tel
  text, formatted the same way the standard formatter would format
  hand-written source.

```tel
# Sketch — names and shapes loose.
let id_type = Type.refined(Type.Int64(), "Id[Person]")
let body    = Expr.call("validate", [Expr.ident("input")])
let f       = Decl.fn("check_id",
                      params = [Param("input", id_type)],
                      ret    = Type.result(Type.unit(), Type.error("Bad")),
                      body   = body)
let module  = Module(imports = [], decls = [f])

write_text("generated/check_id.tel", module.to_source())
```

The point is that *the AST type system catches malformed code*.
Forgetting a parameter, passing the wrong arity, mixing an expression
where a type is expected — all compile errors at the call site of the
generator, not "invalid syntax" errors when the next `tel build` runs.

### A builder DSL over the AST

The constructor-call form above is explicit but verbose. A more readable surface
builds the module as a nested **receiver-closure** builder — the same mechanism
as the `html { … }` markup DSL
([lambda receivers](../09-functions/06-closures-and-lambdas.md#lambda-receivers))
— so nodes read structurally without naming the builder on each line:

```tel
# Sketch — a receiver-block builder over the same AST.
let module = tel_module {
    fn_("check_id") {                      # `this` is a function builder
        param("input", id_type)
        ret(Type.result(Type.unit(), Type.error("Bad")))
        body { call("validate", [ident("input")]) }
    }
}
```

This stays a **typed** Tel-data DSL: the receiver's methods are the only names in
scope, so a malformed node is a compile error and (unlike Groovy/Ruby DSLs)
resolution is fully static and IDE-navigable — the property TIP-0010 exists to
protect. The receiver design is
[TIP-0010](../tips/0010-lambda-receivers-and-builder-dsls.md).

`TODO(open):` whether `std.tel_ast` ships this builder surface in 1.0 or only the
constructor form above — decide alongside the other receiver-DSL customers (the
typed [data-format DSL](13-data-formats.md) and
[`log.sub(...)`](14-observability-and-logging.md)).

## Generators run on the build, not at runtime

`std.tel_ast` is normally consumed by **build-time** code that produces
Tel sources written next to the hand-written ones. The host's build
chain (see [`../18-tooling/03-build-system.md`](../18-tooling/03-build-system.md))
picks the generated sources up like any other input.

`std.tel_ast` is *also* available at runtime — the types are nothing
special — but a script that generates Tel at runtime can do nothing
useful with the result without a host capability to *write the file*
and *re-invoke the compiler*. That capability is not part of `std`;
the host exposes it (or does not) the same way it exposes a
subprocess.

`TODO(open): "available at runtime" is a meaningful choice — runtime
codegen with hot reload is a real pattern (configuration languages,
DSL-driven UI). Decide whether `std` actively supports it or whether
runtime codegen is left as a host concern. Lean: support the AST and
printer; do not ship a runtime compiler invocation.`

## Stability of the AST surface

The AST is part of Tel's frozen-language commitment — a script that
builds a Tel module today should keep producing the same source ten
years from now. New language features add new AST nodes; the existing
nodes stay valid. A consumer that pattern-matches over an AST should
be able to opt in to *non-exhaustive* matching so it does not break
when new nodes appear (see the unions story in
[`../02-philosophy/03-features.md`](../02-philosophy/03-features.md)).

`TODO(open): non-exhaustive matching on the AST nodes is necessary for
forward compatibility; pin it down once the union story is settled.`

## Headline codegen consumers

There are several "generators" that all reduce to the
same machinery — building a Tel module from a description and writing
it to disk:

- **Schema-driven data classes** — see
  [`13-data-formats.md`](13-data-formats.md). The schema is the input;
  the generated module exposes the data classes plus serialisation.
- **Schema-evolution adapters** — also from
  [`13-data-formats.md`](13-data-formats.md). A version-history file
  generates the migration code that reads an older payload into a
  newer shape.
- **Translation accessors** — see
  [`16-internationalisation.md`](16-internationalisation.md). The
  translation files generate typed `tr(...)` accessors so missing keys
  are compile errors.
- **Mappers between data shapes** — Java's MapStruct is the precedent.
  Given two data shapes and a declarative mapping (`source.full_name
  -> dest.name`, `source.created_at -> dest.timestamp`), the
  generator writes the boilerplate. The library exposes the
  *machinery*, not the mappings themselves.
- **Derived / cached properties** — a way to
  declare that `full_name` derives from `first_name` and `last_name`,
  with chosen caching semantics (always recompute, recompute on
  change, invalidate on change, recompute on first access). This is a
  codegen case: a description of the derivation generates the
  accessors and any invalidation hooks. `TODO(open): derived-cache
  generation overlaps with `derive`-style attributes; coordinate.`

The generators themselves live in tooling and crates — `std` only
ships the AST + printer they all use.

## Reading structured Tel: custom lint rules

The same AST types serve the *reader* direction. A
[custom lint rule](../18-tooling/07-linter.md#custom-lints) is a declarative
matcher over structured Tel — patterns over calls, types, imports, and module
shape — so the build tool can analyse a project's own code without running it.
This is the analysis half of Tel-as-data: the lint consumes the AST/IR the
compiler already produced, the mirror image of codegen building one to print.

It is still **not reflection**: the analysis runs over *source/IR at build
time*, never over a running program inspecting itself. And it is **not a macro
system**: a lint reads and reports, it does not rewrite the surrounding code.

## What this is not

- Not reflection. There is no `type_of(value)` returning an AST node — reading
  structured Tel happens at build time over source/IR, not at runtime over a
  live value.
- Not a macro system. A script does not "expand" a compile-time AST
  into the surrounding code; it writes text to a file. Heavy
  metaprogramming is an antifeature
  ([`../02-philosophy/04-antifeatures.md`](../02-philosophy/04-antifeatures.md));
  the narrow exception is `derive`-style attributes.
- Not `eval`. There is no "compile and run this AST" function in
  `std`. The build pipeline picks up generated files; the runtime
  does not pick up generated ASTs.

`TODO(open): the boundary between `tel_ast` (codegen) and `derive` (a
compile-time attribute that *also* generates code) is the same kind
of facility used differently. Confirm they share machinery, and that
neither overlaps into reflection territory.`

## See also

- [Data Formats and Serialization](13-data-formats.md) — the headline
  consumer for codegen
- [Internationalisation and Formatting](16-internationalisation.md) —
  translation-file codegen
- [Features Tel Embraces](../02-philosophy/03-features.md) — the
  `derive` narrow exception to "no metaprogramming"
- [Antifeatures](../02-philosophy/04-antifeatures.md) — why there is no
  reflection
- [Build System](../18-tooling/03-build-system.md) — where generated
  sources are picked up
