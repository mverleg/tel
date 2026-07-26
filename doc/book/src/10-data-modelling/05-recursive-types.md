# Recursive Types

A **recursive type** is one whose definition refers to itself, directly or
through another type. Trees, linked lists, JSON, expression ASTs — most
interesting data is recursive.

```tel
type Json =
    | JsonNull
    | JsonBool   { value: Bool }
    | JsonNumber { value: Real64 }
    | JsonString { value: Text }
    | JsonArray  { items: List[Json] }
    | JsonObject { fields: Map[Text, Json] }

struct TreeNode[T] {
    value:    T,
    children: List[TreeNode[T]],
}

# Recursive *union*: a list is empty or a head plus a tail-list.
type IntList = (Empty | Cons[Int64])
struct Cons[T] { head: T, tail: List[T] }
```

## What — references to oneself, mediated by a container

Tel allows a type to refer to itself in a field's type, **provided the
reference is through a container** (a `List`, a `Map`, an `Option`, another
record). A direct, value-typed self-reference — a field whose type *is* the
enclosing record — is not allowed, because such a type would have infinite
size.

```tel
# OK: indirection through a container.
struct Node { value: Int64, next: Option[Node] }

# Not OK: infinite size — every Node would contain another Node, forever.
struct Bad  { value: Int64, next: Bad }
```

For unions the same applies: members are allowed to refer back to the union
type they belong to (`JsonArray { items: List[Json] }` is fine), but a member
that *is* the union by value would loop.

## Why — and how it works under the hood

Recursive data is the default in real-world domain modelling, so Tel makes it
ordinary, not a special construct. The "must go through a container" rule
exists to keep types finitely sized at the *value* level; the container holds
a heap reference internally so the recursion only follows when the data
actually has children.

This is an *implementation* concern that is mostly invisible — scripts see
`Node` and `Option[Node]`, not allocations. The host runtime is responsible for
the indirection. See `impl-notes/` for how a host may represent it (boxing,
arenas, region allocation).

## Recursive unions and the wrapper-struct rule

Generic recursive unions still need the [wrapper-struct discipline](04-generic-data-types.md)
that keeps untagged unions from collapsing. A `Tree[T] = (Leaf[T] | Branch[Tree[T]])`
only works because `Leaf[T]` and `Branch[...]` are wrapper-tagged types:

```tel
struct Leaf[T]   { value: T }
struct Branch[T] { left: Tree[T], right: Tree[T] }
type   Tree[T]   = (Leaf[T] | Branch[T])
```

Without the wrappers, `Tree[Int64] = (Int64 | Tree[Int64] | ...)` would flatten and
deduplicate in surprising ways (think back to `Option[Option[T]]` — see
[`02-union-types.md`](02-union-types.md)). The wrappers give each variant a
real identity, which `match` can dispatch on cleanly.

## Pattern matching on recursive types

Recursive types are most often consumed by pattern matching that recurses
itself. `match` in Tel supports binding the matched value's parts so the
recursive call has the pieces it needs:

```tel
fn depth[T](tree: Tree[T]) -> Int64 {
    match tree {
        _: Leaf     => 1,
        b: Branch   => 1 + max(depth(b.left), depth(b.right)),
    }
}

# Pattern-matching head/tail of a list:
fn sum(items: List[Int64]) -> Int64 {
    match items {
        Empty      => 0,
        c: Cons    => c.head + sum(c.tail),
    }
}
```

See [`06-pattern-matching-in-depth.md`](06-pattern-matching-in-depth.md) for
more on `match`, including matching head-and-tail of a list.

## Mutual recursion between types

Recursion can also be **mutual**: two or more types refer to each other.

```tel
struct Expr   { op: Op, args: List[Expr] }
struct Op     { name: Text, body: Option[Expr] }
```

The same finite-size rule applies — the references must go through a
container, here `List` and `Option`. The compiler must see all the types
together to type-check the cycle; in Tel that is the normal case, since types
in the same module are mutually visible.

TODO(open): order-of-declaration sensitivity. Some languages require a forward
declaration or a special `rec`/`and` block for mutual recursion. Tel's
working assumption is that types in the same module can refer to each other
freely regardless of declaration order; confirm and document.

## Recursive types and host serialisation

Recursive types — especially `Json`-shaped unions — are the standard payload
shape for crossing the host boundary. There is a real concern here: a
deserializer producing a value of a recursive type must respect any
record-level invariant or refined-type constraint each layer carries (see
[refined types and the outside world](../05-types/12-refined-types.md)). For
deeply nested data this is potentially expensive; the conservative rule is
that constraints run *as values are adopted*, so a malformed deep tree fails
loudly rather than getting partway through.

TODO(open): exact mechanism by which host serialisation traverses recursive
types and validates constraints at each layer. Tied to the schema/codegen
serialisation story.

## What Tel does *not* do with recursive types

Tel floats — and on balance rejects — several powerful but costly
features:

- **Recursive type aliases without wrappers.** `type T = (List[T] | Text)` is
  recursively *flat* and works in some ML-family languages. Tel insists on the
  wrapper-struct rule, because untagged-union collapse hurts here too; the
  wrapper-struct discipline is the answer.
- **Self-referential structs (Rust's `Pin` / `&'self`).** A struct field that
  is a *borrow* of another field of the same struct is a powerful tool — and
  a famously hard one. Rust spent years on `Pin` to make it sound. Tel has no
  exposed borrow / lifetime story (see
  [`../02-philosophy/04-antifeatures.md`](../02-philosophy/04-antifeatures.md)),
  so the question does not arise for users; values are values, references
  through containers are how cycles get expressed.

TODO(open): genuine reference *cycles* in user data (two records that hold
each other through `Option[...]` and form a cycle at runtime). Pure-value
semantics make this hard to construct accidentally — a value is built bottom
up, so there is no "and now point back at the parent" moment. If reference
cycles end up being possible in some host runtime, they need a story
(generational GC, weak references). Lean: cycles are simply not constructible
in user code, and a host that exposes a graph-shaped value must convert it to
an acyclic Tel shape (an explicit `id` / lookup pair) at the boundary. Flag
and defer.

## See also

- [Generic Data Types](04-generic-data-types.md) — the wrapper-struct rule.
- [Union Types](02-union-types.md) — and the collapse problem.
- [Pattern Matching In Depth](06-pattern-matching-in-depth.md).
- [Collection Types](09-collection-types.md) — `List`, `Map`, etc., as the
  containers recursion hides behind.

TODO: review
