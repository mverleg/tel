use crate::qcompiler2::Context;
use crate::types::{Expr, FuncId, PreExpr, ResolveError, ScopeId, SymbolTable, VarId};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

struct Resolver {
    symbol_table: SymbolTable,
    scopes: Vec<Scope>,
    current_scope: ScopeId,
    next_scope_id: usize,
    funcs: HashMap<String, FuncId>,
    func_arities: HashMap<FuncId, usize>,
    in_function: bool,
    base_path: PathBuf,
    ctx: Context,
}

struct Scope {
    parent: Option<ScopeId>,
    vars: HashMap<String, VarId>,
}

impl Resolver {
    fn new(base_path: PathBuf, a_ctx: Context) -> Self {
        let global_scope = Scope {
            parent: None,
            vars: HashMap::new(),
        };

        Resolver {
            symbol_table: SymbolTable::new(),
            scopes: vec![global_scope],
            current_scope: ScopeId(0),
            next_scope_id: 1,
            funcs: HashMap::new(),
            func_arities: HashMap::new(),
            in_function: false,
            base_path,
            ctx: a_ctx,
        }
    }

    fn calculate_arity(expr: &PreExpr, func_name: &str) -> Result<usize, ResolveError> {
        let mut max_arg = 0u8;
        let mut arg_numbers = std::collections::HashSet::new();

        Self::collect_arg_numbers(expr, &mut arg_numbers, &mut max_arg);

        if max_arg == 0 {
            return Ok(0);
        }

        for i in 1..=max_arg {
            if !arg_numbers.contains(&i) {
                return Err(ResolveError::ArityGap {
                    func_name: func_name.to_string(),
                    max_arg: max_arg as usize,
                });
            }
        }

        Ok(max_arg as usize)
    }

    fn collect_arg_numbers(expr: &PreExpr, arg_numbers: &mut std::collections::HashSet<u8>, max_arg: &mut u8) {
        match expr {
            PreExpr::Arg(n) => {
                arg_numbers.insert(*n);
                if *n > *max_arg {
                    *max_arg = *n;
                }
            }
            PreExpr::BinaryOp { left, right, .. } => {
                Self::collect_arg_numbers(left, arg_numbers, max_arg);
                Self::collect_arg_numbers(right, arg_numbers, max_arg);
            }
            PreExpr::Let { value, .. } | PreExpr::Set { value, .. } => {
                Self::collect_arg_numbers(value, arg_numbers, max_arg);
            }
            PreExpr::If { cond, then_branch, else_branch } => {
                Self::collect_arg_numbers(cond, arg_numbers, max_arg);
                Self::collect_arg_numbers(then_branch, arg_numbers, max_arg);
                Self::collect_arg_numbers(else_branch, arg_numbers, max_arg);
            }
            PreExpr::Print(e) | PreExpr::Return(e) => {
                Self::collect_arg_numbers(e, arg_numbers, max_arg);
            }
            PreExpr::Call { args, .. } => {
                for arg in args {
                    Self::collect_arg_numbers(arg, arg_numbers, max_arg);
                }
            }
            PreExpr::Sequence(exprs) => {
                for expr in exprs {
                    Self::collect_arg_numbers(expr, arg_numbers, max_arg);
                }
            }
            PreExpr::Number(_) | PreExpr::Ident(_) | PreExpr::Import(_) | PreExpr::FunctionDef { .. } => {}
        }
    }

    fn enter_scope(&mut self) -> ScopeId {
        let new_id = ScopeId(self.next_scope_id);
        self.next_scope_id += 1;

        let new_scope = Scope {
            parent: Some(self.current_scope),
            vars: HashMap::new(),
        };

        self.scopes.push(new_scope);
        self.current_scope = new_id;
        new_id
    }

    fn exit_scope(&mut self) {
        let scope = &self.scopes[self.current_scope.0];
        if let Some(parent) = scope.parent {
            self.current_scope = parent;
        }
    }

