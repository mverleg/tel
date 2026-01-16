use crate::types::{Expr, FuncId, PreExpr, ResolveError, ScopeId, SymbolTable, VarId};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

struct Resolver {
    symbol_table: SymbolTable,
    scopes: Vec<Scope>,
    current_scope: ScopeId,
    next_scope_id: usize,
    funcs: HashMap<String, FuncId>,
    in_function: bool,
    base_path: PathBuf,
}

struct Scope {
    id: ScopeId,
    parent: Option<ScopeId>,
    vars: HashMap<String, VarId>,
}

impl Resolver {
    fn new(base_path: PathBuf) -> Self {
        let global_scope = Scope {
            id: ScopeId(0),
            parent: None,
            vars: HashMap::new(),
        };

        Resolver {
            symbol_table: SymbolTable::new(),
            scopes: vec![global_scope],
            current_scope: ScopeId(0),
            next_scope_id: 1,
            funcs: HashMap::new(),
            in_function: false,
            base_path,
        }
    }

    fn enter_scope(&mut self) -> ScopeId {
        let new_id = ScopeId(self.next_scope_id);
        self.next_scope_id += 1;

        let new_scope = Scope {
            id: new_id,
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

    fn declare_var(&mut self, name: String) -> VarId {
        let var_id = self.symbol_table.add_var(name.clone(), self.current_scope);
        let scope = &mut self.scopes[self.current_scope.0];
        scope.vars.insert(name, var_id);
        var_id
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
                let var_id = self.declare_var(name);
                Ok(Expr::Let {
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
            PreExpr::Call { func, arg1, arg2 } => {
                let func_id = self.funcs.get(&func)
                    .copied()
                    .ok_or_else(|| ResolveError::UndefinedFunction(func.clone()))?;
                let resolved_arg1 = Box::new(self.resolve_expr(*arg1)?);
                let resolved_arg2 = Box::new(self.resolve_expr(*arg2)?);
                Ok(Expr::Call {
                    func: func_id,
                    arg1: resolved_arg1,
                    arg2: resolved_arg2,
                })
            }
            PreExpr::Arg(n) => {
                if !self.in_function {
                    return Err(ResolveError::ArgOutsideFunction);
                }
                if n != 1 && n != 2 {
                    return Err(ResolveError::InvalidArgNumber(n));
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

        for import_path in imports {
            let full_path = self.base_path.join(&import_path);

            let source = crate::io::load_file(full_path.to_str().unwrap())
                .map_err(|_| ResolveError::UndefinedFunction(import_path.clone()))?;

            let imported_pre_ast = crate::parse::parse(&source)
                .map_err(|_| ResolveError::UndefinedFunction(import_path.clone()))?;

            let func_name = Path::new(&import_path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(&import_path)
                .to_string();

            let mut func_resolver = Resolver::new(full_path.parent().unwrap_or(Path::new(".")).to_path_buf());
            func_resolver.in_function = true;
            func_resolver.process_imports(&imported_pre_ast)?;

            let placeholder_id = FuncId(self.symbol_table.funcs.len() + func_resolver.symbol_table.funcs.len());
            func_resolver.funcs.insert(func_name.clone(), placeholder_id);

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

            let func_id = self.symbol_table.add_func(func_name.clone(), func_ast);
            self.funcs.insert(func_name, func_id);
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

    fn remap_func_ids(expr: &mut Expr, offset: usize) {
        match expr {
            Expr::Call { func, arg1, arg2 } => {
                func.0 += offset;
                Self::remap_func_ids(arg1, offset);
                Self::remap_func_ids(arg2, offset);
            }
            Expr::BinaryOp { left, right, .. } => {
                Self::remap_func_ids(left, offset);
                Self::remap_func_ids(right, offset);
            }
            Expr::Let { value, .. } => {
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
                    if !matches!(expr, PreExpr::Import(_)) {
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
            PreExpr::Import(_) => {
                Ok(Expr::Number(0))
            }
            other => self.resolve_expr(other.clone()),
        }
    }
}

pub fn resolve(pre_ast: PreExpr, base_path: &str) -> Result<(Expr, SymbolTable), ResolveError> {
    let path = Path::new(base_path);
    let dir = path.parent().unwrap_or(Path::new("."));

    let mut resolver = Resolver::new(dir.to_path_buf());
    resolver.process_imports(&pre_ast)?;
    let ast = resolver.resolve_body(&pre_ast)?;
    Ok((ast, resolver.symbol_table))
}
