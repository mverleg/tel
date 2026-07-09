//! Type check + monomorphisation (compiler phase 3).
//!
//! Every function is implicitly generic over a single type parameter
//! `T: Number`: all its arguments and its return value have type `T`. A
//! function is therefore monomorphised into at most one instance per numeric
//! type ([`MonoId`] = function + `Ty`), and argument/return types never mix
//! within one call — there are no implicit conversions.
//!
//! Because a call's result type equals the type it instantiates the callee
//! at, checking one instance never needs the callee's body to be checked
//! first. Instances are processed as a memoized worklist (see
//! `Global::mono_impl` in context.rs), which makes recursion — even
//! polymorphic recursion like `f@i64` calling `f@i32` — terminate trivially.
//!
//! Inside a body, unsuffixed literals and locals are still inferred with a
//! small union-find, so different locals may have different types as long as
//! no operation mixes them. Anything left unconstrained defaults to the
//! instance's own type.

use crate::common::Ctx;
use crate::context::MonoContext;
use crate::graph::MonoId;
use crate::types::{Expr, ExprKind, Loc, Located, MExpr, MonoFuncData, SrcLoc, Trait, Ty, TypeError, Value, VarId};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TermId(usize);

#[derive(Debug, Clone, Copy)]
enum Slot {
    Free,
    Link(TermId),
    Bound(Ty),
}

/// Body of a function instance with inference terms still attached: literals
/// carry their term, calls already point at a concrete instance. Lowered to
/// [`MExpr`] once all constraints are collected.
enum TExpr {
    Number { value: i64, term: TermId },
    VarRef(VarId),
    BinaryOp { op: crate::types::BinOp, left: Box<TExpr>, right: Box<TExpr> },
    Let { var: VarId, value: Box<TExpr> },
    Set { var: VarId, value: Box<TExpr> },
    If { cond: Box<TExpr>, then_branch: Box<TExpr>, else_branch: Box<TExpr> },
    Print(Box<TExpr>),
    Return(Box<TExpr>),
    Panic { source_location: String, frame: u32, node: u32 },
    Call { target: MonoId, args: Vec<Box<TExpr>> },
    Arg(u8),
    Sequence(Vec<TExpr>),
}

/// Check and monomorphise one `(function, type)` instance.
///
/// Returns the specialised body plus the instances it calls (`needed`), which
/// the caller feeds back into the worklist. `is_entry` relaxes the
/// return-type constraint: `main` is not called by anyone, so its final
/// expression may have any type.
pub fn check_function(
    ctx: &MonoContext,
    key: MonoId,
    is_entry: bool,
) -> Result<(MonoFuncData, Vec<MonoId>), Located<TypeError>> {
    let func = ctx.func_registry()
        .get(&key.func_loc)
        .map(|f| f.clone())
        .ok_or_else(|| TypeError::FunctionNotResolved { context: context_str(ctx, key) })?;

    let mut ck = Checker::new(ctx, key, is_entry);
    let (body, body_term) = ck.check(&func.ast)?;
    if !is_entry {
        let ret = ck.ret_term;
        // The return-type constraint spans the whole body; pin it to the body's
        // root node.
        let loc = func.ast.loc;
        ck.unify(body_term, ret).map_err(|e| ck.located(e, loc))?;
    }
    let ast = ck.lower(body)?;

    Ok((MonoFuncData { key, arity: func.arity, ast }, ck.needed))
}

fn context_str(ctx: &MonoContext, key: MonoId) -> String {
    format!("{} @ {}", Ctx(&key.func_loc, ctx.interner()), key.ty)
}

struct Checker<'a> {
    ctx: &'a MonoContext<'a>,
    key: MonoId,
    slots: Vec<Slot>,
    vars: HashMap<VarId, TermId>,
    /// Type of every `(arg N)`: the instance's type parameter.
    arg_term: TermId,
    /// Type of `(return ...)` values and the body result.
    ret_term: TermId,
    /// Instances this body calls; fed back into the mono worklist.
    needed: Vec<MonoId>,
}

impl<'a> Checker<'a> {
    fn new(ctx: &'a MonoContext, key: MonoId, is_entry: bool) -> Self {
        let mut ck = Checker {
            ctx,
            key,
            slots: Vec::new(),
            vars: HashMap::new(),
            arg_term: TermId(0),
            ret_term: TermId(0),
            needed: Vec::new(),
        };
        ck.arg_term = ck.bound(key.ty);
        ck.ret_term = if is_entry { ck.fresh() } else { ck.bound(key.ty) };
        ck
    }

    fn context(&self) -> String {
        context_str(self.ctx, self.key)
    }

