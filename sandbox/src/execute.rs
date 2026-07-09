use crate::types::MExpr;
use crate::types::MonoFuncData;
use crate::types::ExecuteError;
use crate::types::BinOp;
use crate::types::Ty;
use crate::types::Value;
use crate::types::VarId;
use crate::common::Interner;
use crate::Printer;
use log::debug;
use std::collections::HashMap;
use crate::context::ExecContext;
use crate::graph::{ExecId, MonoId, ResolveId};
use dashmap::DashMap;

enum EvalResult {
    Value(Value),
    Return(Value),
}

/// Evaluate a binary op on two operands of the same numeric type.
/// Comparison and logic results are 1/0 in the operand type.
fn eval_binop<T>(op: BinOp, a: T, b: T) -> Result<T, ExecuteError>
where
    T: Copy
        + PartialOrd
        + Default
        + From<bool>
        + std::ops::Add<Output = T>
        + std::ops::Sub<Output = T>
        + std::ops::Mul<Output = T>
        + std::ops::Div<Output = T>,
{
    let zero = T::default();
    Ok(match op {
        BinOp::Add => a + b,
        BinOp::Sub => a - b,
        BinOp::Mul => a * b,
        BinOp::Div => {
            if b == zero {
                return Err(ExecuteError::DivisionByZero);
            }
            a / b
        }
        BinOp::Greater => T::from(a > b),
        BinOp::Less => T::from(a < b),
        BinOp::Equal => T::from(a == b),
        BinOp::And => T::from(a != zero && b != zero),
        BinOp::Or => T::from(a != zero || b != zero),
    })
}

struct Interpreter<'a> {
    values: HashMap<VarId, Value>,
    mono_registry: &'a DashMap<MonoId, MonoFuncData>,
    printer: &'a dyn Printer,
    interner: &'a Interner,
    args: Option<Vec<Value>>,
}

impl<'a> Interpreter<'a> {
    fn new(mono_registry: &'a DashMap<MonoId, MonoFuncData>, printer: &'a dyn Printer, interner: &'a Interner) -> Self {
        Interpreter {
            values: HashMap::new(),
            mono_registry,
            printer,
            interner,
            args: None,
        }
    }

    fn eval(&mut self, expr: &MExpr) -> Result<EvalResult, ExecuteError> {
        match expr {
            MExpr::Number(v) => Ok(EvalResult::Value(*v)),
            MExpr::VarRef(var_id) => {
                let val = *self.values.get(var_id)
                    .expect("resolver guarantees variables are assigned before use");
                Ok(EvalResult::Value(val))
            }
            MExpr::BinaryOp { op, left, right } => {
                let left_val = self.eval_value(left)?;
                let right_val = self.eval_value(right)?;

                let result = match (left_val, right_val) {
                    (Value::I32(a), Value::I32(b)) => Value::I32(eval_binop(*op, a, b)?),
                    (Value::I64(a), Value::I64(b)) => Value::I64(eval_binop(*op, a, b)?),
                    _ => unreachable!("type checker guarantees binary op operands have the same type"),
                };
                Ok(EvalResult::Value(result))
            }
            MExpr::Let { var, value } => {
                let val = self.eval_value(value)?;
                self.values.insert(*var, val);
                Ok(EvalResult::Value(val))
            }
            MExpr::Set { var, value } => {
                let val = self.eval_value(value)?;
                self.values.insert(*var, val);
                Ok(EvalResult::Value(val))
            }
            MExpr::If {
                cond,
                then_branch,
                else_branch,
            } => {
                let cond_val = self.eval_value(cond)?;
                if cond_val.is_truthy() {
                    self.eval(then_branch)
                } else {
                    self.eval(else_branch)
                }
            }
            MExpr::Print(expr) => {
                let val = self.eval_value(expr)?;
                self.printer.print(&val.to_string());
                Ok(EvalResult::Value(val))
            }
            MExpr::Return(expr) => {
                let val = self.eval_value(expr)?;
                Ok(EvalResult::Return(val))
            }
            MExpr::Panic { source_location, frame, node } => {
                Err(ExecuteError::Panic { source_location: source_location.clone(), frame: *frame, node: *node })
            }
            MExpr::Call { func, args } => {
                let mut arg_vals = Vec::new();
                for arg in args {
                    arg_vals.push(self.eval_value(arg)?);
                }

                let func_data = self.mono_registry.get(func)
                    .ok_or_else(|| ExecuteError::Internal(
                        format!("Monomorphised function not found: {}::{} @ {}",
                            func.func_loc.path_str(self.interner), func.func_loc.name_str(self.interner), func.ty)))?;
                debug_assert_eq!(func_data.key, *func);
                debug_assert_eq!(arg_vals.len(), func_data.arity, "resolver checked call arity");
                let result = self.call_function(&func_data.ast, arg_vals)?;
                Ok(EvalResult::Value(result))
            }
            MExpr::Arg(n) => {
                let args = self.args.as_ref().ok_or(ExecuteError::ArgNotProvided(*n))?;
                let index = (*n as usize) - 1;
                let val = *args.get(index).ok_or(ExecuteError::ArgNotProvided(*n))?;
                Ok(EvalResult::Value(val))
            }
            MExpr::Sequence(exprs) => {
                let mut last_val = Value::I64(0);
                for expr in exprs {
                    match self.eval(expr)? {
                        EvalResult::Value(v) => last_val = v,
                        EvalResult::Return(v) => return Ok(EvalResult::Return(v)),
                    }
                }
                Ok(EvalResult::Value(last_val))
            }
        }
    }

