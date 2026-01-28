use crate::types::SymbolTable;
use crate::types::Expr;
use crate::types::ExecuteError;
use crate::types::BinOp;
use crate::types::VarId;
use std::collections::HashMap;
use crate::common::Name;
use crate::context::Context;
use crate::graph::{ExecId, ResolveId};

enum EvalResult {
    Value(i64),
    Return(i64),
}

struct Interpreter<'a> {
    values: HashMap<VarId, i64>,
    symbols: &'a SymbolTable,
    args: Option<Vec<i64>>,
}

impl<'a> Interpreter<'a> {
    fn new(symbols: &'a SymbolTable) -> Self {
        Interpreter {
            values: HashMap::new(),
            symbols,
            args: None,
        }
    }

    fn eval(&mut self, expr: &Expr) -> Result<EvalResult, ExecuteError> {
        match expr {
            Expr::Number(n) => Ok(EvalResult::Value(*n)),
            Expr::VarRef(var_id) => {
                Ok(EvalResult::Value(*self.values.get(var_id).unwrap_or(&0)))
            }
            Expr::BinaryOp { op, left, right } => {
                let left_val = self.eval_value(left)?;
                let right_val = self.eval_value(right)?;

                let result = match op {
                    BinOp::Add => left_val + right_val,
                    BinOp::Sub => left_val - right_val,
                    BinOp::Mul => left_val * right_val,
                    BinOp::Div => {
                        if right_val == 0 {
                            return Err(ExecuteError::DivisionByZero);
                        }
                        left_val / right_val
                    }
                    BinOp::Greater => if left_val > right_val { 1 } else { 0 },
                    BinOp::Less => if left_val < right_val { 1 } else { 0 },
                    BinOp::Equal => if left_val == right_val { 1 } else { 0 },
                    BinOp::And => if left_val != 0 && right_val != 0 { 1 } else { 0 },
                    BinOp::Or => if left_val != 0 || right_val != 0 { 1 } else { 0 },
                };
                Ok(EvalResult::Value(result))
            }
            Expr::Let { var, value } => {
                let val = self.eval_value(value)?;
                self.values.insert(*var, val);
                Ok(EvalResult::Value(val))
            }
            Expr::Set { var, value } => {
                let val = self.eval_value(value)?;
                self.values.insert(*var, val);
                Ok(EvalResult::Value(val))
            }
            Expr::If {
                cond,
                then_branch,
                else_branch,
            } => {
                let cond_val = self.eval_value(cond)?;
                if cond_val != 0 {
                    self.eval(then_branch)
                } else {
                    self.eval(else_branch)
                }
            }
            Expr::Print(expr) => {
                let val = self.eval_value(expr)?;
                println!("{}", val);
                Ok(EvalResult::Value(val))
            }
            Expr::Return(expr) => {
                let val = self.eval_value(expr)?;
                Ok(EvalResult::Return(val))
            }
            Expr::Panic { source_location } => {
                Err(ExecuteError::Panic { source_location: source_location.clone() })
            }
            Expr::Unreachable { .. } => {
                panic!("Unreachable code was reached during execution - this should have been caught during resolution")
            }
            Expr::Call { func, args } => {
                let mut arg_vals = Vec::new();
                for arg in args {
                    arg_vals.push(self.eval_value(arg)?);
                }

                let func_info = &self.symbols.funcs[func.0];
                let result = self.call_function(&func_info.ast, arg_vals)?;
                Ok(EvalResult::Value(result))
            }
            Expr::Arg(n) => {
                let args = self.args.as_ref().ok_or(ExecuteError::ArgNotProvided(*n))?;
                let index = (*n as usize) - 1;
                let val = *args.get(index).ok_or(ExecuteError::ArgNotProvided(*n))?;
                Ok(EvalResult::Value(val))
            }
            Expr::Sequence(exprs) => {
                let mut last_val = 0;
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

    fn eval_value(&mut self, expr: &Expr) -> Result<i64, ExecuteError> {
        match self.eval(expr)? {
            EvalResult::Value(v) => Ok(v),
            EvalResult::Return(v) => Ok(v),
        }
    }

    fn call_function(&mut self, func_ast: &Expr, args: Vec<i64>) -> Result<i64, ExecuteError> {
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

pub fn execute_internal(ast: &Expr, symbols: &SymbolTable) -> Result<(), ExecuteError> {
    let mut interpreter = Interpreter::new(symbols);
    interpreter.eval(ast)?;
    Ok(())
}

pub fn execute(ctx: &Context, path: &str) -> Result<(), ExecuteError> {
    let main = Name::of("main");
    let reesolve_id = ResolveId { func_name: main.clone() };
    // let (my_ast, my_symbols) = crate::resolve::resolve(path)?;
    let (my_ast, my_symbols) = ctx.resolve(reesolve_id)?;
    let exec_id = ExecId { main_func: main };
    execute_internal(&my_ast, &my_symbols)
}