    fn declare_var(&mut self, name: String) -> Result<VarId, ResolveError> {
        if self.resolve_var(&name).is_ok() {
            return Err(ResolveError::VariableAlreadyDefined(name));
        }
        let var_id = self.symbol_table.add_var(name.clone(), self.current_scope);
        let scope = &mut self.scopes[self.current_scope.0];
        scope.vars.insert(name, var_id);
        Ok(var_id)
    }

    fn resolve_var(&self, name: &str) -> Result<VarId, ResolveError> {
        let mut current = Some(self.current_scope);

        while let Some(scope_id) = current {
            let scope = &self.scopes[scope_id.0];
            if let Some(&var_id) = scope.vars.get(name) {
                return Ok(var_id);
            }
            current = scope.parent;
        }

        Err(ResolveError::UndefinedVariable(name.to_string()))
    }

    fn resolve_expr(&mut self, pre_expr: PreExpr) -> Result<Expr, ResolveError> {
        match pre_expr {
            PreExpr::Number(n) => Ok(Expr::Number(n)),
            PreExpr::Ident(name) => {
                let var_id = self.resolve_var(&name)?;
                Ok(Expr::VarRef(var_id))
            }
            PreExpr::BinaryOp { op, left, right } => {
                let resolved_left = Box::new(self.resolve_expr(*left)?);
                let resolved_right = Box::new(self.resolve_expr(*right)?);
                Ok(Expr::BinaryOp {
                    op,
                    left: resolved_left,
                    right: resolved_right,
                })
            }
            PreExpr::Let { name, value } => {
                let resolved_value = Box::new(self.resolve_expr(*value)?);
                let var_id = self.declare_var(name)?;
                Ok(Expr::Let {
                    var: var_id,
                    value: resolved_value,
                })
            }
            PreExpr::Set { name, value } => {
                let resolved_value = Box::new(self.resolve_expr(*value)?);
                let var_id = self.resolve_var(&name)?;
                Ok(Expr::Set {
                    var: var_id,
                    value: resolved_value,
                })
            }
            PreExpr::If {
                cond,
                then_branch,
                else_branch,
            } => {
                let resolved_cond = Box::new(self.resolve_expr(*cond)?);

                self.enter_scope();
                let resolved_then = Box::new(self.resolve_expr(*then_branch)?);
                self.exit_scope();

                self.enter_scope();
                let resolved_else = Box::new(self.resolve_expr(*else_branch)?);
                self.exit_scope();

                Ok(Expr::If {
                    cond: resolved_cond,
                    then_branch: resolved_then,
                    else_branch: resolved_else,
                })
            }
            PreExpr::Print(expr) => {
                let resolved_expr = Box::new(self.resolve_expr(*expr)?);
                Ok(Expr::Print(resolved_expr))
            }
            PreExpr::Return(expr) => {
                let resolved_expr = Box::new(self.resolve_expr(*expr)?);
                Ok(Expr::Return(resolved_expr))
            }
            PreExpr::Import(_) => {
                Err(ResolveError::ImportNotAtTop)
            }
            PreExpr::FunctionDef { .. } => {
                Err(ResolveError::FunctionDefNotAfterImports)
            }
            PreExpr::Call { func, args } => {
                let func_id = self.funcs.get(&func)
                    .copied()
                    .ok_or_else(|| ResolveError::UndefinedFunction(func.clone()))?;

                let expected_arity = self.func_arities.get(&func_id)
                    .copied()
                    .unwrap_or_else(|| self.symbol_table.funcs[func_id.0].arity);
                let got_arity = args.len();

                if expected_arity != got_arity {
                    return Err(ResolveError::ArityMismatch {
                        func_name: func.clone(),
                        expected: expected_arity,
                        got: got_arity,
                    });
                }

                let mut resolved_args = Vec::new();
                for arg in args {
                    resolved_args.push(Box::new(self.resolve_expr(*arg)?));
                }
                Ok(Expr::Call {
                    func: func_id,
                    args: resolved_args,
                })
            }
            PreExpr::Arg(n) => {
                if !self.in_function {
                    return Err(ResolveError::ArgOutsideFunction);
                }
                Ok(Expr::Arg(n))
            }
            PreExpr::Sequence(exprs) => {
                let mut resolved_exprs = Vec::new();
                for expr in exprs {
                    resolved_exprs.push(self.resolve_expr(expr)?);
                }
                Ok(Expr::Sequence(resolved_exprs))
            }
        }
    }