    fn eval_value(&mut self, expr: &MExpr) -> Result<Value, ExecuteError> {
        match self.eval(expr)? {
            EvalResult::Value(v) => Ok(v),
            EvalResult::Return(v) => Ok(v),
        }
    }

    fn call_function(&mut self, func_ast: &MExpr, args: Vec<Value>) -> Result<Value, ExecuteError> {
        let saved_args = self.args.take();
        let saved_values = self.values.clone();

        self.args = Some(args);
        self.values.clear();

        let result = match self.eval(func_ast)? {
            EvalResult::Value(v) => v,
            EvalResult::Return(v) => v,
        };

        self.args = saved_args;
        self.values = saved_values;

        Ok(result)
    }
}

pub async fn execute(ctx: &ExecContext, path: ExecId) -> Result<(), ExecuteError> {
    // The backend flavor lands here (roadmap item 15): execute is the
    // sandbox's codegen-analog, so opt-level is consumed at this level and
    // never above it. No optimizer branches on it yet — this is the wiring
    // endpoint a real codegen would read.
    debug!("execute: starting for {:?} (opt-level {:?})", path, ctx.opt_level());
    let my_main_func = path.main_loc.clone();
    let reesolve_id = ResolveId { func_loc: my_main_func.clone() };
    debug!("execute: resolving {:?}", reesolve_id);
    // A compile error carries a structural locator; upgrade it to
    // `path:line:col` here, on the error path, via the span sidecar — the happy
    // path never demands one (plans/fast-mode.md).
    ctx.resolve_all(&[reesolve_id]).await.map_err(|e| ExecuteError::Compile(ctx.render_located(&e)))?;
    debug!("execute: resolved, now type checking and monomorphising");
    // The entry point is not called by anyone, so its type parameter is just
    // the default numeric type.
    let entry = MonoId { func_loc: my_main_func, ty: Ty::I64 };
    ctx.mono(entry).map_err(|e| ExecuteError::Compile(ctx.render_located(&e)))?;
    debug!("execute: monomorphised, now evaluating");
    let my_ast = ctx.mono_registry().get(&entry)
        .expect("entry instance was just monomorphised")
        .ast.clone();
    let mut interpreter = Interpreter::new(ctx.mono_registry(), ctx.printer(), ctx.interner());
    if let Err(e) = interpreter.eval(&my_ast) {
        // Upgrade a fired `panic` from its coarse file path to a span-accurate
        // `path:line:col` now that the wave has unwound — demanding the span
        // sidecar only on this error path keeps the happy path span-free
        // (plans/fast-mode.md).
        return Err(match e {
            ExecuteError::Panic { source_location, frame, node } => {
                let source_location = ctx.render_panic_location(&source_location, frame, node);
                ExecuteError::Panic { source_location, frame, node }
            }
            other => other,
        });
    }
    debug!("execute: completed successfully");
    Ok(())
}
