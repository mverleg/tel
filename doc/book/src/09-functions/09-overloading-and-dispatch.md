# Overloading and Dispatch

<!-- TODO: review -->

Tel has **no function overloading by arity or by argument type**. Two
functions in the same scope must have *different names*. Polymorphism over
types is supplied by **traits**, and the variation people usually reach for
overloading to express is supplied by **default and named arguments**.

## What — and what it isn't

```tel
fn area(r: Rect) -> Real64 { r.w * r.h }
fn area(c: Circle) -> Real64 { 3.14 * (c.r * c.r) }   # REJECTED — same name
```

The two definitions above are a name clash, not two overloads. The Tel
answers are:

- Give them different names (`area_of_rect`, `area_of_circle`), or
- Make `Shape` a [union](../10-data-modelling/02-union-types.md) of
  `(Rect | Circle)` and write one `area(s: Shape)` that pattern-matches, or
- Define an `Area` [trait](../10-data-modelling/03-traits-or-interfaces.md)
  and implement it for each type, then call `s.area()`.

Bluntly: shipping overloads, even just by arity, multiplies what
a name can mean at a call site and interacts badly with
[default arguments](04-default-and-named-arguments.md) and
[function references](07-higher-order-functions.md). One name, one function.

## What replaces overloading

### 1. Default and named arguments

For "the same operation with optional extras," use defaults — see
[Default and Named Arguments](04-default-and-named-arguments.md). One
signature replaces a family of overloads:

```tel
# instead of three overloads for `connect`
fn connect(host: Text, port: Int64 = 8080, retries: Int64 = 3) -> Connection
```

This is the *primary* answer to "but I want overloads."

### 2. Union-typed parameters plus `match`

For "accept any of several distinct input types," use a union parameter and
dispatch inside the body:

```tel
fn describe(v: Int64 | Text | Date) -> Text {
    match v {
        Int64(n)  => "${n} (a number)"
        Text(s) => "\"${s}\""
        Date(d) => d.iso()
    }
}
```

The dispatch is *visible*. There is no hidden overload resolution that picks
a different body depending on the static type — the same body runs and a
`match` chooses the branch.

### 3. Trait dispatch

For "different *behaviour* per type, called uniformly," use a trait — Tel's
form of polymorphism (see
[Traits or Interfaces](../10-data-modelling/03-traits-or-interfaces.md)):

```tel
trait Area {
    fn area(self) -> Real64
}

impl Area for Rect   { fn area(self) -> Real64 { self.w * self.h } }
impl Area for Circle { fn area(self) -> Real64 { 3.14 * (self.r * self.r) } }

shape.area()        # resolves through the trait
```

A trait method call **is** a kind of dispatch — but it is a single, named,
documented kind, not free-form overloading. The trait is the contract; the
impl is one of its implementations.

`TODO(open): static vs dynamic trait dispatch.` Whether a trait method is
always statically resolved (monomorphised per concrete impl) or whether
trait-object dynamic dispatch exists at all is open. Tel is wary of
dynamic dispatch (it complicates AOT compilation and bounds analysis) but
admits some cases need it. Defer to
[Traits or Interfaces](../10-data-modelling/03-traits-or-interfaces.md).

### 4. Method-call syntax is not overloading

`x.f(y)` is just `f(x, y)` — [method syntax](08-method-syntax.md). Two
free functions named `f` with different first-parameter types would still be
a name clash. Method-call syntax does not give you overloading by receiver
type; trait dispatch does.

## Operator overloading is embraced

