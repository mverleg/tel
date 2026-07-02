mod execute;
mod parse;
mod resolve;
mod types;
mod graph;
mod context;
mod common;

use std::fmt;
use std::collections::HashSet;
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

pub async fn run_file(path: &str, show_deps: bool) -> Result<(), Error> {
    let printer: &'static dyn Printer = Box::leak(Box::new(StdoutPrinter));
    run_file_with_printer(path, show_deps, printer).await
}

pub async fn run_file_with_printer(path: &str, show_deps: bool, printer: &'static dyn Printer) -> Result<(), Error> {
    //TODO @mark: get rid of leak if ever continuous process without shared cache
    let core = Box::leak(Box::new(Global::new(printer)));
    let ctx = RootContext::new(core);
    let exec_id = ExecId { main_loc: FQ::intern(ctx.interner(), path, "main") };
    ctx.execute(exec_id).await
        .map_err(|e| Error::Execute("main".to_string(), e))?;

    if show_deps {
        printer.print("\nDependency tree:");
        let mut visited = HashSet::new();
        visited.insert(StepId::Root);
        if let Some(my_root_deps_ref) = ctx.graph().get_dependencies(&StepId::Root) {
            let my_root_deps: Vec<_> = my_root_deps_ref.iter().collect();
            let dep_count = my_root_deps.len();
            for (idx, dep) in my_root_deps.iter().enumerate() {
                let is_last = idx == dep_count - 1;
                visualize_tree(&ctx, dep, "", is_last, &mut visited, printer);
            }
        }
    }

    Ok(())
}

