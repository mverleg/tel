# Default and Named Arguments

<!-- TODO: review -->

Tel lets a parameter have a **default value** and lets a caller pass arguments
**by name**. Together these are how a function signature grows over time
*without breaking existing callers* — which, for a language frozen at 1.0,
matters a great deal.

## What

### Default values

A parameter may declare a default, used when the caller omits that argument. A
defaulted parameter is **keyword-only** — it lives after the `*` marker (see
[Parameter sections](02-parameters-and-arguments.md#parameter-sections-positional-vararg-keyword-only-block)),
so it is always passed by name:

```tel
fn connect(host: Text, *, port: Int64 = 8080, retries: Int64 = 3) -> Connection {
    ...
}

connect("example.org")                      # port = 8080, retries = 3
connect("example.org", retries = 5)         # port = 8080, retries = 5
```

### Named (keyword) arguments

Keyword-only parameters are passed by name, in any order:

```tel
connect("example.org", retries = 5)        # skip `port`, set `retries`
connect("example.org", port = 9000)
```

`host` is positional, so it is *not* named — only keyword-only parameters are.
Named arguments let a caller skip earlier optional parameters and make a call
site self-documenting — `retries = 5` reads better than a bare `5`.

This is the same name-based idea as
[object construction](../06-bindings-and-scope/06-destructuring.md): match by
name, not just position. Punning may extend here too — passing a local `port`
as `port = port` could shorten to `port` — though this is so far defined
only for record construction.

This is now settled by the **binary** parameter-section model: a parameter is
either positional (never named, no default) or keyword-only (always named, may
default), with a `vararg` and a trailing `block` as the other sections — see
[Parameter sections](02-parameters-and-arguments.md#parameter-sections-positional-vararg-keyword-only-block).
So "every parameter callable by name" is answered *no* (only keyword-only ones),
and positional arguments never follow named ones (they are in different
sections).

## Backwards-compatible signature evolution

This is the *reason* the feature exists. Adding a parameter with a default does
not break existing calls — they keep compiling and behaving identically:

```tel
# v1
fn render(page: Page) -> Html

# v2 — every v1 call site still works unchanged
fn render(page: Page, theme: Theme = Theme.default) -> Html
```

Defaults and named arguments are also Tel's answer to *not having function
overloading* (see [Overloading and Dispatch](09-overloading-and-dispatch.md)):
one function with optional parameters replaces a family of overloads.

### The limits

This does not cover every case overloads would:

- It cannot express "accept a `Text`, **or** an `Int64` and a `Float`" as
  distinct shapes. A union parameter `(Text | Int64)` is checkable, but selecting
  behaviour on it is a runtime `match`, not a compile-time overload choice.
- Even adding an optional parameter is not *always* perfectly compatible — it
  can interact with [function references](07-higher-order-functions.md) and
  arity-sensitive call resolution: "adding a method
  overload with different arity isn't compatible, because of method
  references."

TODO(open): exactly which signature changes Tel guarantees as compatible —
add-with-default clearly; the interaction with function references and with
named-argument resolution needs a precise rule. Defer to the stability/
compatibility discussion.

## Why

- **Stability.** A frozen language still needs APIs that can take on new
  options. Defaults make "add a parameter" a non-breaking change.
- **One way, not overloads.** A single function with optional parameters is
  one signature to read and one body to maintain — *one good way over many
  clever ones* ([priorities](../02-philosophy/01-priorities.md)).
- **Readable calls.** Named arguments turn anonymous positional values into
  labelled ones at the point a reader sees them.

## See also

- [Parameters and Arguments](02-parameters-and-arguments.md)
- [Overloading and Dispatch](09-overloading-and-dispatch.md) — why Tel has no
  overloading, and what replaces it.
- [Variadic Functions](05-variadic-functions.md)
- [Destructuring and Object Construction](../06-bindings-and-scope/06-destructuring.md)