Tel deliberately **keeps operator overloading** for user types — `+`, `-`,
`*`, `/`, `==`, `<`, etc. — even though it rejects custom operators, custom
precedence, and heavy metaprogramming. It is allowed only as the *expression*
form of implementing a language-defined trait. See
[Precedence and Associativity](../04-syntax/04-precedence-and-associativity.md#operator-overloading).
This is *not* function overloading: there is one operator, with one parse, and
the trait says what it does for a given type.

The reason it earns its place where other symbolic tricks do not: for the
operations everyone already knows, a conventional symbol is *as clear as* a
named method — `a * b` reads no worse than `a.mul(b)`, and for a `Matrix`,
a `EuroAmt`, or a `Vec3` it reads better. That clarity holds only because the
operator set, its precedence, and the contract behind each operator are all
**fixed by the language**: a reader never has to learn a project's private
operators or re-derive a custom precedence. Overloading changes only the
per-type *meaning*, never the syntax — which is exactly why it does not become
the hidden-behaviour hazard that custom operators
([antifeatures](../02-philosophy/04-antifeatures.md)) would be. The matching
constraint — an overloaded operator must behave like the operator it spells,
with no hidden effects (no I/O, no lazy loading behind `*`) — is covered with
the operator-trait set in
[Precedence and Associativity](../04-syntax/04-precedence-and-associativity.md#operator-overloading).

There is also a floated `__multi_add__`-style hook so `a + b + c + d`
dispatches as a single n-ary call instead of nested binary calls — useful for
types where adding many values at once is materially cheaper. That is an
optimisation detail of `+`, not a separate kind of overload.

`TODO(open): n-ary operator dispatch.` Whether `__multi_add__` (or some
better name) exists in 1.0 is open; it conflicts mildly with *one good way*
and is an optimisation, not a correctness feature. Decide alongside the
operator-trait set.

### Making a value callable (`__call__`) — leaning no

A related idea is a `__call__`-style hook that makes an *arbitrary value*
callable like a function — so `widget(x)` would invoke an overloaded call
operator on `widget` rather than only working when `widget` is a function. This
is the call-operator analogue of the operators above.

Tel is **leaning against** offering it. A call site `widget(x)` that does not
actually call a function — and instead dispatches through a hidden operator on
whatever `widget` happens to be — is exactly the kind of surprise the
no-overloading and *if it looks correct, it is correct* rules exist to avoid: a
reader can no longer tell from `widget(x)` whether a function is being called.

`TODO(open): should `__call__` (overloading function-call on a value) exist at
all? Leaning NO — seen as confusing.`

## Constraints this rules out

There are cases overloads-by-type *do* cover that the
defaults / union / trait combination does not:

- *"Accept a `Text`, **or** an `Int64` and a `Float`"* with each shape having
  its own body and types fixed at compile time. A union parameter `Text |
  (Int64, Float)` plus `match` works, but the dispatch is at runtime, not at
  the call site.
- *Adding a method with different arity* is not a backwards-compatible
  change, because [function references](07-higher-order-functions.md) embed
  arity.

`TODO(open): API evolution & function references.` Note that "adding
a method overload with different arity isn't compatible, because of method
references." Since Tel forbids overloading anyway, the binary question
becomes: which signature changes *are* compatible? Add-with-default, yes.
Reorder of named-only parameters, probably yes. Adding a new positional
parameter, no. The full list belongs in a stability chapter and is currently
unresolved.

## Why no overloading

- **Stability.** A frozen API gains parameters via defaults; it does not
  sprout sibling overloads that quietly change which one a call resolves to.
- **Readability.** *If it looks correct, it is correct.* Two functions with
  the same name and different signatures force a reader (and an IDE) to
  re-derive overload resolution before they know what runs.
- **One good way.** A union + `match` is explicit. A trait is explicit. A
  default-argument signature is one definition. Overloads add a fourth axis
  on top.
- **Easier tooling.** A function reference, a trace, a "go to definition"
  click each have *one* destination per name in a scope.

## Bugs the no-overloading rule prevents

A concrete catalogue case: two
overloads `from(B)` and `from(C)` existed in one library. A class `A
implements B` worked fine — calls resolved to `from(B)`. A *later* change
added `interface C` to `class A` (`A implements B, C`). Every call
`from(new A())` became ambiguous, and the compile failure surfaced in
*caller* repositories, not in the change that introduced it.

This is the structural problem Tel rejects overloading to prevent. With no
overloading, a generic function `fn from[T: B](...)` and `fn from[T: C](...)`
collide on the name — the author chooses different names at the
declaration site, and adding an interface implementation to a third type
later cannot break far-away callers.

Variants from the same catalogue worth recording:

- **`toDate(Int64)` and `toDate(long)` with different meanings.** Both
  overloads parsed the same value as a date but with different
  conventions (packed date vs timestamp). The widening `Int64 → long`
  silently picked the wrong overload. With one name per intent
  (`packed_date_to_date`, `timestamp_to_date`), the right operation is
  chosen at the call site, not by the compiler's resolution rules.

## How it looks in practice

```tel
# Want "render a value as text"?  Trait, not overloads.
trait AsText {
    fn as_text(self) -> Text
}

impl AsText for Int64  { fn as_text(self) -> Text { format_int(self) } }
impl AsText for Date { fn as_text(self) -> Text { self.iso() } }

# Want "connect with optional retries"?  Defaults, not overloads.
fn connect(host: Text, port: Int64 = 8080, retries: Int64 = 3) -> Connection { ... }

# Want "accept either a name or an id"?  Union + match, not overloads.
fn find(who: UserId | Text) -> Option[User] {
    match who {
        UserId(id) => by_id(id)
        Text(name) => by_name(name)
    }
}
```

## See also

- [Default and Named Arguments](04-default-and-named-arguments.md)
- [Method Syntax](08-method-syntax.md)
- [Higher-Order Functions](07-higher-order-functions.md)
- [Traits or Interfaces](../10-data-modelling/03-traits-or-interfaces.md)
- [Union Types](../10-data-modelling/02-union-types.md)
- [Precedence and Associativity](../04-syntax/04-precedence-and-associativity.md) —
  operator overloading.
- [Features](../02-philosophy/03-features.md) — traits, no inheritance.
- [Antifeatures](../02-philosophy/04-antifeatures.md) — no implicit
  conversions; no surprise control flow.