    /// Pin a type error to the node it came from, so the driver can upgrade it
    /// to `path:line:col` via the span sidecar. The file is the checked
    /// instance's defining file (its FQ path); the locator is structural, so a
    /// sibling edit that shifts byte offsets leaves it — and the cached,
    /// unchanged instance it points into — undisturbed.
    fn located(&self, error: TypeError, loc: Loc) -> Located<TypeError> {
        let file = self.key.func_loc.path_str(self.ctx.interner()).to_string();
        Located::at(error, SrcLoc::new(file, loc))
    }

    fn fresh(&mut self) -> TermId {
        self.slots.push(Slot::Free);
        TermId(self.slots.len() - 1)
    }

    fn bound(&mut self, ty: Ty) -> TermId {
        self.slots.push(Slot::Bound(ty));
        TermId(self.slots.len() - 1)
    }

    /// Find the representative of `t`, with path compression.
    fn find(&mut self, t: TermId) -> TermId {
        match self.slots[t.0] {
            Slot::Link(parent) => {
                let root = self.find(parent);
                self.slots[t.0] = Slot::Link(root);
                root
            }
            _ => t,
        }
    }

    fn resolved(&mut self, t: TermId) -> Option<Ty> {
        let root = self.find(t);
        match self.slots[root.0] {
            Slot::Bound(ty) => Some(ty),
            _ => None,
        }
    }

    fn unify(&mut self, a: TermId, b: TermId) -> Result<(), TypeError> {
        let ra = self.find(a);
        let rb = self.find(b);
        if ra == rb {
            return Ok(());
        }
        match (self.slots[ra.0], self.slots[rb.0]) {
            (Slot::Bound(ta), Slot::Bound(tb)) => {
                if ta == tb {
                    self.slots[rb.0] = Slot::Link(ra);
                    Ok(())
                } else {
                    Err(TypeError::Mismatch { context: self.context(), expected: ta, found: tb })
                }
            }
            (Slot::Bound(_), Slot::Free) => {
                self.slots[rb.0] = Slot::Link(ra);
                Ok(())
            }
            _ => {
                self.slots[ra.0] = Slot::Link(rb);
                Ok(())
            }
        }
    }

    /// Force `t` to a concrete type, defaulting an unconstrained term to the
    /// instance's own type parameter.
    fn resolve_or_default(&mut self, t: TermId) -> Ty {
        let root = self.find(t);
        match self.slots[root.0] {
            Slot::Bound(ty) => ty,
            _ => {
                self.slots[root.0] = Slot::Bound(self.key.ty);
                self.key.ty
            }
        }
    }

    /// Enforce the `Number` bound where an operand must be numeric. A still
    /// unconstrained term carries the bound implicitly (it can only ever be
    /// bound to a numeric type, and defaults to one).
    fn require_number(&mut self, t: TermId) -> Result<(), TypeError> {
        if let Some(ty) = self.resolved(t) {
            self.require_concrete_number(ty)?;
        }
        Ok(())
    }

    fn require_concrete_number(&self, ty: Ty) -> Result<(), TypeError> {
        if !ty.implements(Trait::Number) {
            return Err(TypeError::TraitNotSatisfied { context: self.context(), ty, tr: Trait::Number });
        }
        Ok(())
    }

