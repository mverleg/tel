mod execute;
mod parse;
mod resolve;
mod typecheck;
mod types;
mod graph;
mod context;
mod common;
mod keys;
mod store;

use std::fmt;
use std::collections::HashSet;
use std::sync::Arc;
use crate::common::{Ctx, FQ};
use crate::context::{Global, RootContext};
use crate::graph::{ExecId, StepId};

pub trait Printer: Send + Sync {
    fn print(&self, message: &str);
}

pub struct StdoutPrinter;

impl Printer for StdoutPrinter {
    fn print(&self, message: &str) {
        println!("{}", message);
    }
}

pub struct NoopPrinter;

impl Printer for NoopPrinter {
    fn print(&self, _message: &str) {
        // Do nothing
    }
}

#[derive(Debug)]
pub enum Error {
    Io(String, std::io::Error),
    Parse(String, types::ParseError),
    Resolve(String, types::ResolveError),
    Execute(String, types::ExecuteError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Io(path, e) => write!(f, "IO error in {}: {}", path, e),
            Error::Parse(path, e) => write!(f, "Parse error in {}: {}", path, e),
            Error::Resolve(name, e) => write!(f, "Resolve error in {}: {}", name, e),
            Error::Execute(name, e) => write!(f, "Execute error in {}: {}", name, e),
        }
    }
}

impl std::error::Error for Error {}

fn visualize_tree(ctx: &RootContext, step: &StepId, prefix: &str, is_last: bool, visited: &mut HashSet<StepId>, printer: &dyn Printer) {
    let connector = if is_last { "└── " } else { "├── " };
    printer.print(&format!("{}{}{}", prefix, connector, Ctx(step, ctx.interner())));

    if visited.contains(step) {
        let extension = if is_last { "    " } else { "│   " };
        printer.print(&format!("{}{}(already shown)", prefix, extension));
        return;
    }
    visited.insert(step.clone());

    if let Some(my_deps_ref) = ctx.graph().get_dependencies(step) {
        let my_deps: Vec<_> = my_deps_ref.iter().collect();
        let dep_count = my_deps.len();

        for (idx, dep) in my_deps.iter().enumerate() {
            let is_last_dep = idx == dep_count - 1;
            let extension = if is_last { "    " } else { "│   " };
            let new_prefix = format!("{}{}", prefix, extension);
            visualize_tree(ctx, dep, &new_prefix, is_last_dep, visited, printer);
        }
    }
}

fn print_dependency_tree(ctx: &RootContext) {
    let printer = ctx.printer();
    printer.print("\nDependency tree:");
    let mut visited = HashSet::new();
    visited.insert(StepId::Root);
    if let Some(my_root_deps_ref) = ctx.graph().get_dependencies(&StepId::Root) {
        let my_root_deps: Vec<_> = my_root_deps_ref.iter().collect();
        let dep_count = my_root_deps.len();
        for (idx, dep) in my_root_deps.iter().enumerate() {
            let is_last = idx == dep_count - 1;
            visualize_tree(ctx, dep, "", is_last, &mut visited, printer);
        }
    }
}

/// A persistent, reusable compiler process: create one, compile against it
/// repeatedly, drop it when done.
///
/// [`run_file`] builds a fresh `Compiler` on every call, so nothing is cached
/// between compiles. Holding a `Compiler` instead keeps one [`Global`] — the
/// content store and binding layer — alive across every
/// [`run`](Compiler::run) call. That is the incremental-from-main mode of
/// plans/roadmap.md: each run is a demand-driven pull from the entry point,
/// re-deriving content keys top-down and stopping at cache hits, so only what
/// main transitively needs — and among that, only what actually changed — is
/// recomputed. An unchanged or reverted source file costs a read + digest,
/// never a recompute; a changed file is recomputed (never served stale)
/// because its content key differs.
///
/// Runs are serialized by `&mut self`: the positional mono registry is
/// per-run state, so two interleaved runs on one `Compiler` must not race it.
/// (Separate `Compiler`s may run concurrently.) Ownership is ordinary `Arc`
/// sharing — dropping the `Compiler` reclaims everything once in-flight
/// spawned tasks finish; nothing is leaked.
pub struct Compiler {
    core: Arc<Global>,
}

impl Compiler {
    /// Create a compiler with an empty, persistent cache.
    pub fn new(printer: Arc<dyn Printer>) -> Compiler {
        Compiler { core: Arc::new(Global::new(printer)) }
    }

    /// Compile and execute `path`, reusing anything already cached from previous
    /// runs of this `Compiler`.
    pub async fn run(&mut self, path: &str, show_deps: bool) -> Result<(), Error> {
        let ctx = RootContext::new(self.core.clone());
        let exec_id = ExecId { main_loc: FQ::intern(ctx.interner(), path, "main") };
        ctx.execute(exec_id).await
            .map_err(|e| Error::Execute("main".to_string(), e))?;

        // The ancestor-path check makes a cyclic import fail resolution, so a
        // compile that got this far must have an acyclic resolve graph; the
        // post-hoc DFS stays as a debug-only verification of that invariant.
        debug_assert!(
            ctx.graph().find_resolve_cycle(&exec_id.main_loc).is_none(),
            "compile succeeded but the resolve graph contains a cycle"
        );

        if show_deps {
            print_dependency_tree(&ctx);
        }

        Ok(())
    }

    /// Number of distinct source contents parsed and cached so far. Lets callers
    /// observe cross-run reuse: an unchanged or reverted file leaves this
    /// unchanged on the next [`run`](Compiler::run).
    pub fn cached_parse_count(&self) -> usize {
        self.core.cached_parse_count()
    }

    /// Number of distinct monomorphised instances cached so far, keyed by
    /// source content + instance. Editing a file adds fresh entries only for
    /// the instances defined in that file; unchanged or reverted files reuse
    /// their entries on the next [`run`](Compiler::run).
    pub fn cached_mono_count(&self) -> usize {
        self.core.cached_mono_count()
    }

    /// Number of parse computations actually executed. Unlike
    /// [`cached_parse_count`](Compiler::cached_parse_count) this counts work
    /// done rather than entries stored: a cache hit — including a cached
    /// *error* — does not grow it.
    pub fn computed_parse_count(&self) -> usize {
        self.core.computed_parse_count()
    }

    /// Number of mono checks actually executed (cache hits excluded).
    pub fn computed_mono_count(&self) -> usize {
        self.core.computed_mono_count()
    }

    /// Number of distinct resolve answers cached so far, keyed by the parse
    /// answer's fingerprint plus the imports' answer fingerprints.
    pub fn cached_resolve_count(&self) -> usize {
        self.core.cached_resolve_count()
    }

    /// Number of resolve computations actually executed. A cache hit —
    /// including a cached *error* — does not grow it; nor does serving a
    /// resolve whose key was re-derived but found stored (that is the
    /// transitive validation working).
    pub fn computed_resolve_count(&self) -> usize {
        self.core.computed_resolve_count()
    }
}

pub async fn run_file(path: &str, show_deps: bool) -> Result<(), Error> {
    run_file_with_printer(path, show_deps, Arc::new(StdoutPrinter)).await
}

pub async fn run_file_with_printer(path: &str, show_deps: bool, printer: Arc<dyn Printer>) -> Result<(), Error> {
    // Each call builds a fresh, single-shot compiler (no cross-run cache). Use
    // `Compiler` directly to keep the cache alive across runs.
    Compiler::new(printer).run(path, show_deps).await
}

