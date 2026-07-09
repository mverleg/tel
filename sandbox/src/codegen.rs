//! Python code generation — a backend that stands *beside* `execute`.
//!
//! `execute` is the interpreting backend: it walks the monomorphised registry
//! and produces side effects directly. This module is the compiling backend:
//! it walks the *same* registry and lowers every reachable instance to a
//! single, self-contained Python script with a `#!/usr/bin/env python3`
//! shebang, so the emitted file is executable on its own.
//!
//! The point is to show the shape of a compiler backend, not to be a good one.
//! Two simplifications are deliberate and called out in the emitted comments:
//!
//! * Tel's `i32`/`i64` distinction is erased — Python integers are unbounded,
//!   so wrapping semantics are not modelled. Monomorphisation still happened,
//!   so each `(function, type)` pair becomes its own Python `def`; we just
//!   don't emit different arithmetic for the two.
//! * Every Tel expression yields a value, but Python separates statements from
//!   expressions, so control-flow forms (`if`, `let`, sequences) are lowered
//!   to statements writing into a fresh temporary, and the temporary is the
//!   value. This produces correct-but-verbose code with occasional dead
//!   assignments — exactly the kind of thing a real backend would optimise
//!   away and this one does not.
//!
//! The front end (resolve + monomorphise) is shared with `execute` verbatim:
//! codegen only replaces the last phase.

use crate::common::Interner;
use crate::context::ExecContext;
use crate::graph::{ExecId, MonoId, ResolveId};
use crate::types::{BinOp, MExpr, ResolveError, Ty, TypeError, Value};
use dashmap::DashMap;
use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

/// Anything that can stop codegen before a line of Python is emitted: the
/// shared front end failing. Codegen adds no failure modes of its own — a
/// well-typed program always lowers.
#[derive(Debug)]
pub enum CodegenError {
    Resolve(ResolveError),
    Type(TypeError),
}

impl std::fmt::Display for CodegenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CodegenError::Resolve(e) => write!(f, "{}", e),
            CodegenError::Type(e) => write!(f, "{}", e),
        }
    }
}

impl std::error::Error for CodegenError {}

impl From<ResolveError> for CodegenError {
    fn from(e: ResolveError) -> Self {
        CodegenError::Resolve(e)
    }
}

impl From<TypeError> for CodegenError {
    fn from(e: TypeError) -> Self {
        CodegenError::Type(e)
    }
}

/// Lower the program rooted at `path` to a standalone Python script.
///
/// Runs the exact front end `execute` runs — resolve the entry, then
/// monomorphise from it, which populates `ctx.mono_registry()` with every
/// reachable `(function, type)` instance — and then, instead of interpreting,
/// emits one Python `def` per instance plus a `main()` driver.
pub async fn generate_python(ctx: &ExecContext, path: ExecId) -> Result<String, CodegenError> {
    let main_loc = path.main_loc.clone();
    ctx.resolve_all(&[ResolveId { func_loc: main_loc.clone() }]).await?;
    // The entry point is called by no one, so — like the interpreter — its
    // type parameter defaults to the numeric default, i64.
    let entry = MonoId { func_loc: main_loc, ty: Ty::I64 };
    ctx.mono(entry)?;

    let gen = PyGen::new(ctx.interner(), ctx.mono_registry(), entry);
    Ok(gen.emit_module())
}

/// The whole emitter state: the shared interner (to render names for
/// comments), the monomorphised registry to lower, the chosen Python name for
/// every instance, and which instance is `main`.
struct PyGen<'a> {
    interner: &'a Interner,
    registry: &'a DashMap<MonoId, crate::types::MonoFuncData>,
    /// Emission order and per-instance Python identifier. Sorted for
    /// deterministic output (the registry is a concurrent map with no order).
    order: Vec<MonoId>,
    names: HashMap<MonoId, String>,
    entry: MonoId,
}

