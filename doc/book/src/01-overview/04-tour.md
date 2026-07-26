# A Tour of Tel

<!-- TODO: review -->

A quick guided walk through the language. The goal is to give a feel for what
Tel looks like and how its pieces fit together; depth and precision live in
later chapters, linked from each section.

Syntax in this tour is **pseudocode**. Tel's surface is not yet pinned down,
so the examples lean Tel-ish but stay loose — reading them you should grasp
the *intent*, not commit to exact spellings.

For deeper rationale see
[Priorities](../02-philosophy/01-priorities.md),
[Features](../02-philosophy/03-features.md), and
[Antifeatures](../02-philosophy/04-antifeatures.md).

## A first script

A Tel script is a small thing the host hands data and capabilities to:

```tel
# A scoring rule a host calls per order.
# The host injects `clock` and `log`; the script has no other I/O.
fn score(an_order: Order, a_clock: Clock, a_log: Log) -> Result[Score, Reject] {
    if an_order.total <= EuroAmt(0) {
        return Err(Reject.NonPositiveTotal)
    }
    let age_days = a_clock.now().days_since(an_order.placed_at)
    a_log.info("scoring order", an_order.id, age_days)
    Ok(Score.from(an_order, age_days))
}
```

Two things to notice up front:

- `Clock` and `Log` come in as parameters. There is no ambient `print`, no
  global clock, no implicit filesystem — see
  [Capability-based I/O](../02-philosophy/03-features.md) and
  [no ambient I/O](../02-philosophy/04-antifeatures.md).
- Errors are values returned through `Result`, not exceptions. See
  [Error handling](../13-error-handling/01-philosophy.md).

## Values, types, and bindings

Tel is **statically and strictly typed**. Public signatures are explicit;
local bindings can be inferred where unambiguous.

```tel
let n = 42                # inferred as Int64
let title: Text = "Tel"   # explicit
let prices: List[EuroAmt] = []
```

Bindings are **immutable by default**. There is no `null` and no
uninitialised binding. Optionality is an `Option`-shaped type. Mutability,
where it appears at all, is explicit.

`TODO(open): exact spelling of mutable bindings — see the mutability open
question in [`../02-philosophy/04-antifeatures.md`](../02-philosophy/04-antifeatures.md).`

## Records

A **record** combines several named fields into a value. Records are Tel's
product type; in other languages they may be called *struct*, *class*, or
*data class*.

```tel
record Person {
    name: Name,
    birthday: PastDate,
}

let alice = Person(
    name = Name.new("Alice").unwrap(),
    birthday = PastDate(1988, 1, 31),
)
```

Records can carry behaviour, either as inherent methods or by implementing
traits:

```tel
impl Person {
    fn greet(self, log: Log) {
        log.info("hello,", self.name)
    }
}
```

Records cannot extend other records. See
[no inheritance](../02-philosophy/04-antifeatures.md) for why; shared
behaviour goes through traits and delegation. See
[`../10-data-modelling/01-records.md`](../10-data-modelling/01-records.md).

## Unions

A **union** says a value is *one of* several types. It is Tel's sum type;
elsewhere called *enum*, *sealed class*, or *oneof*.

```tel
record Network { socket: Socket, secure: Bool }
record Disk    { fh: FileReadHandle }

type DataSource = (Network | Disk)
```

Tel unions are **untagged**: the type *is* the tag. `(Text | Int64 | Int64)`
collapses to `(Text | Int64)`. A given record can belong to **multiple** unions
without changing it:

```tel
record A {}
record B {}
record C {}

type P = (A | B)
type Q = (B | C)   # B participates in both, unchanged
```

A `match` over a union is **exhaustive** by default — every member must be
handled or the compiler complains:

```tel
fn describe(src: DataSource) -> Text = match src {
    Network(n) => "network, secure=" + n.secure.to_text(),
    Disk(d)    => "disk fh=" + d.fh.to_text(),
}
```

See [`../10-data-modelling/02-union-types.md`](../10-data-modelling/02-union-types.md)
and [Match expressions](../08-control-flow/02-match-expressions.md).

