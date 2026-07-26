# I/O and Filesystem

<!-- TODO: review -->

## What

`std` describes file and stream I/O — `File`, directory, reader/writer types
and the operations on them — but **there is no ambient I/O**. A script cannot
open a file, read stdin, or write stdout on its own. Every I/O operation is
reached through a **capability** the host explicitly hands the script.

## Why: no ambient I/O

This is one of Tel's defining commitments
([`../02-philosophy/04-antifeatures.md`](../02-philosophy/04-antifeatures.md)):

- **Sandboxing is the default.** A script that was not given a filesystem
  capability simply cannot touch the filesystem — there is no global to reach
  for. The host decides, per script, what is reachable.
- **Reproducibility.** With I/O injected rather than ambient, a test can hand
  the script a fake filesystem and get deterministic behaviour.
- **One script, many hosts.** A browser host has no filesystem; it grants no
  filesystem capability, and the script must cope. The same source still runs
  — it just sees fewer capabilities.

## How it looks

The host passes a capability in as an ordinary argument. The script can only
do what the capability's type allows:

```tel
# The host injects `fs`; the script has no other filesystem access.
fn load_rules(fs: ReadDir) -> Result[List[Rule], IoError] {
    let text = fs.read_text("rules.tel")?
    parse_rules(text)
}
```

A capability is a normal value: it can be a *narrow* grant (read-only access
to one directory, `ReadDir` above) rather than blanket filesystem power. The
host chooses the narrowest type that still lets the script do its job.

A script that does I/O *down its call tree* but does not use it directly can
thread the capability through implicitly as part of an injected *context*,
without naming it in every intermediate signature. `TODO(open): the
context-threading mechanism (how a capability passes through functions that
do not use it) is only sketched, not designed — coordinate with
the capabilities chapter.`

## Resource cleanup

Tel is garbage-collected, so file handles and other host resources are **not**
cleaned up by RAII/destructors. Cleanup is **scope-based** — a `with`-style
construct (Python `with`, Java try-with-resources) closes the handle when the
scope ends. Because Tel has no exceptions and aborts rather than unwinds (see
[the error-handling antifeature](../02-philosophy/04-antifeatures.md)), a
crashing task drops its whole heap at once; scoped cleanup covers the handles
that the heap drop does not. `TODO(open): exact spelling of the scoped-cleanup
construct, and whether a task abort still runs it.`

## I/O, blocking, and concurrency

Host-exposed I/O operations may block. How blocking I/O interacts with Tel's
task model — whether an I/O-bound capability call should be marked so the
scheduler can place it off the CPU pool — is discussed in
[`12-concurrency-utilities.md`](12-concurrency-utilities.md) and
[`../19-use-cases/01-hello-world.md`](../19-use-cases/01-hello-world.md).

## Atomic file writes

A common, load-bearing pattern: writing to a file
"atomically" — appearing in the destination only when fully written, so a
crash mid-write leaves the previous file intact, not a half-written one.
The library exposes this directly:

```tel
# Sketch — syntax not pinned down. `dir` is a host-granted ReadWriteDir.
dir.write_atomic("report.json", body) ?

# Equivalent expanded form:
let tmp = dir.create_temp_in(".report.json.tmp") ?
tmp.write_all(body) ?
tmp.fsync() ?                          # see "Durability" below
dir.rename(tmp.path, "report.json") ?  # atomic on the same filesystem
```

The capability must support both temp-file creation in the same
directory (so the final rename can be atomic on the same filesystem) and
the rename itself. `TODO(open): rename atomicity on Windows / on some
network filesystems is weaker than on POSIX; document the contract the
capability promises.`

## File locking

The library exposes advisory file locking through the filesystem
capability — explicit, not implicit. A handle can be opened with a lock
mode (`shared`, `exclusive`, `none`), and the lock policy is part of the
opening contract. The required modes:

- **Reader / writer split**, with fairness options (`fair`,
  `prefer_reader`, `prefer_writer`).
- **Try-lock** for non-blocking acquisition; the operation returns a
  `Result` indicating contention.
- **Auto-release on scope exit** through the scoped-cleanup construct
  above.

`TODO(open): file locking is host-OS-dependent — Unix `flock` vs Windows
`LockFileEx` vs network filesystems where locking is unreliable. Decide
how much of the variance the library hides and how much it exposes
through the capability type.`

## Directory utilities

The most common directory operations are simple enough that the library
ships them directly — every one of them goes through the capability, never
through an ambient global:

- **`read_dir(path)`** — iterate entries; cheap, lazy.
- **`walk(path)`** — recursive iterator, depth- or breadth-first;
  "sorely lacking in Python," so the library treats it as core.
- **`find(path, predicate)`** — recursive search returning matching paths.
- **`create_temp_file()` / `create_temp_dir()`** — host-managed temporary
  storage, reachable only through a capability that grants it.
- **`next_available_name(dir, base, ext)`** — yields
  `report-1.json`, `report-2.json`, … without rolling the logic by hand.

The host may also expose well-known directory shortcuts as separate
capabilities — `current_dir`, `project_dir`, `user_config_dir`,
`system_config_dir`, `temp_dir` — each as a narrow `ReadDir` or
`ReadWriteDir`, never as a blanket "filesystem" handle. The list of
shortcuts available depends on what the host environment can promise
(a browser host promises very little). `TODO(open): exact set of
well-known directories; the answer is host-shaped and may belong in
[`10-os-and-process.md`](10-os-and-process.md).`

## Compile-time file inclusion

A separate `std` facility lets a source unit *depend* on a non-Tel file
(text, JSON, schema, SQL) at compile time:

```tel
# Sketch — exact spelling TBD.
let schema_text: Text = include_text!("schema/v1.sql")
let schema_bytes: Bytes = include_bytes!("templates/logo.png")
```

The file is read once during compilation, baked into the binary, and the
file is registered as a *build dependency* so changes invalidate the cache.
The file must not be modified mid-compilation; the
compiler should warn (or fail) when it detects a race. `TODO(open): this
is a metaprogramming-shaped feature; it conflicts mildly with the
"no heavy metaprogramming" rule
([`../02-philosophy/04-antifeatures.md`](../02-philosophy/04-antifeatures.md)).
Decide whether it lives in `std`, in the build tool, or as a `derive`-class
attribute. Most likely a tightly scoped `std` macro.`

## Durability

A `write_all` returns when the bytes are handed to the OS. Whether the
bytes are *on disk* is a separate question; the capability exposes a
`fsync()` for callers that need durability before continuing. The library
does not call `fsync` for the user — the cost is too high to pay
implicitly, and most scripts don't need it. The atomic-write helper
above sequences fsync ↔ rename correctly so callers don't have to. 
`TODO(open): fsync semantics vary across hosts; document the contract
narrowly.`

## See also

- [Time](09-time.md)
- [Networking](11-networking.md)
- [OS and Process](10-os-and-process.md)
- [Antifeatures](../02-philosophy/04-antifeatures.md)