impl<'a> PyGen<'a> {
    fn new(
        interner: &'a Interner,
        registry: &'a DashMap<MonoId, crate::types::MonoFuncData>,
        entry: MonoId,
    ) -> Self {
        // Deterministic order: sort by (path, name, type). Without this the
        // output would shuffle run to run, which makes it useless for tests
        // and diffs.
        let mut order: Vec<MonoId> = registry.iter().map(|e| e.key().clone()).collect();
        order.sort_by(|a, b| Self::sort_key(interner, a).cmp(&Self::sort_key(interner, b)));

        // Assign a unique, readable Python identifier per instance. The entry
        // is always `main`; the rest are `<name>_<ty>`, disambiguated with a
        // numeric suffix if two files define the same name at the same type.
        let mut names = HashMap::new();
        let mut used: HashSet<String> = HashSet::new();
        used.insert("main".to_string());
        for id in &order {
            if *id == entry {
                names.insert(id.clone(), "main".to_string());
                continue;
            }
            let base = format!(
                "{}_{}",
                sanitize(id.func_loc.name_str(interner)),
                id.ty
            );
            let mut candidate = base.clone();
            let mut n = 1;
            while used.contains(&candidate) {
                candidate = format!("{}_{}", base, n);
                n += 1;
            }
            used.insert(candidate.clone());
            names.insert(id.clone(), candidate);
        }

        PyGen { interner, registry, order, names, entry }
    }

    fn sort_key(interner: &Interner, id: &MonoId) -> String {
        format!(
            "{}::{}::{}",
            id.func_loc.path_str(interner),
            id.func_loc.name_str(interner),
            id.ty
        )
    }

    /// Emit the complete module: shebang, prelude, one function per instance,
    /// then the entry-point guard.
    fn emit_module(&self) -> String {
        let mut out = String::new();
        out.push_str("#!/usr/bin/env python3\n");
        out.push_str("# Generated by the Tel sandbox Python backend (src/codegen.rs).\n");
        out.push_str("# One `def` per monomorphised (function, type) instance.\n");
        out.push_str("# NOTE: i32/i64 wrapping is not modelled — Python ints are unbounded.\n\n");

        // Tiny runtime prelude: the one place backend-specific semantics live.
        // Tel integer division truncates toward zero (like Rust); Python `//`
        // floors, so we cannot lower `/` to `//` directly.
        out.push_str("def _div(a, b):\n");
        out.push_str("    # Tel division truncates toward zero; Python // floors.\n");
        out.push_str("    q = abs(a) // abs(b)\n");
        out.push_str("    return -q if (a < 0) != (b < 0) else q\n\n\n");

        for id in &self.order {
            self.emit_function(&mut out, id);
            out.push_str("\n\n");
        }

        out.push_str("if __name__ == \"__main__\":\n");
        writeln!(out, "    {}()", self.names[&self.entry]).unwrap();
        out
    }

    fn emit_function(&self, out: &mut String, id: &MonoId) {
        let data = self.registry.get(id).expect("id came from the registry");
        let name = &self.names[id];
        let params: Vec<String> = (1..=data.arity).map(|i| format!("a{}", i)).collect();

        writeln!(
            out,
            "# {}::{} @ {}",
            id.func_loc.path_str(self.interner),
            id.func_loc.name_str(self.interner),
            id.ty
        )
        .unwrap();
        writeln!(out, "def {}({}):", name, params.join(", ")).unwrap();

        let mut body = Body::new();
        let value = self.emit(&mut body, &data.ast, 1);
        for line in &body.lines {
            out.push_str(line);
            out.push('\n');
        }
        writeln!(out, "    return {}", value).unwrap();
    }