    fn process_imports(&mut self, pre_ast: &PreExpr) -> Result<(), ResolveError> {
        let imports = self.extract_imports(pre_ast)?;

        for import_name in imports {
            if import_name.contains('.') {
                return Err(ResolveError::InvalidImportPath(import_name.clone()));
            }
            let full_path = self.base_path.join(format!("{}.telsb", import_name));

            let source = crate::io::load_file(full_path.to_str().unwrap(), &self.ctx)
                .map_err(|_| ResolveError::UndefinedFunction(import_name.clone()))?;

            let imported_pre_ast = crate::parse::parse(&source, full_path.to_str().unwrap(), &self.ctx)
                .map_err(|_| ResolveError::UndefinedFunction(import_name.clone()))?;

            let arity = Self::calculate_arity(&imported_pre_ast, &import_name)?;

            let mut func_resolver = Resolver::new(full_path.parent().unwrap_or(Path::new(".")).to_path_buf(), self.ctx.clone());
            func_resolver.in_function = true;
            func_resolver.process_imports(&imported_pre_ast)?;

            let placeholder_id = FuncId(self.symbol_table.funcs.len() + func_resolver.symbol_table.funcs.len());
            func_resolver.funcs.insert(import_name.clone(), placeholder_id);
            func_resolver.func_arities.insert(placeholder_id, arity);

            let mut func_ast = func_resolver.resolve_body(&imported_pre_ast)?;

            let offset = self.symbol_table.funcs.len();

            for mut func_info in func_resolver.symbol_table.funcs {
                Self::remap_func_ids(&mut func_info.ast, offset);
                self.symbol_table.funcs.push(func_info);
            }

            Self::remap_func_ids(&mut func_ast, offset);

            for (name, old_id) in func_resolver.funcs {
                let new_id = FuncId(old_id.0 + offset);
                self.funcs.insert(name, new_id);
            }

            for (old_id, arity_value) in func_resolver.func_arities {
                let new_id = FuncId(old_id.0 + offset);
                self.func_arities.insert(new_id, arity_value);
            }

            let func_id = self.symbol_table.add_func(import_name.clone(), func_ast, arity);
            self.funcs.insert(import_name, func_id);
            self.func_arities.insert(func_id, arity);
        }

        Ok(())
    }

    fn extract_imports(&self, pre_expr: &PreExpr) -> Result<Vec<String>, ResolveError> {
        let mut imports = Vec::new();

        match pre_expr {
            PreExpr::Sequence(exprs) => {
                let mut seen_non_import = false;
                for expr in exprs {
                    match expr {
                        PreExpr::Import(path) => {
                            if seen_non_import {
                                return Err(ResolveError::ImportNotAtTop);
                            }
                            imports.push(path.clone());
                        }
                        _ => {
                            seen_non_import = true;
                        }
                    }
                }
            }
            PreExpr::Import(path) => {
                imports.push(path.clone());
            }
            _ => {}
        }

        Ok(imports)
    }

    fn extract_function_defs(&self, pre_expr: &PreExpr) -> Result<Vec<(String, PreExpr)>, ResolveError> {
        let mut function_defs = Vec::new();

        match pre_expr {
            PreExpr::Sequence(exprs) => {
                let mut seen_function_def = false;
                let mut seen_other = false;

                for expr in exprs {
                    match expr {
                        PreExpr::Import(_) => {
                            if seen_function_def || seen_other {
                                return Err(ResolveError::ImportNotAtTop);
                            }
                        }
                        PreExpr::FunctionDef { name, body } => {
                            if seen_other {
                                return Err(ResolveError::FunctionDefNotAfterImports);
                            }
                            seen_function_def = true;
                            function_defs.push((name.clone(), (**body).clone()));
                        }
                        _ => {
                            seen_other = true;
                        }
                    }
                }
            }
            PreExpr::FunctionDef { name, body } => {
                function_defs.push((name.clone(), (**body).clone()));
            }
            _ => {}
        }

        Ok(function_defs)
    }

