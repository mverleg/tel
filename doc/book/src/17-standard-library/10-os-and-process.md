# OS and Process

<!-- TODO: review -->

## What

This topic covers what a Tel script can — and mostly cannot — do with the
operating system: environment variables, process spawning, exit codes,
randomness, and similar OS facilities.

The short answer: **almost none of it is available by default.** Tel is a
*guest* inside a host program. The host owns the OS, the process, and the
process lifecycle; a script reaches an OS facility only through a host-granted
capability, and most hosts grant none.

## Why: the host owns the machine

This follows directly from the embedding philosophy
([`../02-philosophy/04-antifeatures.md`](../02-philosophy/04-antifeatures.md)):

- **No ambient OS access.** No global environment, no implicit process exit,
  no ambient randomness. A script that wants an environment variable receives
  an `Env` capability; one that was not given it cannot read the environment.
- **No process lifecycle.** Tel does not own `main`-to-exit, signals, or
  shutdown — those belong to the host. A standalone-application story is an
  explicit non-goal
  ([`../01-overview/03-goals-and-non-goals.md`](../01-overview/03-goals-and-non-goals.md)).
- **No low-level machine access.** No threads, mutexes, raw memory, or
  syscalls in user code.

## Randomness is a capability

Randomness is treated exactly like time and I/O: a script that needs random
values receives a random-source capability from the host. There is **no
ambient global RNG**. This keeps runs reproducible — a test injects a seeded
or scripted source — and keeps untrusted scripts from a side channel the host
did not approve.

```tel
# The host injects `rng`; without it, the script has no randomness.
fn pick_winner(entries: List[Entry], rng: Random) -> Entry {
    entries.at(rng.below(entries.size()))
}
```

`TODO(open): randomness (and `unsafe`) could be modelled as
*effects* with a handler, rather than as plain injected values. Decide
alongside the effects/capabilities design; for now an injected capability
value is the model.`

## Process spawning and native work

If a script genuinely needs to run a subprocess or call native code, that is
**the host's job**: the host exposes the specific operation it is willing to
allow as a capability. Tel does not provide a general "spawn any process"
facility — that would be ambient OS power by another name. A hot native
kernel likewise belongs in the host behind a capability, not in Tel
([`../01-overview/03-goals-and-non-goals.md`](../01-overview/03-goals-and-non-goals.md)).

When the host *does* expose subprocess execution, the
"command" must be a **value, not a string**:

```tel
# Sketch — syntax not pinned down. `shell` is a host-granted capability.
let cmd = Command("rg", ["--json", pattern])
    .stdin(Stdin.Null)
    .timeout(Duration.seconds(5))
let result = shell.run(cmd) ?
```

A command-as-value can be wrapped, timed, retried, logged, piped, mocked
in tests, and inspected without re-parsing a shell string. Shell-style
string commands are rejected: they encourage injection bugs and make it
impossible to attach the metadata above. `TODO(open): the exact shape of
`Command` — argv-style only, or piped-graph (`cmd1 | cmd2`) — and how
piping interacts with the task model.`

## Standard streams

Stdin, stdout, and stderr are **not** ambient. When a host wants to grant them,
they appear as arguments to the script's entry point — capabilities like any
other:

```tel
fn main(stdin: Read, stdout: Write, stderr: Write, env: Env) -> ExitCode {
    ...
}
```

A browser host grants none of these; a CLI-shaped host grants all four. A
script that wants to read stdin must accept it; one that wasn't given
stdout has nowhere to print. `TODO(open): the exact signature of an entry
point — whether `main` is named or anonymous, whether the capabilities
are passed individually or as a *context* (see
[`08-io-and-filesystem.md`](08-io-and-filesystem.md)). Both options are open.`

`TODO(open): is `print` (to stdout/stderr) the one ambient exception?` Treating
stdout/stderr as capabilities is the consistent position, but it makes the most
basic debugging — dropping a `print` into a function — require threading a
capability down to that function. The pragmatic argument: assume Linux-style
that *some* stdout/stderr almost always exists, and make `print` ambient as the
single exception to [no ambient I/O](../02-philosophy/04-antifeatures.md). A
host that genuinely runs Tel with no console can still discard the stream. This
trades a core philosophy maxim for ergonomics, so it is a **philosophy-chapter
decision**, not one to settle here. Lean: allow ambient `print`/`eprint` for
diagnostics only, with all *structured* output still going through granted
capabilities. Pre-pivot tension — re-justify against embedding.

The library also sketches a *typed-shell* alternative: streams whose
payload is structured (JSON or a binary equivalent), auto-rendered to
text when connected to a terminal. The motivation is to make piped Tel
programs compose by type rather than by string. `TODO(open): treat
typed-shell as a direction, not a feature. Re-justify against embedding:
this only earns its place if Tel is also used as a shell language, which
is a *secondary* use case.`

## Environment, args, and config

The same rule applies to environment variables, command-line arguments,
and config file lookups: each is a capability the host grants. The library
provides a parameter-parsing helper that uniformly falls back from CLI
args to env vars to a config file — but only when the script is given the
relevant capabilities. `TODO(open): the parameter-parsing helper is on
the boundary of "stdlib" and "third-party library"; the lean is toward stdlib
because parameter handling is a near-universal need for scripts, but the
fit with embedding is weaker than for the core capabilities.`

## CLI argument parsing

For scripts that *are* host entry points, `std` exposes a declarative
argument parser: the script describes its arguments as a typed record,
and the parser produces a value (or a structured error / `--help` text).
Prior art the design should copy: Rust's `clap` (in its derive form) and
Python's `argparse`, with the names normalised to Tel.

```tel
# Sketch — syntax not pinned down.
record Args {
    input: Path,
    @short("o") output: Option[Path],
    @flag verbose: Bool,
    @rest extras: List[Text],
}

fn main(argv: Args, stdin: Read, stdout: Write) -> ExitCode {
    ...
}
```

The parser:

- Reports usage / help / version from the record's declarations; the
  script does not write the help text by hand.
- Supports subcommands as a union over per-subcommand record types.
- Surfaces parse failures as a structured `Err`, not a process exit, so
  the script picks the exit code.
- Reads from the argv capability the host hands to `main`. A host that
  does not grant argv (e.g. a browser embedding) simply has no `Args` to
  pass.

`TODO(open): the declarative-record form requires attribute-driven
codegen, which sits on the narrow edge of Tel's metaprogramming antifeature.
Coordinate with [Derive and Attributes](../15-metaprogramming/03-derive-and-attributes.md);
the alternative is a builder-style API that produces the same parser at
runtime.`

## Resource-bound execution

A host that runs untrusted scripts can cap their OS-level footprint — see the
resource-bound execution mode in
[`../18-tooling/01-compiler.md`](../18-tooling/01-compiler.md), which limits
total operations and memory so a runaway script cannot starve the host.

## See also

- [I/O and Filesystem](08-io-and-filesystem.md)
- [Time](09-time.md)
- [Compiler](../18-tooling/01-compiler.md) — resource-bound execution
- [Randomness, Hashing and Crypto](15-randomness-hashing-and-crypto.md) —
  the `Random` capability surface
