use crate::common::{Name, FQ};
use crate::context::ResolveContext;
use crate::graph::ResolveId;
use crate::types::{Expr, ExprKind, FuncId, Loc, Located, PreExpr, PreExprKind, ResolveError, ScopeId, SrcLoc, SymbolTable, VarId};
use log::debug;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::path::PathBuf;

struct Resolver<'a> {
    ctx: &'a ResolveContext<'a>,
    symbol_table: SymbolTable,  // Now only for vars
    scopes: Vec<Scope>,
    current_scope: ScopeId,
    next_scope_id: usize,
    funcs: HashMap<String, FuncId>,  // Name -> FQ-based FuncId (what's callable in this scope)
    in_function: bool,
    current_file: PathBuf,
    current_context: Name,
    /// Functions this resolution registered (file-function + locals), in
    /// registration order — returned so the answer can carry the definitions
    /// for replay on a cache hit.
    registered: Vec<FQ>,
}

struct Scope {
    parent: Option<ScopeId>,
    vars: HashMap<String, VarId>,
}

impl<'a> Resolver<'a> {
    fn new(ctx: &'a ResolveContext, current_file: PathBuf, context: Name) -> Self {
        let global_scope = Scope {
            parent: None,
            vars: HashMap::new(),
        };

        Resolver {
            ctx,
            symbol_table: SymbolTable::new(),
            scopes: vec![global_scope],
            current_scope: ScopeId(0),
            next_scope_id: 1,
            funcs: HashMap::new(),
            in_function: false,
            current_file,
            current_context: context,
            registered: Vec::new(),
        }
    }

    fn context_str(&self) -> String {
        self.current_context.resolve(self.ctx.interner()).to_string()
    }

    /// The location string for diagnostics that point at this file
    /// (`panic`/`unreachable`). File-level granularity — the sandbox tracks no
    /// line numbers.
    fn source_location_str(&self) -> String {
        self.current_file.to_string_lossy().to_string()
    }

    /// Pin a resolve error to the node it came from, so the driver can upgrade
    /// it to `path:line:col` via the span sidecar. The locator is structural,
    /// so a sibling edit that shifts byte offsets does not disturb it (or the
    /// cached, unchanged function it points into).
    fn located(&self, error: ResolveError, loc: Loc) -> Located<ResolveError> {
        Located::at(error, SrcLoc::new(self.source_location_str(), loc))
    }

    fn calculate_arity(expr: &PreExpr, func_name: &str, context: &str) -> Result<usize, ResolveError> {
        let mut max_arg = 0u8;
        let mut arg_numbers = std::collections::HashSet::new();

        Self::collect_arg_numbers(expr, &mut arg_numbers, &mut max_arg);

        if max_arg == 0 {
            return Ok(0);
        }

        for i in 1..=max_arg {
            if !arg_numbers.contains(&i) {
                return Err(ResolveError::ArityGap {
                    context: context.to_string(),
                    func_name: func_name.to_string(),
                    max_arg: max_arg as usize,
                });
            }
        }

        Ok(max_arg as usize)
    }