    fn check(&mut self, expr: &Expr) -> Result<(TExpr, TermId), Located<TypeError>> {
        let loc = expr.loc;
        match &expr.kind {
            ExprKind::Number { value, ty } => {
                let term = match ty {
                    Some(t) => {
                        self.require_concrete_number(*t).map_err(|e| self.located(e, loc))?;
                        self.bound(*t)
                    }
                    None => self.fresh(),
                };
                Ok((TExpr::Number { value: *value, term }, term))
            }
            ExprKind::VarRef(var_id) => {
                let term = *self.vars.get(var_id)
                    .expect("resolver guarantees variables are declared before use");
                Ok((TExpr::VarRef(*var_id), term))
            }
            ExprKind::BinaryOp { op, left, right } => {
                let (tl, l) = self.check(left)?;
                let (tr, r) = self.check(right)?;
                self.unify(l, r).map_err(|e| self.located(e, loc))?;
                self.require_number(l).map_err(|e| self.located(e, loc))?;
                Ok((TExpr::BinaryOp { op: *op, left: Box::new(tl), right: Box::new(tr) }, l))
            }
            ExprKind::Let { var, value } => {
                let (tv, t) = self.check(value)?;
                self.vars.insert(*var, t);
                Ok((TExpr::Let { var: *var, value: Box::new(tv) }, t))
            }
            ExprKind::Set { var, value } => {
                let (tv, t) = self.check(value)?;
                let var_term = *self.vars.get(var)
                    .expect("resolver guarantees variables are declared before use");
                self.unify(var_term, t)?;
                Ok((TExpr::Set { var: *var, value: Box::new(tv) }, t))
            }
            ExprKind::If { cond, then_branch, else_branch } => {
                let (tc, c) = self.check(cond)?;
                self.require_number(c).map_err(|e| self.located(e, loc))?;
                let (tt, t_then) = self.check(then_branch)?;
                let (te, t_else) = self.check(else_branch)?;
                self.unify(t_then, t_else).map_err(|e| self.located(e, loc))?;
                Ok((TExpr::If {
                    cond: Box::new(tc),
                    then_branch: Box::new(tt),
                    else_branch: Box::new(te),
                }, t_then))
            }
            ExprKind::Print(e) => {
                let (te, t) = self.check(e)?;
                Ok((TExpr::Print(Box::new(te)), t))
            }
            ExprKind::Return(e) => {
                let (te, t) = self.check(e)?;
                let ret = self.ret_term;
                self.unify(t, ret).map_err(|e| self.located(e, loc))?;
                // A return never yields a value in place, so its own type is
                // unconstrained (like Rust's `!`).
                let never = self.fresh();
                Ok((TExpr::Return(Box::new(te)), never))
            }
            ExprKind::Panic { source_location } => {
                let never = self.fresh();
                Ok((TExpr::Panic { source_location: source_location.clone(), frame: expr.loc.frame, node: expr.loc.node }, never))
            }
            ExprKind::Call { func, args } => {
                // All arguments share the callee's single type parameter.
                let unified = self.fresh();
                let mut targs = Vec::with_capacity(args.len());
                for arg in args {
                    let (ta, t) = self.check(arg)?;
                    self.unify(unified, t).map_err(|e| self.located(e, loc))?;
                    targs.push(Box::new(ta));
                }
                // The instantiation type must be concrete here to pick the
                // instance; a zero-arg or unconstrained call follows the
                // enclosing instance's type.
                let inst_ty = self.resolve_or_default(unified);
                self.require_concrete_number(inst_ty).map_err(|e| self.located(e, loc))?;
                let target = MonoId { func_loc: func.0, ty: inst_ty };
                self.needed.push(target);
                let result = self.bound(inst_ty);
                Ok((TExpr::Call { target, args: targs }, result))
            }
            ExprKind::Arg(n) => Ok((TExpr::Arg(*n), self.arg_term)),
            ExprKind::Sequence(exprs) => {
                let mut texprs = Vec::with_capacity(exprs.len());
                let mut last_term = self.fresh();
                for e in exprs {
                    let (te, t) = self.check(e)?;
                    texprs.push(te);
                    last_term = t;
                }
                Ok((TExpr::Sequence(texprs), last_term))
            }
        }
    }

    /// Resolve all remaining terms and produce the monomorphised body.
    fn lower(&mut self, expr: TExpr) -> Result<MExpr, TypeError> {
        Ok(match expr {
            TExpr::Number { value, term } => {
                let ty = self.resolve_or_default(term);
                let val = match ty {
                    Ty::I32 => Value::I32(i32::try_from(value).map_err(|_| {
                        TypeError::LiteralOutOfRange { context: self.context(), value, ty }
                    })?),
                    Ty::I64 => Value::I64(value),
                };
                MExpr::Number(val)
            }
            TExpr::VarRef(var) => MExpr::VarRef(var),
            TExpr::BinaryOp { op, left, right } => MExpr::BinaryOp {
                op,
                left: Box::new(self.lower(*left)?),
                right: Box::new(self.lower(*right)?),
            },
            TExpr::Let { var, value } => MExpr::Let { var, value: Box::new(self.lower(*value)?) },
            TExpr::Set { var, value } => MExpr::Set { var, value: Box::new(self.lower(*value)?) },
            TExpr::If { cond, then_branch, else_branch } => MExpr::If {
                cond: Box::new(self.lower(*cond)?),
                then_branch: Box::new(self.lower(*then_branch)?),
                else_branch: Box::new(self.lower(*else_branch)?),
            },
            TExpr::Print(e) => MExpr::Print(Box::new(self.lower(*e)?)),
            TExpr::Return(e) => MExpr::Return(Box::new(self.lower(*e)?)),
            TExpr::Panic { source_location, frame, node } => MExpr::Panic { source_location, frame, node },
            TExpr::Call { target, args } => MExpr::Call {
                func: target,
                args: args.into_iter()
                    .map(|a| self.lower(*a).map(Box::new))
                    .collect::<Result<_, _>>()?,
            },
            TExpr::Arg(n) => MExpr::Arg(n),
            TExpr::Sequence(exprs) => MExpr::Sequence(
                exprs.into_iter().map(|e| self.lower(e)).collect::<Result<_, _>>()?,
            ),
        })
    }
}
