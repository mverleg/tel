
# Tel syntax

## Goal

The goal of the syntax is to 

* The syntax should be left-recursive with lookahead of 1, to be easy and fast to parse
* The syntax should look terse for small scripts, while expressive enough for medium programs

## Passes

Note that both forward references and recursion are allowed without special syntax (and the
AST is immutable), therefore resolving variable references is a separate 'pass', after 
lexing and building the AST.

## Details

Some details about the syntax:

* Indentation is not significant, but newlines are; statements must end with ';' and/or a newline
* You can split expressions over multiple lines by
  - breaking after a symbol that can't be the end, e.g. '+', '(' or '='
  - breaking before `.`
  - adding `...` before the linebreak
* Comments start with '#' and are always single-line
* Types are always required on the signatures of public 'things' (enums, structs, functions), otherwise can be omitted if inferrable
* For reasons of performance, simplicity and backwards compatibility, type inference is from expression to result and not exceptionally smart
* Variables can be declared without any keyword (if immutable), or with "mut" or "local"
* There is a preference for left-to-right style, with some operators having attribute syntax (e.g. `.assert`)
* Closures that take 0 or 1 arguments and don't need type annotations can be written as just `{...}` anywhere an expression is expected, and can use `it` as the arg
* Closures that take more than 1 argument are written the same as functions, e.g. `fn(a, b) {...}`
* Closures can be placed outside a function invocation, and will be passed as the last positional argument
* Using `self` can be omitted when used, and is not declared as part of functions
* Lexical scope corresponds to blocks wrapped in `{` and `}`, whether functions, closures or statements 
* Existing conventions are followed in many cases, even if there are theoretical argumetns for other ways. For example, `[T]` makes sense for generics as it is one in a family of types. And `f{x, y} ( return x + y )` makes sense, because arguments are data and the body is code, and e.g. structs use `{}` to group data, while expressions are grouped by `()`. But both of these would be really confusing for programmers coming from other languages

## Type syntax

The guiding rule is that **a type is written like a value, with each literal replaced by its type**:

* A tuple value `(1, 3, x=8)` has type `(int32, int32, x=int32)` — each slot's type independently, named slots keep their name
* A closure value `fn(a, b) { a + b }` has type `fn(int32, real64) : int32` — the body block is replaced by `: <return type>`

Notes:

* Union types have no value-shaped layout, because a value inhabits exactly one arm. They are written within parentheses: `(int32 | text | real64)`
* Function types use the `fn` keyword, matching the value syntax for multi-argument closures (`fn(a, b) {...}`). The `|x, y|`-style is **not** used for function types: it would collide with the union `|`, and a leading `fn` keeps the first token unambiguous for the left-recursive, lookahead-1 parser.

### Bracketing return types

The return type after `:` extends maximally to the right. Every type form is self-delimited (its own syntax marks where it ends) *except* a function type, whose trailing `: T` has an open right edge:

* atoms are a single token: `int32`
* tuples and unions close with `)`: `(int32, text)`, `(int32 | text)`
* generics close with `]`: `[int32]`
* a function type does not close itself: `fn(int32) : T` runs on

Therefore a return type must be grouped **if and only if it is itself a function type** — and the grouping bracket is the ordinary grouping parenthesis `()`, the same one that groups any expression or type, not a special bracket:

```
fn() : int32                          # bare: atom is self-delimited
fn() : (int32, text)                  # bare: tuple closes itself
fn() : (int32 | text)                 # bare: union closes itself
fn() : [int32]                        # bare: generic closes itself
fn(int32) : (fn(int32, real64) : int32)   # grouped: return is a function
```

`()` is chosen over `{}`: `{}` already means a code block (and a struct's data body), so under "types mirror values" it would have to mirror *block*, not *grouping* — using it to group a type invents a third meaning and breaks the mirror. `()` already means "group one thing" for expressions, so mirroring it into types as "group one type" is consistent. There is no collision with a one-tuple: a tuple needs a top-level comma or named field (`(x,)`, `(a = 1)`), so `(fn(int32) : int32)` — having neither — reads as grouping, exactly per the existing tuple-vs-grouping rule.

A function type appearing as an argument or tuple element does not need grouping, because the surrounding `,` or `)` already bounds it (e.g. `fn(fn() : i32, i32) : i32`).