    /// Lower one expression. Statement-producing forms push lines into `body`
    /// at `indent`; the return value is a Python *expression string* that
    /// evaluates to this Tel expression's value (Tel expressions are all
    /// value-producing).
    fn emit(&self, body: &mut Body, expr: &MExpr, indent: usize) -> String {
        match expr {
            MExpr::Number(v) => match v {
                Value::I32(n) => n.to_string(),
                Value::I64(n) => n.to_string(),
            },
            MExpr::VarRef(var) => format!("v{}", var.0),
            MExpr::Arg(n) => format!("a{}", n),
            MExpr::BinaryOp { op, left, right } => {
                let l = self.emit(body, left, indent);
                let r = self.emit(body, right, indent);
                match op {
                    BinOp::Add => format!("({} + {})", l, r),
                    BinOp::Sub => format!("({} - {})", l, r),
                    BinOp::Mul => format!("({} * {})", l, r),
                    BinOp::Div => format!("_div({}, {})", l, r),
                    // Comparisons and logic yield 1/0 in Tel, not Python bools.
                    BinOp::Greater => format!("(1 if {} > {} else 0)", l, r),
                    BinOp::Less => format!("(1 if {} < {} else 0)", l, r),
                    BinOp::Equal => format!("(1 if {} == {} else 0)", l, r),
                    BinOp::And => format!("(1 if ({} != 0 and {} != 0) else 0)", l, r),
                    BinOp::Or => format!("(1 if ({} != 0 or {} != 0) else 0)", l, r),
                }
            }
            MExpr::Let { var, value } | MExpr::Set { var, value } => {
                let val = self.emit(body, value, indent);
                body.push(indent, format!("v{} = {}", var.0, val));
                format!("v{}", var.0)
            }
            MExpr::If { cond, then_branch, else_branch } => {
                let c = self.emit(body, cond, indent);
                let tmp = body.fresh();
                // Tel truthiness is `!= 0`.
                body.push(indent, format!("if {} != 0:", c));
                let then_val = self.emit(body, then_branch, indent + 1);
                body.push(indent + 1, format!("{} = {}", tmp, then_val));
                body.push(indent, "else:".to_string());
                let else_val = self.emit(body, else_branch, indent + 1);
                body.push(indent + 1, format!("{} = {}", tmp, else_val));
                tmp
            }
            MExpr::Print(inner) => {
                let val = self.emit(body, inner, indent);
                body.push(indent, format!("print({})", val));
                val
            }
            MExpr::Return(inner) => {
                let val = self.emit(body, inner, indent);
                body.push(indent, format!("return {}", val));
                // Dead after the return, but every arm must yield a value expr.
                val
            }
            MExpr::Panic { source_location, .. } => {
                body.push(
                    indent,
                    format!("raise Exception({:?})", format!("panic at {}", source_location)),
                );
                "0".to_string()
            }
            MExpr::Call { func, args } => {
                let arg_exprs: Vec<String> =
                    args.iter().map(|a| self.emit(body, a, indent)).collect();
                let name = self.names.get(func).cloned().unwrap_or_else(|| {
                    // Every reachable instance is monomorphised, so this should
                    // not happen; fall back to a readable-if-broken name.
                    format!(
                        "MISSING_{}_{}",
                        sanitize(func.func_loc.name_str(self.interner)),
                        func.ty
                    )
                });
                format!("{}({})", name, arg_exprs.join(", "))
            }
            MExpr::Sequence(exprs) => {
                if exprs.is_empty() {
                    return "0".to_string();
                }
                let (last, rest) = exprs.split_last().unwrap();
                for e in rest {
                    let val = self.emit(body, e, indent);
                    // Keep the value expression as a bare statement so any side
                    // effect (a call) still runs; pure ones are harmless no-ops.
                    body.push(indent, val);
                }
                self.emit(body, last, indent)
            }
        }
    }
}

/// Accumulates the indented statement lines of one function body plus a
/// counter for fresh temporaries.
struct Body {
    lines: Vec<String>,
    tmp: usize,
}

impl Body {
    fn new() -> Self {
        Body { lines: Vec::new(), tmp: 0 }
    }

    fn push(&mut self, indent: usize, line: String) {
        self.lines.push(format!("{}{}", "    ".repeat(indent), line));
    }

    fn fresh(&mut self) -> String {
        let t = format!("t{}", self.tmp);
        self.tmp += 1;
        t
    }
}

/// Turn a Tel identifier into a safe Python identifier fragment.
fn sanitize(name: &str) -> String {
    let mut out: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect();
    if out.is_empty() || out.chars().next().unwrap().is_ascii_digit() {
        out.insert(0, '_');
    }
    out
}