    fn process_local_functions(&mut self, pre_ast: &PreExpr) -> Result<(), ResolveError> {
        let function_defs = self.extract_function_defs(pre_ast)?;

        for (func_name, func_body) in function_defs {
            if self.funcs.contains_key(&func_name) {
                return Err(ResolveError::FunctionAlreadyDefined(func_name));
            }

            let arity = Self::calculate_arity(&func_body, &func_name)?;

            let saved_in_function = self.in_function;
            self.in_function = true;

            let resolved_body = self.resolve_expr(func_body)?;

            self.in_function = saved_in_function;

            let func_id = self.symbol_table.add_func(func_name.clone(), resolved_body, arity);
            self.funcs.insert(func_name, func_id);
            self.func_arities.insert(func_id, arity);
        }

        Ok(())
    }

    fn remap_func_ids(expr: &mut Expr, offset: usize) {
        match expr {
            Expr::Call { func, args } => {
                func.0 += offset;
                for arg in args {
                    Self::remap_func_ids(arg, offset);
                }
            }
            Expr::BinaryOp { left, right, .. } => {
                Self::remap_func_ids(left, offset);
                Self::remap_func_ids(right, offset);
            }
            Expr::Let { value, .. } | Expr::Set { value, .. } => {
                Self::remap_func_ids(value, offset);
            }
            Expr::If { cond, then_branch, else_branch } => {
                Self::remap_func_ids(cond, offset);
                Self::remap_func_ids(then_branch, offset);
                Self::remap_func_ids(else_branch, offset);
            }
            Expr::Print(e) | Expr::Return(e) => {
                Self::remap_func_ids(e, offset);
            }
            Expr::Sequence(exprs) => {
                for expr in exprs {
                    Self::remap_func_ids(expr, offset);
                }
            }
            Expr::Number(_) | Expr::VarRef(_) | Expr::Arg(_) => {}
        }
    }

    fn resolve_body(&mut self, pre_ast: &PreExpr) -> Result<Expr, ResolveError> {
        match pre_ast {
            PreExpr::Sequence(exprs) => {
                let mut resolved_exprs = Vec::new();
                for expr in exprs {
                    if !matches!(expr, PreExpr::Import(_) | PreExpr::FunctionDef { .. }) {
                        resolved_exprs.push(self.resolve_expr(expr.clone())?);
                    }
                }
                if resolved_exprs.is_empty() {
                    Ok(Expr::Number(0))
                } else if resolved_exprs.len() == 1 {
                    Ok(resolved_exprs.into_iter().next().unwrap())
                } else {
                    Ok(Expr::Sequence(resolved_exprs))
                }
            }
            PreExpr::Import(_) | PreExpr::FunctionDef { .. } => {
                Ok(Expr::Number(0))
            }
            other => self.resolve_expr(other.clone()),
        }
    }
}

pub fn resolve(pre_ast: PreExpr, base_path: &str, a_ctx: &Context) -> Result<(Expr, SymbolTable), ResolveError> {
    let path = Path::new(base_path);
    let dir = path.parent().unwrap_or(Path::new("."));

    let my_func_name = path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("main");

    a_ctx.in_resolve(my_func_name, |ctx| {
        let mut resolver = Resolver::new(dir.to_path_buf(), ctx);
        resolver.process_imports(&pre_ast)?;
        resolver.process_local_functions(&pre_ast)?;
        let ast = resolver.resolve_body(&pre_ast)?;
        Ok((ast, resolver.symbol_table))
    })
}
