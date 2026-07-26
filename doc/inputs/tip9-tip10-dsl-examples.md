# DSL examples using inline lambdas (TIP-0009) + lambda receivers (TIP-0010)

Scratch input: small, self-contained DSLs written in the proposed syntax from
**TIP-0009 (inline lambdas / non-local control flow)** and **TIP-0010 (lambda
receivers and builder DSLs)**, plus the rest of Tel as used in
`examples/json-schema-validator.tel`. Meant as raw material for worked-example
chapters and as a sanity check on the two TIPs.

These examples reflect a **design decision taken while drafting them** (see next
section): receivers do **not** use a leading `.` — a receiver closure is just a
method with a rebound `this`, and bare names resolve through it.

Everything here is loose pseudocode — the goal is the *call-site* shape, not
pinned syntax. `TODO(open):` markers flag spots still open.

## The model: a receiver closure is a method with a rebound `this`

The earlier draft of TIP-0010 marked every receiver use with a leading `.`
(`.head`, `.title`). That was reversed, for two reasons:

1. **The leading `.` is already spoken for — chaining.** Tel uses a *line-leading*
   `.` to continue a method chain
   ([lexical-structure §08](../book/src/03-lexical-structure/08-whitespace-and-newlines.md)),
   so `html { .head() / .body() }` would parse as the single chain
   `body(head(it))`, not two receiver statements. Chaining is the far more
   established meaning of `.`; the receiver must yield it. (Visual Basic gets the
   dotted receiver to work only *because* it does **not** use a line-leading `.`
   for chaining — Tel does, so it can't.)
2. **A receiver closure and a method are the same thing** — a function with a
   `this`/`self` context — and should share *one* rule for how that context is
   reached. Methods already resolve bare names through `self`
   ([method syntax](../book/src/09-functions/08-method-syntax.md): inside an
   `impl`, `apply(rate, total)` is `self.apply(self.total)`). A receiver block is
   just that, with `this` rebound to the builder. So bare-name-through-context is
   **not new implicitness** — it is the rule methods already have, applied
   uniformly.

The resulting rules (all confirmed):

- **Bare name** → a lexical local / free function first; otherwise a member of the
  current context `this` — exactly as in a method body.
- **`self` / `this`** → the current context, named explicitly (to disambiguate or
  to pass it along).
- **`expr.member` / line-leading `.`** → ordinary method access and chaining.
  **Never** overloaded for the receiver.
- **`outer return` / `outer break` / `outer continue`** (TIP-0009) → *up* into the
  declaring function.
- **Single context, innermost only.** At most one `this` is active; there is no
  outer-receiver scope chain (so Kotlin's `@DslMarker` leak cannot occur). A
  nested block's `this` shadows an enclosing method's `self`; reach the outer one
  by naming it.
- **Explicitness is set at the declaration.** A bare `{ … }` block omits its
  receiver → implicit `this`, bare-name resolution on. A `|h| { … }` block names
  it → you must write `h.foo`, and bare names do **not** reach it. (`TODO(open):`
  whether the implicit form is plain omission or an explicit placeholder token
  such as `_` / `self` at the declaration — omission is lightest, a placeholder is
  more visible.)
- **No-shadowing** makes a bare name that collides with a context member a compile
  error, not a silent capture — so "what does this name mean?" always has an
  answer.

How a receiver block parameter is **written** still follows TIP-0010's
method-as-value shape — `Recv.fn(args) : Ret` is "a block whose `this` is a
`Recv`" — and is **invoked** like a method on the chosen receiver: `d.content()`
runs `content` with `d` as `this`.

So the three directions a name can go are now: **bare** (here / into `this`),
**`self`** (the context, explicitly), and **`outer`** (up into the declaring
function) — none of which touch the `.` that does chaining.

---

## Example 1 — an HTML builder (TIP-0010 receivers)

The canonical builder DSL. Each block's `this` is a `Doc` buffer, so nested tags
call `title` / `h1` / `li` as bare names — like a method body calling its own
helpers. Nothing to bind, so every block is a bare `{ … }`.

```tel
struct Doc { out: TextBuilder }

# `content` is a block whose `this` is a `Doc` (the `Doc.fn() : Unit` type).
# `d.content()` runs it with `d` as `this`, so bare `title`/`h1` inside resolve to
# `d`, exactly as a bare call in a method body resolves to `self`.
fn elem(d: Doc, name: Text, content: Doc.fn() : Unit) {
    d.out.push("<${name}>")
    d.content()
    d.out.push("</${name}>")
}

fn html(content: Doc.fn() : Unit) -> Text {
    let uniq d = Doc { out = TextBuilder() }
    d.elem("html", content)
    d.out.finish()
}

# Container tags forward their block to `elem`. A receiver block is an ordinary
# value, so passing `content` straight through is fine (TIP-0010: a receiver
# closure may escape — it says nothing about control flow).
fn head(d: Doc, content: Doc.fn() : Unit) { d.elem("head", content) }
fn body(d: Doc, content: Doc.fn() : Unit) { d.elem("body", content) }
fn ul(d: Doc,   content: Doc.fn() : Unit) { d.elem("ul", content) }

# Leaf tags. Inside `{ text(s) }` the call `text(s)` is `this.text(s)` (this = the
# Doc), while `s` is the lexical parameter — no clash: lexical names win, and
# no-shadowing would make any genuine collision a compile error.
fn title(d: Doc, s: Text) { d.elem("title") { text(s) } }
fn h1(d: Doc,    s: Text) { d.elem("h1")    { text(s) } }
fn p(d: Doc,     s: Text) { d.elem("p")     { text(s) } }
fn li(d: Doc,    s: Text) { d.elem("li")    { text(s) } }

fn text(d: Doc, s: Text) { d.out.push(escape(s)) }

# Assumed stdlib: Text.replace(from, to) -> Text.
fn escape(s: Text) -> Text {
    s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")
}
```

Usage reads like the markup it produces — no `h.` / `hd.` / `b.` noise, and no
leading `.` either. Each line is a plain newline-terminated statement (it does
*not* start with `.`, so the chain-continuation rule never fires):

```tel
let page = html {
    head {
        title("Sales report")
    }
    body {
        h1("Sales report")
        p("Figures for Q3 follow.")
        ul {
            li("Region A: 1.2M")
            li("Region B: 0.9M")
        }
    }
}
# <html><head><title>Sales report</title></head><body><h1>Sales report</h1>
#   <p>Figures for Q3 follow.</p><ul><li>Region A: 1.2M</li>
#   <li>Region B: 0.9M</li></body></html>
```

Each block has exactly one `this` — its own `Doc` — and there is no
outer-receiver scope chain, so `title` inside `head { … }` is unambiguous with no
label and no `@DslMarker`-style patch.

---

## Example 2 — queries: receivers + dataframe transforms, then a TIP-0009 iterator

This is the example combining **two** features. A naive query DSL — a receiver
block that pushes raw SQL strings into a builder and returns `Text` — is
stringly-typed: column names and predicates are unchecked, injection-prone, and
the result carries no schema. The typed answer is to make the query the
**query carrier** of the [TIP-0008 dataframe calculus](../book/src/tips/0008-named-axis-dataframes.md)
(`project` → `SELECT`, `filter` → `WHERE`, `extend` → `SELECT expr AS`, …) and
to use **TIP-0010 receiver blocks for the per-row expressions**, so columns are
bare names resolved against the row.

### A typed query — receiver blocks over the dataframe calculus

```tel
# A handle to a table/query, carrying its row schema as a record type.
# policies : Query[{ id: Int64, holder: Text, status: Text,
#                    premium: EurAmt, cost: EurAmt, created_at: Date }]

let report =
    policies
        .filter \ status == "active" and premium > eur(1000)    # row receiver
        .extend(margin = premium - cost)                        # RHS is row-scoped
        .sort_by \ created_at 
        .take(20)
        .project[id, holder, margin]
# : Query[{ id: Int64, holder: Text, margin: EurAmt }]
```

Two features doing two jobs, cleanly separated:

- **The chain is the TIP-0008 query carrier** (`filter` / `extend` / `sort_by` /
  `take` / `project`). Each is a transform whose *result type* the compiler
  computes from the row schema, so the pipeline ends at a statically-known
  `Query[{ id, holder, margin }]` — not `Text`. The leading `.` here is ordinary
  **chaining** (each step returns a new typed carrier), exactly the meaning we
  reserved it for.
- **Each read is a TIP-0010 receiver closure over the iterated unit** — here a
  **row**, since these are all per-row ops. Inside `filter { … }` and
  `sort_by { … }`, and on the RHS of `extend(margin = …)`, the column names
  `status` / `premium` / `cost` / `created_at` are **bare** because they resolve
  against the row context — the same rule a method body uses for `self`. No
  `it.`, no string, no qualifier. (For the aggregations `agg` / `pivot` the same
  closure's `this` is instead the *group*, so a bare name is a `Column`; this
  example has none.)

So the result type is `Query[{ id: Int64, holder: Text, margin: EurAmt }]`, every
column reference is checked, and the predicate is a typed expression rather than a
SQL string. The carrier then lowers to SQL (predicate + projection pushdown).

`TODO(open):` the TIP-0008 **query carrier** is itself an open question there
(1.0 or later). The connection this example leans on — *that a dataframe read is
a TIP-0010 receiver closure* — is now **resolved** and stated in both TIPs (see
[TIP-0008](../book/src/tips/0008-named-axis-dataframes.md) §"The governing
principle" / §"One lambda flavor", and the matching item in TIP-0010). The
resolved rule is sharper than "`this` = the row": the receiver is **whatever the
operation iterates** — a **row** for the per-row ops (`filter` / `extend` /
`map`, so a bare name is a scalar field) and a **group** for the aggregations
(`agg` / `pivot`, so a bare name is a `Column`). This example uses only per-row
ops, so `this` is the row throughout.

`TODO(open):` clause **order vs schema**. `project` is written last so `filter`
/ `sort_by` can still see `status` / `created_at` before they are dropped. SQL's
`WHERE` / `ORDER BY` may reference unselected columns; the typed calculus cannot
once `project` has removed them. Decide whether the query carrier relaxes this
(remembers pre-projection columns for predicate/ordering) or requires
`project` last.

### Walking the results — an inline iterator (TIP-0009)

`each_row` is a *custom iterator* that feels like a built-in `for` because it is
`inline`, and whose block also carries a `Row` context so bare `col("…")` reads
the current row.

```tel
struct Row { cells: Map[Text, Text] }

fn col(r: Row, name: Text) -> Text {
    match r.cells.get(name) {
        some(v) => v,
        none    => "",
    }
}

# `inline` GRANTS the block the `outer` powers; the `Row.fn()` type GRANTS it a
# `this`. Inside the block:
#   bare `return`    ends just this iteration's block (a per-row skip)
#   `outer continue` skips to the next row, explicitly
#   `outer break`    stops the scan       — targets *this* helper's loop
#   `outer return v` leaves the ENCLOSING function with `v`
inline fn each_row(rows: List[Row], body: Row.fn() : Unit) {
    for r in rows {
        r.body()
    }
}
```

`outer return` — leave the function the moment a row matches. Note the three
kinds of name: `col` is a context member, `outer` jumps up, and there are no
bare lexical locals here:

```tel
fn first_active_holder(rows: List[Row]) -> Option[Text] {
    rows.each_row {
        if col("status") != "active" {
            outer continue                 # not this one — next row
        }
        outer return some(col("holder"))   # found it — leave the function
    }
    none                                   # no active rows
}
```

`outer break` — stop the scan early; here `out` and `name` are plain lexical
locals while `col` is the row context, side by side:

```tel
fn holders_until_blank(rows: List[Row]) -> List[Text] {
    let uniq out = ListBuilder[Text]()     # `out` — lexical local
    rows.each_row {
        let name = col("holder")           # `name` — local;  `col` — the row
        if name.is_empty() {
            outer break                     # stop at the first blank row
        }
        out.push(name)
    }
    out.finish()
}
```

The one-keyword seam is deliberate (TIP-0009): a built-in `for` body breaks with
a bare `break`; this helper's block needs `outer break`, telling the reader
"control is leaving through a helper, not a language loop."

This is the resolved TIP-0009 rule: `outer break` breaks the loop *where the
block is written* — and because `each_row` is `inline`, its `for` is spliced into
the declaring function, so that loop *is* the helper's `for`. "The one where it's
defined" and "the helper's internal loop" are the same loop after inlining.

---

## Example 3 — routing insurance-policy requests (TIP-0009 **and** TIP-0010)

The synergy case: a dispatcher where each rule is both a **receiver** block (bare
`param("id")` reads the matched path parameters) and an **inline** block
(`outer return` hands a `Response` straight back out of `dispatch`).

```tel
struct Request  { method: Text, path: Text, body: Text }
struct Response { status: Int64, body: Text }
struct Params   { values: Map[Text, Text] }

fn param(p: Params, name: Text) -> Text {
    match p.values.get(name) {
        some(v) => v,
        none    => "",
    }
}

# Match "/policies/:id/claims" against a concrete path, binding `:name` segments.
# Assumed stdlib: Text.trim_prefix(":") -> Text (drops a leading ":" if present).
fn match_path(pattern: Text, path: Text) -> Option[Params] {
    let ps = pattern.split("/")
    let cs = path.split("/")
    if ps.len() != cs.len() { return none }

    let uniq vals = MapBuilder[Text, Text]()
    for entry in ps.enumerate() {
        let pseg = entry.1
        let cseg = cs[entry.0]
        if pseg.starts_with(":") {
            vals.push(pseg.trim_prefix(":"), cseg)   # bind the param
        } else if pseg != cseg {
            return none                              # literal segment mismatch
        }
    }
    some(Params { values = vals.finish() })
}

# `on` is the bridge. It is `inline` ONLY to grant its block the `outer return`
# power — `on` itself contains no `outer`; the jump is written at the call site,
# inside the block. The block's `this` is the matched `Params`, so bare
# `param("id")` works.
inline fn on(req: Request, method: Text, pattern: Text, handle: Params.fn() : Unit) {
    if req.method == method {
        match match_path(pattern, req.path) {
            some(params) => params.handle(),   # run the matched block, this = params
            none         => {},                # fall through to the next rule
        }
    }
}
```

The dispatcher reads as a flat table. Each `req.on(…)` line starts with `req`
(not `.`), so it is a plain statement — no chaining. Watch the three name kinds
in one place: `param("id")` is the block's context, `req.body` is a lexical name
(dispatch's argument), and `outer return` jumps up out of `dispatch`:

```tel
fn dispatch(req: Request) -> Response {
    req.on("GET", "/policies") {
        outer return Response { status = 200, body = list_policies() }
    }
    req.on("GET", "/policies/:id") {
        outer return Response { status = 200, body = get_policy(param("id")) }
    }
    req.on("POST", "/policies/:id/claims") {
        # `param("id")` — context (Params);  `req.body` — lexical;  `outer return` — up.
        outer return Response { status = 201, body = open_claim(param("id"), req.body) }
    }

    # Reached only when no rule fired — every match left via `outer return`.
    Response { status = 404, body = "no route" }
}

fn list_policies()                  -> Text { "[ … policy list … ]" }
fn get_policy(id: Text)             -> Text { "{ policy ${id} }" }
fn open_claim(id: Text, body: Text) -> Text { "{ claim on ${id}: ${body} }" }
```

This is the receiver-**and**-inline cell, and it shows why the two TIPs are
orthogonal-but-composing rather than one feature:

- The **context** (`Params`) does the scope work (bare `param`), and could escape
  if stored.
- **`inline`** does the control-flow work (`outer return`), and forbids escape.
- A rule with neither is a value lambda; with only `inline` (no context) it is
  `with_lock(m) { … }`; with only a context it is the HTML builder above.

`on` mixes a `Params` context with `inline`, and that is fine by the resolved
rule: an inline parameter is checked non-escaping whether or not it also carries
a receiver (non-escaping is a control-flow condition; the receiver is orthogonal).
Here `handle` is invoked immediately and never stored, so it keeps its `outer`
powers.

---

## Decisions taken (were open questions)

- **`\` and receivers are one feature; no implicit `it`.** The old `\` = `|it|`
  sugar is dropped. A block's single input is its **receiver `self`**, reached by
  bare name exactly as a method body reaches `self`; `\` is just the
  single-expression spelling of a self-receiver block (`|x|` names the input
  instead and turns bare fall-through off). So Example 2's `.filter \ status ==
  "active"` is now valid *as written* — `self` is the row, `status` / `premium`
  are bare fields — no rewrite to `{ … }` needed.
- **Implicit context name — `self`** (not `this` / `it`). Methods and receiver
  blocks share the one name; where older prose above says `this`, read `self`.
  The *type* still carries the explicitness (`Recv.fn() : R` has a receiver,
  `Fn() -> R` does not); no per-literal marker.
- **Receivers are universal.** The same mechanism serves `list.map` (element =
  `self`), a dataframe row, an HTML builder, a matched route — receivers are not
  confined to record/builder carriers. Single input → `self` (bare); a named or
  multiple inputs → `|a, b|`; no input → param-less block.
- **Receiver ownership — owned, borrowed, or uniquely borrowed**, the same modes a
  method's `self` has: `Recv.fn() : R` (shared `Alias` borrow — read-only DSL /
  dataframe row), `!Recv.fn() : R` (unique borrow — the mutating builder), and a
  consuming form (`!Recv` by value, ended — a finaliser). `!` / `uniq` track
  ownership, not mutation.
- **Name-clash rule — decided, strict start.** A **local / parameter** colliding
  with a receiver member is a **compile error** (qualify `self.x` or rename the
  local) — stricter than Kotlin / Swift / C# / Scala, chosen because relaxing
  later is backwards-compatible and tightening is not; `TODO(open):` relax if too
  noisy, the constructor `self.x = x` being the sharp case. A **free function**
  never clashes (UFCS: `foo` ≡ `self.foo`). A **global with a qualified path** (a
  type, a module constant) loses to the member for a bare name, reachable by its
  path.
- **Rejected: every first argument an implicit receiver.** Considered for
  method/receiver uniformity, rejected — bare fall-through stays opt-in (UFCS
  already unifies *calls*; making it automatic hits the JavaScript-`with` regret
  and detonates the strict clash rule across every function body). Recorded in
  [antifeatures](../book/src/02-philosophy/04-antifeatures.md).
- **Receiver-block call spelling — `receiver.closure()`.** A receiver-closure
  parameter is a *value bound to a local name*, never a method of the receiver
  type (a "method" is just UFCS over a free function; a closure value isn't one).
  So `recv.run()` ≡ `run(recv)` with `recv` as the block's `this`. No ambiguity
  with a method `run` on `recv`: the parameter is the in-scope lexical `run`, and
  no-shadowing makes a same-named method in scope a conflict, not a silent
  alternative.
- **Implicit-context: the *type* carries the explicitness, not a per-literal
  token.** "No receiver vs implicit receiver" and "where do modifiers go" are
  answered by the parameter *type*: `Fn() -> R` has no receiver, `Recv.fn() : R`
  has one, and a modifier rides the type (`!Recv.fn()` for a `uniq` receiver).
  The call-site literal stays bare (`{ … }` = implicit `this`, `|h| { … }` =
  explicit named) so nested DSLs read well — requiring a token like `_` /
  `implicit` per literal would force `html |_| { head |_| { … } }` and kill the
  ergonomics. `TODO(open):` if a *visible* literal marker is still wanted, prefer
  `|self| { … }` (binds the context as `self`, self-documenting, modifier slot
  `|uniq self|`) over `_`/`implicit` — final "require it or not" is yours.
- **`outer break` / `outer continue` target — the loop where the block is
  written.** Because the inline helper is *spliced* into the declaring function,
  "the nearest enclosing loop at the block's site" *is* the helper's loop. So in
  `each_row`, `outer break` breaks the spliced `for` — "the one where it's
  defined" and "the helper's internal loop" are the same loop after inlining.
- **Inline + receiver escape-ness — yes, non-escaping, same rule.** Non-escaping
  is a control-flow soundness condition (you cannot `outer` into a frame that has
  gone); the receiver is an orthogonal axis and does not touch it. An `inline`
  parameter is non-escaping whether or not it also carries a receiver.
- **method-syntax revision — done.** The leading-`.`-stands-for-`self` rule in
  [method syntax](../book/src/09-functions/08-method-syntax.md) has been dropped;
  leading `.` is chaining only. Disambiguation is by no-shadowing (a collision is
  an error) and the explicit `self` keyword.

## Open questions still surfaced

- **Query carrier (TIP-0008).** Example 2's typed query relies on the TIP-0008
  *query carrier*, which is still open there (1.0 or later). (The connection that
  a dataframe read is a TIP-0010 receiver closure — `this` = the iterated unit, a
  row for per-row ops, a group for aggregations — is now resolved and recorded in
  both TIPs, so it is no longer an open item.)
- **Clause order vs schema** (Example 2). Whether the query carrier remembers
  pre-projection columns so `filter` / `sort_by` may reference unselected
  columns, or requires `project` last.
- **Trailing block with a parameter — resolved.** A trailing block **names** its
  input with `|x| { … }` (or `|x| …`), or takes it as the receiver `self` via
  `\ …` / `\{ … }`; a bare `{ … }` is param-less, its context (if any) coming from
  the type. `TODO(open):` only the rare "receiver block that *also* binds a
  `|name|`" grammar is still open — see
  `07-expressions/06-function-application.md`.