`TODO(open): whether union members must be concrete types or whether traits
can also participate — see the open question in
[`../02-philosophy/04-antifeatures.md`](../02-philosophy/04-antifeatures.md).`

## Traits

A **trait** describes behaviour a type can implement. Polymorphism in Tel is
trait dispatch — there is no class inheritance.

```tel
trait Greeter {
    fn greet(self, log: Log)
}

impl Greeter for Person {
    fn greet(self, log: Log) {
        log.info("hi,", self.name)
    }
}
```

Generic functions can require multiple traits at once — an *intersection
type* used as a type bound:

```tel
fn pythagoras[T](x: T, y: T) -> T
    where T: Add[T, Out = T] + Mul[T, Out = T]
{
    x * x + y * y
}
```

The `T: Add + Mul` part is a bound, not a value type — you cannot construct
an `Add + Mul`, only require it. See
[`../10-data-modelling/03-traits-or-interfaces.md`](../10-data-modelling/03-traits-or-interfaces.md).

## Errors as values

There are no exceptions and no `try`/`catch`. Fallible functions return
`Result[Ok, Err]` and the caller decides what to do.

```tel
fn parse_amount(s: Text) -> Result[EuroAmt, ParseError] {
    let n = Int64.parse(s)?            # short-circuit on Err
    if n < 0 { return Err(ParseError.Negative) }
    Ok(EuroAmt(n))
}
```

A `?`-shaped operator (final spelling TBD) propagates an `Err` to the
caller; otherwise it unwraps to the `Ok` value. See
[`../13-error-handling/01-philosophy.md`](../13-error-handling/01-philosophy.md).

## Capabilities, not ambient I/O

Anything outside the program — files, network, clock, randomness, environment
— arrives as a **capability** the host hands in:

```tel
fn snapshot(fs: FileSystem, clock: Clock, target: Path) -> Result[Unit, IoError] {
    let now = clock.now()
    let name = target.with_extension("snap-" + now.iso())
    fs.write(name, gather_state())
}
```

A script with no `FileSystem` parameter cannot touch the disk; a host running
in a browser simply does not hand one out. This is also what makes time and
randomness trivially mockable for tests. See
[`../02-philosophy/03-features.md`](../02-philosophy/03-features.md).

## Tasks, not threads

Concurrency is expressed as **tasks** — small units of work the host's
runtime can schedule however it wants (fiber, worker thread, JS microtask,
sequential continuation). User code never names a thread or a mutex.

```tel
let a = task { compute_left() }
let b = task { compute_right() }
combine(a.await, b.await)
```

The same script runs on a host that has real parallelism and on a host that
runs tasks sequentially on a tick. See
[`../14-concurrency-and-parallelism/01-overview.md`](../14-concurrency-and-parallelism/01-overview.md)
and the
[host-portable concurrency](../02-philosophy/03-features.md) commitment.

## Modules — only when you want them

A 30-line modding hook needs no modules. When code grows, group related
items into modules and import what you need:

```tel
import drawing.shapes as shapes

let r = shapes.Rect(10, 20)
```

You can rename a module on import for clarity or to resolve a clash, but
individual members keep the name they were defined with. See
[`../11-modules-and-packages/01-modules.md`](../11-modules-and-packages/01-modules.md).

## What you have not had to think about

By this point in a tour of many other languages you would have met threads,
mutexes, GC tuning, allocators, `null`, exceptions, class hierarchies, or a
build system. Tel deliberately keeps all of these out of the surface — most
because they belong to the host, the rest because they make small scripts
harder to read.

## Where to go next

- [Goals and Non-Goals](03-goals-and-non-goals.md) — what Tel commits to and
  what it deliberately rejects.
- [Priorities and Trade-offs](../02-philosophy/01-priorities.md) — the ranked
  principles that decide design calls.
- [Records](../10-data-modelling/01-records.md),
  [Union Types](../10-data-modelling/02-union-types.md),
  [Traits](../10-data-modelling/03-traits-or-interfaces.md) — the data
  modelling story in depth.
- [Use Cases](../19-use-cases/01-hello-world.md) — small worked examples.
