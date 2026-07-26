# Linear Resources: Connections, Transactions, Senders

This showcase exercises Tel's [substructural types](../12-memory-and-runtime/08-substructural-types.md)
end to end. It builds a tiny "transfer money between accounts" routine on top
of a database, and shows how *linear by default* turns three classic bugs —
a leaked connection, a forgotten commit, and a mutable value aliased into a
race — into compile errors, while ordinary data stays effortless.

The four cells of the affine × relevant table all show up here:

| Value | `Alias`? | `Discard`? | Why |
| --- | --- | --- | --- |
| `Db` connection | no | no | one writer (no races), must `close` (no leak) — **linear** |
| `Txn` | no | no | one owner, must `commit`/`rollback` — **linear** |
| `RowBuf` scratch buffer | no | yes | one writer, but may be abandoned to GC — **affine** |
| `Sender` (audit log) | yes | no | many producers, each must be released — **relevant** |
| `Money`, `AccountId` | yes | yes | plain immutable data — **unrestricted** |

## Plain data is free

Values you would never think twice about carry both capabilities
automatically, derived from their fields. No annotation, no ceremony:

```tel
type AccountId = record { the_value: Text }     # all fields unrestricted
type Money     = record { the_cents: Int64 }      # => Alias + Discard, derived

let my_amount = Money(cents: 5_00)
let my_copy   = my_amount                        # aliasing is just binding — no .clone()
log(my_amount); log(my_copy)                     # both still usable; dropping is fine
```

Because `Money` is `Alias`, the second binding is not a move; because it is
`Discard`, nothing forces you to "use" it. This is the world most languages
give *every* value — Tel just makes it the thing you opt into, not the default.

## A linear connection: leak becomes a compile error

A database connection must have exactly one writer (or you get races) and must
be closed (or you leak a socket). That is precisely **linear** — so `Db`
derives *neither* capability. It exposes a single consuming use method,
`close`, and an `AutoUse` so the common path needs no keystroke:

```tel
type Db = relevant record { the_socket: HostSocket }   # mutable, no Alias/Discard

fn close(a self: Db) -> () { self.the_socket.shutdown() }   # the "use"
impl AutoUse for Db { fn use(a self: Db) { self.close() } }

fn run() {
    let my_db = Db.connect("postgres://...")
    do_work(my_db)
    # my_db.close() inserted here automatically by AutoUse —
    # the compiler proved this is where my_db's life ends
}
```

Remove the connection from the proven-drop path and the guarantee bites:

```tel
fn leak() -> () {
    let my_db = Db.connect("postgres://...")
    return                       # COMPILE ERROR: `my_db` is relevant and never used.
                                 #   help: call `close`, or let it reach end of scope so
                                 #         AutoUse can close it.
}
```

And because `Db` is affine, you cannot smuggle a second writer out:

```tel
fn race(a db: Db) {
    let my_other = db            # MOVE: `db` is now inaccessible — no second owner exists
    spawn { my_other.write(...) }
    db.write(...)               # COMPILE ERROR: `db` was moved into `my_other`
}
```

## A transaction: forgotten commit becomes a compile error

`Txn` is linear for the same reasons, but it has **two** use methods — there
is no single "right" way to retire it, so it deliberately omits `AutoUse`:

```tel
type Txn = relevant record { the_db: &!Db }   # borrows the connection

fn commit(a self: Txn)   -> () { self.the_db.exec("COMMIT") }
fn rollback(a self: Txn) -> () { self.the_db.exec("ROLLBACK") }

fn transfer(a db: &!Db, a from: AccountId, a to: AccountId, a amt: Money) -> Result[()] {
    let my_txn = db.begin()
    my_txn.debit(from, amt)?            # `?` on a relevant Result — propagates if Err
    my_txn.credit(to, amt)?
    my_txn.commit()                     # the use. Forgetting it does not compile.
}
```

Two bugs are now unrepresentable:

- **Forgotten settle.** Dropping `my_txn` without `commit`/`rollback` is a
  compile error — there is no `AutoUse` and no `Discard`, so the value has
  nowhere to go.
- **Lost error.** `debit` returns a `Result`, which is itself relevant; the
  `?` is how it is used. Ignoring the `Result` would not compile either, so a
  failed debit can never be silently skipped while the credit proceeds.

Note the `&!Db` borrow: the transaction *borrows* the connection rather
than owning it, so the connection's own linearity is untouched — the borrow
suspends `db` for the transaction's scope and reinstates it after.

## A scratch buffer: affine but discardable

Building a row buffer needs one writer (mutable ⇒ affine), but if a query
errors out half-built there is nothing to clean up — you can just walk away
and let GC reclaim it. So `RowBuf` opts into `Discard` only:

```tel
type RowBuf = discard record { the_rows: !List[Row] }   # affine + Discard

fn collect(a db: &!Db, a q: Query) -> Result[List[Row]] {
    let uniq my_buf = RowBuf()
    for a row in db.stream(q)? {        # if `?` short-circuits, my_buf is just abandoned —
        my_buf.the_rows.push(row)       # no compile error, no leak, GC handles it
    }
    Ok(my_buf.the_rows.finish())
}
```

`RowBuf` is still affine — you cannot alias a half-built buffer into another
task — but it is not relevant, so the early `?` return is allowed to drop it.

## Multiple producers: aliasable but relevant

The audit log is fed by several tasks at once, so its `Sender` must be
**aliasable** — but the channel should close promptly once *every* sender is
gone, so each sender must be released. That is `Alias` **without** `Discard`:

```tel
let (the_tx, the_rx) = audit_channel()        # the_tx: Sender[Audit] — Alias, relevant

spawn { let mine = the_tx; mine.send(...); mine.release() }   # alias #1, released
spawn { let mine = the_tx; mine.send(...); mine.release() }   # alias #2, released
the_tx.release()                                              # original, released
# when the last live Sender is released, the_rx observes channel-closed
```

Each `mine` is a fresh alias of one underlying channel handle — *not* a deep
copy, and pointedly *not* spelled `.clone()` (forking a reference and copying
data must never look alike). But every alias is relevant: skip a `release()`
and that task does not compile, which is what guarantees the channel cannot
hang open because one producer forgot to leave.

## Why this is a good fit for Tel

- The dangerous defaults — leak, lost error, mutable alias — are exactly the
  ones you must *opt out* of, so the safe version is the one you get for free.
- Nothing here needed a lifetime annotation, an `unsafe`, or a GC finaliser.
  The obligations are checked at compile time and cost nothing at runtime; on
  task [abort](../13-error-handling/04-panics-and-aborts.md) the whole heap is
  dropped wholesale, so relevance never forces failure-path destructors.
- Ordinary data (`Money`, `AccountId`) stays as lightweight as in any
  scripting language — the machinery only appears where a real resource is at
  stake.

## See also

- [Substructural Types](../12-memory-and-runtime/08-substructural-types.md) —
  the affine/relevant model and the `Alias`/`Discard`/`AutoUse` mechanism.
- [References and Aliasing](../12-memory-and-runtime/04-references-and-aliasing.md)
  — the `&!T` borrow the transaction uses.
- [Error Propagation](../13-error-handling/03-error-propagation.md) — why
  `Result` is relevant.