    fn collect_arg_numbers(expr: &PreExpr, arg_numbers: &mut std::collections::HashSet<u8>, max_arg: &mut u8) {
        match &expr.kind {
            PreExprKind::Arg(n) => {
                arg_numbers.insert(*n);
                if *n > *max_arg {
                    *max_arg = *n;
                }
            }
            PreExprKind::BinaryOp { left, right, .. } => {
                Self::collect_arg_numbers(left, arg_numbers, max_arg);
                Self::collect_arg_numbers(right, arg_numbers, max_arg);
            }
            PreExprKind::Let { value, .. } | PreExprKind::Set { value, .. } => {
                Self::collect_arg_numbers(value, arg_numbers, max_arg);
            }
            PreExprKind::If { cond, then_branch, else_branch } => {
                Self::collect_arg_numbers(cond, arg_numbers, max_arg);
                Self::collect_arg_numbers(then_branch, arg_numbers, max_arg);
                Self::collect_arg_numbers(else_branch, arg_numbers, max_arg);
            }
            PreExprKind::Print(e) | PreExprKind::Return(e) => {
                Self::collect_arg_numbers(e, arg_numbers, max_arg);
            }
            PreExprKind::Panic { .. } | PreExprKind::Unreachable { .. } => {}
            PreExprKind::Call { args, .. } => {
                for arg in args {
                    Self::collect_arg_numbers(arg, arg_numbers, max_arg);
                }
            }
            PreExprKind::Sequence(exprs) => {
                for expr in exprs {
                    Self::collect_arg_numbers(expr, arg_numbers, max_arg);
                }
            }
            PreExprKind::Number { .. } | PreExprKind::Ident(_) | PreExprKind::Import(_) | PreExprKind::FunctionDef { .. } => {}
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
            return Err(ResolveError::VariableAlreadyDefined(self.context_str(), name));
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

        Err(ResolveError::UndefinedVariable(self.context_str(), name.to_string()))
    }

    fn resolve_expr(&mut self, pre_expr: PreExpr) -> Result<Expr, Located<ResolveError>> {
        debug!("resolve_expr: {:?} (in_function={})", pre_expr, self.in_function);
        let loc = pre_expr.loc;
        let kind = match pre_expr.kind {
            PreExprKind::Number { value, ty } => ExprKind::Number { value, ty },
            PreExprKind::Ident(name) => {
                let var_id = self.resolve_var(&name).map_err(|e| self.located(e, loc))?;
                ExprKind::VarRef(var_id)
            }
            PreExprKind::BinaryOp { op, left, right } => {
                let resolved_left = Box::new(self.resolve_expr(*left)?);
                let resolved_right = Box::new(self.resolve_expr(*right)?);
                ExprKind::BinaryOp {
                    op,
                    left: resolved_left,
                    right: resolved_right,
                }
            }
            PreExprKind::Let { name, value } => {
                let resolved_value = Box::new(self.resolve_expr(*value)?);
                let var_id = self.declare_var(name).map_err(|e| self.located(e, loc))?;
                ExprKind::Let {
                    var: var_id,
                    value: resolved_value,
                }
            }
            PreExprKind::Set { name, value } => {
                let resolved_value = Box::new(self.resolve_expr(*value)?);
                let var_id = self.resolve_var(&name).map_err(|e| self.located(e, loc))?;
                ExprKind::Set {
                    var: var_id,
                    value: resolved_value,
                }
            }
            PreExprKind::If {
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

                ExprKind::If {
                    cond: resolved_cond,
                    then_branch: resolved_then,
                    else_branch: resolved_else,
                }
            }
            PreExprKind::Print(expr) => {
                let resolved_expr = Box::new(self.resolve_expr(*expr)?);
                ExprKind::Print(resolved_expr)
            }
            PreExprKind::Return(expr) => {
                let resolved_expr = Box::new(self.resolve_expr(*expr)?);
                ExprKind::Return(resolved_expr)
            }
            // The location is attached here, not at parse: the parse answer is
            // shared across identical files at different paths, so it must be
            // path-free; this step's key pins the FQ, so embedding the path in
            // the *resolve* answer is sound.
            PreExprKind::Panic => {
                ExprKind::Panic { source_location: self.source_location_str() }
            }
            PreExprKind::Unreachable => {
                return Err(self.located(ResolveError::UnreachableCode { context: self.context_str(), source_location: self.source_location_str(), frame: loc.frame, node: loc.node }, loc));
            }
            PreExprKind::Import(_) => {
                return Err(self.located(ResolveError::ImportNotAtTop(self.context_str()), loc));
            }
            PreExprKind::FunctionDef { .. } => {
                return Err(self.located(ResolveError::FunctionDefNotAfterImports(self.context_str()), loc));
            }
            PreExprKind::Call { func, args } => {
                let func_id = self.funcs.get(&func)
                    .cloned()
                    .ok_or_else(|| self.located(ResolveError::UndefinedFunction(self.context_str(), func.clone()), loc))?;

                let expected_arity = self.ctx.func_registry()
                    .get(&func_id.0)
                    .map(|f| f.arity)
                    .ok_or_else(|| self.located(ResolveError::UndefinedFunction(self.context_str(), func.clone()), loc))?;
                let got_arity = args.len();

                if expected_arity != got_arity {
                    return Err(self.located(ResolveError::ArityMismatch {
                        context: self.context_str(),
                        func_name: func.clone(),
                        expected: expected_arity,
                        got: got_arity,
                    }, loc));
                }

                let mut resolved_args = Vec::new();
                for arg in args {
                    resolved_args.push(Box::new(self.resolve_expr(*arg)?));
                }
                ExprKind::Call {
                    func: func_id,
                    args: resolved_args,
                }
            }
            PreExprKind::Arg(n) => {
                debug!("Processing Arg({}) in context {:?}, in_function={}", n, self.current_context, self.in_function);
                if !self.in_function {
                    debug!("ERROR: Arg used outside function - in_function={}", self.in_function);
                    return Err(self.located(ResolveError::ArgOutsideFunction(self.context_str()), loc));
                }
                debug!("Arg({}) resolved successfully", n);
                ExprKind::Arg(n)
            }
            PreExprKind::Sequence(exprs) => {
                let mut resolved_exprs = Vec::new();
                for expr in exprs {
                    resolved_exprs.push(self.resolve_expr(expr)?);
                }
                ExprKind::Sequence(resolved_exprs)
            }
        };
        Ok(Expr::new(loc, kind))
    }

    fn extract_function_defs(&self, pre_expr: &PreExpr) -> Result<Vec<(String, PreExpr)>, ResolveError> {
        let mut function_defs = Vec::new();

        match &pre_expr.kind {
            PreExprKind::Sequence(exprs) => {
                let mut seen_function_def = false;
                let mut seen_other = false;

                for expr in exprs {
                    match &expr.kind {
                        PreExprKind::Import(_) => {
                            if seen_function_def || seen_other {
                                return Err(ResolveError::ImportNotAtTop(self.context_str()));
                            }
                        }
                        PreExprKind::FunctionDef { name, body } => {
                            if seen_other {
                                return Err(ResolveError::FunctionDefNotAfterImports(self.context_str()));
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
            PreExprKind::FunctionDef { name, body } => {
                function_defs.push((name.clone(), (**body).clone()));
            }
            _ => {}
        }

        Ok(function_defs)
    }

    fn process_local_functions(&mut self, pre_ast: &PreExpr) -> Result<(), Located<ResolveError>> {
        let function_defs = self.extract_function_defs(pre_ast)?;
        debug!("process_local_functions: found {} function definitions", function_defs.len());

        for (func_name, func_body) in function_defs {
            debug!("Processing local function: {}", func_name);
            if self.funcs.contains_key(&func_name) {
                return Err(ResolveError::FunctionAlreadyDefined(self.context_str(), func_name).into());
            }

            let arity = Self::calculate_arity(&func_body, &func_name, &self.context_str())?;
            debug!("Function {} has arity {}", func_name, arity);

            let saved_in_function = self.in_function;
            debug!("Setting in_function=true for local function {} (was {})", func_name, saved_in_function);
            self.in_function = true;

            let resolved_body = self.resolve_expr(func_body)?;

            self.in_function = saved_in_function;
            debug!("Restored in_function={} after local function {}", saved_in_function, func_name);

            let func_loc = FQ::intern(self.ctx.interner(), &self.current_file.to_string_lossy(), &func_name);
            let func_id = FuncId(func_loc);

            // Register in global registry
            use crate::types::FuncData;
            self.ctx.func_registry().insert(func_loc, FuncData {
                loc: func_loc,
                arity,
                ast: resolved_body,
            });
            self.registered.push(func_loc);

            self.funcs.insert(func_name, func_id);
        }

        Ok(())
    }


    fn resolve_body(&mut self, pre_ast: &PreExpr) -> Result<Expr, Located<ResolveError>> {
        debug!("resolve_body: in_function={}, context={:?}", self.in_function, self.current_context);
        let loc = pre_ast.loc;
        match &pre_ast.kind {
            PreExprKind::Sequence(exprs) => {
                let mut resolved_exprs = Vec::new();
                for expr in exprs {
                    if !matches!(expr.kind, PreExprKind::Import(_) | PreExprKind::FunctionDef { .. }) {
                        resolved_exprs.push(self.resolve_expr(expr.clone())?);
                    }
                }
                if resolved_exprs.is_empty() {
                    Ok(Expr::new(loc, ExprKind::Number { value: 0, ty: None }))
                } else if resolved_exprs.len() == 1 {
                    Ok(resolved_exprs.into_iter().next().unwrap())
                } else {
                    Ok(Expr::new(loc, ExprKind::Sequence(resolved_exprs)))
                }
            }
            PreExprKind::Import(_) | PreExprKind::FunctionDef { .. } => {
                Ok(Expr::new(loc, ExprKind::Number { value: 0, ty: None }))
            }
            other => self.resolve_expr(PreExpr::new(loc, other.clone())),
        }
    }
}

/// Extract the import names declared at the top of a parsed file, validating
/// their placement and form.
///
/// A pure function of the parse answer: this is how the *dependency set* of a
/// resolve step is re-derived fresh on every demand (never read from recorded
/// edges, which may reflect other content), so the step's content key can be
/// built before the step runs.
pub(crate) fn extract_import_names(pre_expr: &PreExpr, context: &str) -> Result<Vec<String>, ResolveError> {
    let mut imports = Vec::new();

    match &pre_expr.kind {
        PreExprKind::Sequence(exprs) => {
            let mut seen_non_import = false;
            for expr in exprs {
                match &expr.kind {
                    PreExprKind::Import(path) => {
                        if seen_non_import {
                            return Err(ResolveError::ImportNotAtTop(context.to_string()));
                        }
                        imports.push(path.clone());
                    }
                    _ => {
                        seen_non_import = true;
                    }
                }
            }
        }
        PreExprKind::Import(path) => {
            imports.push(path.clone());
        }
        _ => {}
    }

    let mut seen_paths = HashSet::with_capacity(imports.len());
    let mut seen_names = HashMap::with_capacity(imports.len());
    for import_path in &imports {
        check_import_path(import_path, context)?;
        if !seen_paths.insert(import_path.as_str()) {
            return Err(ResolveError::DuplicateImport(context.to_string(), import_path.clone()));
        }
        // Two distinct files whose last segment matches would both want the
        // same callable name. Rejected rather than ordered: picking a winner
        // is the shadowing rule this design does not have.
        let name = import_callable_name(import_path);
        if let Some(previous) = seen_names.insert(name, import_path.as_str()) {
            return Err(ResolveError::ImportNameCollision(
                context.to_string(), name.to_string(), previous.to_string(), import_path.clone()));
        }
    }

    Ok(imports)
}

/// An import path is **absolute against the project root** and names exactly
/// one file: `/lib/vec` is `<root>/lib/vec.telsb`, or the sealed equivalent
/// when `lib` is a locked package (plans/external-deps.md, "Import form
/// decision"). There is no search path and no relative form, so an import has
/// one candidate by construction — nothing to disambiguate, and no precedence
/// rule that would silently decide which of two files wins.
///
/// The extension is *not* written: it is the language's, not the author's.
fn check_import_path(path: &str, context: &str) -> Result<(), ResolveError> {
    let invalid = |why: &str| {
        Err(ResolveError::InvalidImportPath(
            context.to_string(), format!("{path} ({why})")))
    };
    let Some(rest) = path.strip_prefix('/') else {
        return invalid("import paths are absolute against the project root, so must start with '/'");
    };
    if rest.is_empty() {
        return invalid("names no file");
    }
    for segment in rest.split('/') {
        match segment {
            "" => return invalid("has an empty path segment"),
            // Rejected because they are the relative forms in disguise: they
            // would reintroduce importer-relative resolution, and `..` could
            // escape the root entirely.
            "." | ".." => return invalid("must not contain '.' or '..' segments"),
            s if s.contains('.') => return invalid("must not contain '.' (the file extension is implied)"),
            _ => {}
        }
    }
    Ok(())
}

/// The name an imported file is callable by: its last path segment. `/lib/vec`
/// is called as `vec` — the same "filename without extension" rule the
/// bare-name form used, now read off an absolute path.
pub(crate) fn import_callable_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// Resolve a parsed file body, with its imports already resolved and their
/// callable names supplied via `imports`.
///
/// Synchronous by design: import resolution — the only step that recursed —
/// happens *before* this body runs (in `Global::resolve_one`), because the
/// step's content key is built from the imports' answer fingerprints and a
/// key must be derivable before its step executes. The file is always resolved
/// as an implicit function (imported or main), so `(arg N)` is allowed.
///
/// Returns the resolved body, the symbol table, and the functions this
/// resolution registered (in registration order).
pub(crate) fn resolve_body(ctx: &ResolveContext, id: ResolveId, pre_ast: &PreExpr, imports: &[(String, FuncId)]) -> Result<(Expr, SymbolTable, Vec<FQ>), Located<ResolveError>> {
    let fq = id.func_loc;
    let context = fq.name();
    let context_str = context.resolve(ctx.interner());
    debug!("resolve_body: starting for {:?}", fq);
    let path = Path::new(fq.path_str(ctx.interner()));

    let mut resolver = Resolver::new(ctx, path.to_path_buf(), context);
    for (name, func_id) in imports {
        resolver.funcs.insert(name.clone(), func_id.clone());
    }
    debug!("resolve_body: processing local functions for {:?}", context);
    resolver.process_local_functions(pre_ast)?;

    // The file is resolved as an implicit function; pre-register it (with a
    // placeholder AST) so recursive calls can find it during body resolution.
    resolver.in_function = true;
    let arity = Resolver::calculate_arity(pre_ast, context_str, context_str)?;
    debug!("resolve_body: pre-registering implicit function {:?} with arity {}", context, arity);
    let func_loc = FQ::intern(ctx.interner(), &resolver.current_file.to_string_lossy(), context_str);
    let func_id = FuncId(func_loc);

    use crate::types::FuncData;
    ctx.func_registry().insert(func_loc, FuncData {
        loc: func_loc,
        arity,
        ast: Expr::new(Loc::SYNTHETIC, ExprKind::Number { value: 0, ty: None }),
    });
    resolver.registered.push(func_loc);
    resolver.funcs.insert(context_str.to_string(), func_id);

    debug!("resolve_body: resolving body for {:?} (in_function={})", context, resolver.in_function);
    let ast = resolver.resolve_body(pre_ast)?;

    // Swap the placeholder for the real AST now that the body is resolved.
    if let Some(mut func_data) = ctx.func_registry().get_mut(&func_loc) {
        func_data.ast = ast.clone();
    }

    debug!("resolve_body: completed for {:?}", context);
    Ok((ast, resolver.symbol_table, resolver.registered))
}

