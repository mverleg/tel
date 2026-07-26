# Variadic Functions

<!-- TODO: review -->

A *variadic* function accepts any number of arguments in its last parameter
position.

## What

A variadic parameter collects zero or more trailing arguments into a single
sequence the body can iterate. It is written with the **`vararg`** keyword
before the parameter name:

```tel
fn sum_all(vararg values: Int64) -> Int64 {
    let uniq total = 0
    for v in values { total = total + v }
    total
}

sum_all()              # 0
sum_all(1, 2, 3)       # 6
```

- The variadic is the **last positional** parameter; everything before it is
  matched positionally as usual. It **ends the positional section**, so any
  parameters after it are
  [keyword-only](02-parameters-and-arguments.md#parameter-sections-positional-vararg-keyword-only-block).
- Inside the body the parameter behaves as an immutable sequence of the element
  type.

The `vararg` keyword is settled. It replaces the earlier `...`/postfix-`Type...`
sketches: the marker is a **word, not a sigil**, matching the keyword style used
across [parameter sections](02-parameters-and-arguments.md#parameter-sections-positional-vararg-keyword-only-block)
and sidestepping the token shared by the `...` continuation marker and the
`...rest` destructuring marker.

A caller can **splat** an existing bundle into a variadic call with a leading
`...`, the same spelling as any other [bundle splat](../05-types/04-tuples-and-arrays.md#tuples-as-argument-bundles):
the bundle's trailing positionals spread into the vararg, which collects them at
the call.

```tel
let pts = (p1, p2, p3)
draw(0, 0, ...pts)          # the three points spread into `vararg points`
```

Because the splat is written and the bundle's shape is statically known, this
desugars to an ordinary call (see
[splatting a bundle into a call](../07-expressions/06-function-application.md#splatting-a-bundle-into-a-call)).
Splatting an *unknown*-shape bundle is the separate row-polymorphism question and
is not part of this.

TODO(open): whether variadics are needed at all, given that passing an explicit
`List[T]` is one extra pair of brackets and keeps signatures simpler. The
input mentions variadics only in passing; this topic is intentionally short.

## Why

- Variadics suit a small set of genuinely n-ary operations (a `max` of several
  values, a string-join). For most "many values" cases, an explicit list
  argument is clearer and is what Tel reaches for first — *one good way over
  many clever ones* ([priorities](../02-philosophy/01-priorities.md)).
- They must not become a backdoor form of [overloading](09-overloading-and-dispatch.md):
  a variadic is *one* function, with *one* element type.

## See also

- [Parameters and Arguments](02-parameters-and-arguments.md)
- [Default and Named Arguments](04-default-and-named-arguments.md)
- [Tuples and Arrays](../05-types/04-tuples-and-arrays.md)
