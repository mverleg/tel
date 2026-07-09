//! Sym-free, self-contained encoding of content-store answers for the disk
//! tier (plans/roadmap.md Phase 3, plans/daemon.md "Versioning").
//!
//! The problem: stored answers carry interned ids — `FQ` inside `FuncId` and
//! `MonoId` — and a [`Sym`] is a process-local index, meaningless in any
//! other process. Hashing already solves this through `StableHash` resolving
//! syms to strings; persistence uses the same idea at the cache boundary:
//! every entry is written as a [`PortableEntry`] carrying its own **string
//! table**, with syms replaced by table indices on write and re-interned on
//! read. Entries are therefore meaningful in any process, and `FQ`s (which
//! repeat heavily — every `Call` carries one) cost four bytes per use
//! instead of an inlined path string.
//!
//! The mirrors are hand-written exhaustive matches, deliberately: adding a
//! variant to `Expr`/`MExpr` breaks compilation here — the same guardrail
//! that `StableHash` provides — instead of silently mis-persisting. The
//! alternatives (stateful serde seeds, ambient thread-local interners) were
//! rejected as more code and less visible; `Sym`'s serde derive stays what
//! it is, an in-memory convenience.
//!
//! Only resolve and mono answers need this: parse answers (`PreExpr`,
//! `ParseError`) are string-based and Sym-free, so the disk tier serializes
//! them directly with serde/postcard.
//!
//! Reading is total, not panicking: an out-of-range string index (bit rot,
//! truncated write) makes `from_portable` return `None`, which the disk
//! tier treats as a cache miss — the recompute overwrites the bad entry.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use crate::common::{Interner, Name, Path, Sym, FQ};
use crate::graph::MonoId;
use crate::keys::Fingerprint;
use crate::store::{MonoAnswer, ResolveAnswer, ResolveEntry};
use crate::types::{BinOp, Expr, ExprKind, FuncData, FuncId, Loc, Located, MExpr, MonoFuncData, ResolveError, SymbolTable, Ty, TypeError, Value, VarId};

/// One self-contained stored value: the string table all `u32` sym indices
/// in `value` point into, plus the mirrored value itself.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct PortableEntry<T> {
    strings: Vec<String>,
    value: T,
}

/// Write-side context: resolves syms via the live interner and dedups them
/// into the entry's string table.
struct PortableWriter<'a> {
    interner: &'a Interner,
    ids: HashMap<Sym, u32>,
    strings: Vec<String>,
}

impl<'a> PortableWriter<'a> {
    fn new(interner: &'a Interner) -> PortableWriter<'a> {
        PortableWriter { interner, ids: HashMap::new(), strings: Vec::new() }
    }

    fn sym(&mut self, sym: Sym) -> u32 {
        if let Some(&ix) = self.ids.get(&sym) {
            return ix;
        }
        let ix = self.strings.len() as u32;
        self.strings.push(self.interner.resolve(sym).to_string());
        self.ids.insert(sym, ix);
        ix
    }

    fn fq(&mut self, fq: &FQ) -> PFq {
        PFq {
            path: self.sym(fq.path().sym()),
            name: self.sym(fq.name().sym()),
        }
    }

    fn finish<T>(self, value: T) -> PortableEntry<T> {
        PortableEntry { strings: self.strings, value }
    }
}

/// Read-side context: the stored string table re-interned into the live
/// interner, exactly once per entry; every lookup after that is an index.
struct PortableReader {
    syms: Vec<Sym>,
}

impl PortableReader {
    fn new(strings: &[String], interner: &Interner) -> PortableReader {
        PortableReader { syms: strings.iter().map(|s| interner.intern(s)).collect() }
    }

    fn sym(&self, ix: u32) -> Option<Sym> {
        self.syms.get(ix as usize).copied()
    }

    fn fq(&self, fq: &PFq) -> Option<FQ> {
        Some(FQ::from_parts(
            Path::from_sym(self.sym(fq.path)?),
            Name::from_sym(self.sym(fq.name)?),
        ))
    }
}

// ---- Mirror types ----------------------------------------------------------
//
// Structure mirrors the real types one-to-one; the only difference is that
// every sym-carrying leaf (`FQ`) becomes indices into the entry's string
// table, and `usize` fields become `u64` so the encoded form is
// platform-width-free by type, not just by postcard's varint accident.

#[derive(Debug, Serialize, Deserialize)]
struct PFq {
    path: u32,
    name: u32,
}

// Mirrors the core `Expr` wrapper: the structural `(frame, node)` locator rides
// on the node, the shape in `kind`, so a loaded answer round-trips to the same
// `StableHash` fingerprint.
#[derive(Debug, Serialize, Deserialize)]
struct PExpr {
    frame: u32,
    node: u32,
    kind: PExprKind,
}

#[derive(Debug, Serialize, Deserialize)]
enum PExprKind {
    Number { value: i64, ty: Option<Ty> },
    VarRef(VarId),
    BinaryOp { op: BinOp, left: Box<PExpr>, right: Box<PExpr> },
    Let { var: VarId, value: Box<PExpr> },
    Set { var: VarId, value: Box<PExpr> },
    If { cond: Box<PExpr>, then_branch: Box<PExpr>, else_branch: Box<PExpr> },
    Print(Box<PExpr>),
    Return(Box<PExpr>),
    Panic { source_location: String },
    Call { func: PFq, args: Vec<Box<PExpr>> },
    Arg(u8),
    Sequence(Vec<PExpr>),
}

#[derive(Debug, Serialize, Deserialize)]
struct PFuncData {
    loc: PFq,
    arity: u64,
    ast: PExpr,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct PResolveAnswer {
    ast: PExpr,
    table: SymbolTable,
    funcs: Vec<(PFq, PFuncData)>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct PResolveEntry {
    // The error carries a `Located<ResolveError>` — already Sym-free (paths and
    // context are plain strings), so it round-trips as-is, no mirror needed.
    answer: Result<PResolveAnswer, Located<ResolveError>>,
    fingerprint: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct PMonoId {
    func_loc: PFq,
    ty: Ty,
}

#[derive(Debug, Serialize, Deserialize)]
enum PMExpr {
    Number(Value),
    VarRef(VarId),
    BinaryOp { op: BinOp, left: Box<PMExpr>, right: Box<PMExpr> },
    Let { var: VarId, value: Box<PMExpr> },
    Set { var: VarId, value: Box<PMExpr> },
    If { cond: Box<PMExpr>, then_branch: Box<PMExpr>, else_branch: Box<PMExpr> },
    Print(Box<PMExpr>),
    Return(Box<PMExpr>),
    Panic { source_location: String, frame: u32, node: u32 },
    Call { func: PMonoId, args: Vec<Box<PMExpr>> },
    Arg(u8),
    Sequence(Vec<PMExpr>),
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct PMonoFuncData {
    key: PMonoId,
    arity: u64,
    ast: PMExpr,
}

pub(crate) type PMonoAnswer = Result<(PMonoFuncData, Vec<PMonoId>), Located<TypeError>>;

// ---- Conversions: write side -------------------------------------------------

fn expr_to_portable(expr: &Expr, w: &mut PortableWriter<'_>) -> PExpr {
    let kind = match &expr.kind {
        ExprKind::Number { value, ty } => PExprKind::Number { value: *value, ty: *ty },
        ExprKind::VarRef(var) => PExprKind::VarRef(*var),
        ExprKind::BinaryOp { op, left, right } => PExprKind::BinaryOp {
            op: *op,
            left: Box::new(expr_to_portable(left, w)),
            right: Box::new(expr_to_portable(right, w)),
        },
        ExprKind::Let { var, value } => PExprKind::Let { var: *var, value: Box::new(expr_to_portable(value, w)) },
        ExprKind::Set { var, value } => PExprKind::Set { var: *var, value: Box::new(expr_to_portable(value, w)) },
        ExprKind::If { cond, then_branch, else_branch } => PExprKind::If {
            cond: Box::new(expr_to_portable(cond, w)),
            then_branch: Box::new(expr_to_portable(then_branch, w)),
            else_branch: Box::new(expr_to_portable(else_branch, w)),
        },
        ExprKind::Print(inner) => PExprKind::Print(Box::new(expr_to_portable(inner, w))),
        ExprKind::Return(inner) => PExprKind::Return(Box::new(expr_to_portable(inner, w))),
        ExprKind::Panic { source_location } => PExprKind::Panic { source_location: source_location.clone() },
        ExprKind::Call { func, args } => PExprKind::Call {
            func: w.fq(&func.0),
            args: args.iter().map(|a| Box::new(expr_to_portable(a, w))).collect(),
        },
        ExprKind::Arg(n) => PExprKind::Arg(*n),
        ExprKind::Sequence(items) => PExprKind::Sequence(items.iter().map(|e| expr_to_portable(e, w)).collect()),
    };
    PExpr { frame: expr.loc.frame, node: expr.loc.node, kind }
}

fn mexpr_to_portable(expr: &MExpr, w: &mut PortableWriter<'_>) -> PMExpr {
    match expr {
        MExpr::Number(value) => PMExpr::Number(*value),
        MExpr::VarRef(var) => PMExpr::VarRef(*var),
        MExpr::BinaryOp { op, left, right } => PMExpr::BinaryOp {
            op: *op,
            left: Box::new(mexpr_to_portable(left, w)),
            right: Box::new(mexpr_to_portable(right, w)),
        },
        MExpr::Let { var, value } => PMExpr::Let { var: *var, value: Box::new(mexpr_to_portable(value, w)) },
        MExpr::Set { var, value } => PMExpr::Set { var: *var, value: Box::new(mexpr_to_portable(value, w)) },
        MExpr::If { cond, then_branch, else_branch } => PMExpr::If {
            cond: Box::new(mexpr_to_portable(cond, w)),
            then_branch: Box::new(mexpr_to_portable(then_branch, w)),
            else_branch: Box::new(mexpr_to_portable(else_branch, w)),
        },
        MExpr::Print(inner) => PMExpr::Print(Box::new(mexpr_to_portable(inner, w))),
        MExpr::Return(inner) => PMExpr::Return(Box::new(mexpr_to_portable(inner, w))),
        MExpr::Panic { source_location, frame, node } => PMExpr::Panic { source_location: source_location.clone(), frame: *frame, node: *node },
        MExpr::Call { func, args } => PMExpr::Call {
            func: mono_id_to_portable(func, w),
            args: args.iter().map(|a| Box::new(mexpr_to_portable(a, w))).collect(),
        },
        MExpr::Arg(n) => PMExpr::Arg(*n),
        MExpr::Sequence(items) => PMExpr::Sequence(items.iter().map(|e| mexpr_to_portable(e, w)).collect()),
    }
}

fn mono_id_to_portable(id: &MonoId, w: &mut PortableWriter<'_>) -> PMonoId {
    PMonoId { func_loc: w.fq(&id.func_loc), ty: id.ty }
}

pub(crate) fn resolve_entry_to_portable(entry: &ResolveEntry, interner: &Interner) -> PortableEntry<PResolveEntry> {
    let mut w = PortableWriter::new(interner);
    let answer = match &entry.answer {
        Ok(answer) => Ok(PResolveAnswer {
            ast: expr_to_portable(&answer.ast, &mut w),
            table: answer.table.clone(),
            funcs: answer.funcs.iter().map(|(fq, data)| {
                let fq = w.fq(fq);
                let data = PFuncData {
                    loc: w.fq(&data.loc),
                    arity: data.arity as u64,
                    ast: expr_to_portable(&data.ast, &mut w),
                };
                (fq, data)
            }).collect(),
        }),
        Err(e) => Err(e.clone()),
    };
    let value = PResolveEntry { answer, fingerprint: entry.fingerprint.raw_bits() };
    w.finish(value)
}

pub(crate) fn mono_answer_to_portable(answer: &MonoAnswer, interner: &Interner) -> PortableEntry<PMonoAnswer> {
    let mut w = PortableWriter::new(interner);
    let value = match answer {
        Ok((data, instances)) => Ok((
            PMonoFuncData {
                key: mono_id_to_portable(&data.key, &mut w),
                arity: data.arity as u64,
                ast: mexpr_to_portable(&data.ast, &mut w),
            },
            instances.iter().map(|id| mono_id_to_portable(id, &mut w)).collect(),
        )),
        Err(e) => Err(e.clone()),
    };
    w.finish(value)
}

// ---- Conversions: read side --------------------------------------------------

fn expr_from_portable(expr: &PExpr, r: &PortableReader) -> Option<Expr> {
    let kind = match &expr.kind {
        PExprKind::Number { value, ty } => ExprKind::Number { value: *value, ty: *ty },
        PExprKind::VarRef(var) => ExprKind::VarRef(*var),
        PExprKind::BinaryOp { op, left, right } => ExprKind::BinaryOp {
            op: *op,
            left: Box::new(expr_from_portable(left, r)?),
            right: Box::new(expr_from_portable(right, r)?),
        },
        PExprKind::Let { var, value } => ExprKind::Let { var: *var, value: Box::new(expr_from_portable(value, r)?) },
        PExprKind::Set { var, value } => ExprKind::Set { var: *var, value: Box::new(expr_from_portable(value, r)?) },
        PExprKind::If { cond, then_branch, else_branch } => ExprKind::If {
            cond: Box::new(expr_from_portable(cond, r)?),
            then_branch: Box::new(expr_from_portable(then_branch, r)?),
            else_branch: Box::new(expr_from_portable(else_branch, r)?),
        },
        PExprKind::Print(inner) => ExprKind::Print(Box::new(expr_from_portable(inner, r)?)),
        PExprKind::Return(inner) => ExprKind::Return(Box::new(expr_from_portable(inner, r)?)),
        PExprKind::Panic { source_location } => ExprKind::Panic { source_location: source_location.clone() },
        PExprKind::Call { func, args } => ExprKind::Call {
            func: FuncId(r.fq(func)?),
            args: args.iter().map(|a| expr_from_portable(a, r).map(Box::new)).collect::<Option<Vec<_>>>()?,
        },
        PExprKind::Arg(n) => ExprKind::Arg(*n),
        PExprKind::Sequence(items) => ExprKind::Sequence(items.iter().map(|e| expr_from_portable(e, r)).collect::<Option<Vec<_>>>()?),
    };
    Some(Expr::new(Loc { frame: expr.frame, node: expr.node }, kind))
}

fn mexpr_from_portable(expr: &PMExpr, r: &PortableReader) -> Option<MExpr> {
    Some(match expr {
        PMExpr::Number(value) => MExpr::Number(*value),
        PMExpr::VarRef(var) => MExpr::VarRef(*var),
        PMExpr::BinaryOp { op, left, right } => MExpr::BinaryOp {
            op: *op,
            left: Box::new(mexpr_from_portable(left, r)?),
            right: Box::new(mexpr_from_portable(right, r)?),
        },
        PMExpr::Let { var, value } => MExpr::Let { var: *var, value: Box::new(mexpr_from_portable(value, r)?) },
        PMExpr::Set { var, value } => MExpr::Set { var: *var, value: Box::new(mexpr_from_portable(value, r)?) },
        PMExpr::If { cond, then_branch, else_branch } => MExpr::If {
            cond: Box::new(mexpr_from_portable(cond, r)?),
            then_branch: Box::new(mexpr_from_portable(then_branch, r)?),
            else_branch: Box::new(mexpr_from_portable(else_branch, r)?),
        },
        PMExpr::Print(inner) => MExpr::Print(Box::new(mexpr_from_portable(inner, r)?)),
        PMExpr::Return(inner) => MExpr::Return(Box::new(mexpr_from_portable(inner, r)?)),
        PMExpr::Panic { source_location, frame, node } => MExpr::Panic { source_location: source_location.clone(), frame: *frame, node: *node },
        PMExpr::Call { func, args } => MExpr::Call {
            func: mono_id_from_portable(func, r)?,
            args: args.iter().map(|a| mexpr_from_portable(a, r).map(Box::new)).collect::<Option<Vec<_>>>()?,
        },
        PMExpr::Arg(n) => MExpr::Arg(*n),
        PMExpr::Sequence(items) => MExpr::Sequence(items.iter().map(|e| mexpr_from_portable(e, r)).collect::<Option<Vec<_>>>()?),
    })
}

fn mono_id_from_portable(id: &PMonoId, r: &PortableReader) -> Option<MonoId> {
    Some(MonoId { func_loc: r.fq(&id.func_loc)?, ty: id.ty })
}

/// `None` means the entry is corrupt (indices out of range) — the disk tier
/// treats that as a miss.
pub(crate) fn resolve_entry_from_portable(entry: &PortableEntry<PResolveEntry>, interner: &Interner) -> Option<ResolveEntry> {
    let r = PortableReader::new(&entry.strings, interner);
    let answer = match &entry.value.answer {
        Ok(answer) => Ok(ResolveAnswer {
            ast: expr_from_portable(&answer.ast, &r)?,
            table: answer.table.clone(),
            funcs: answer.funcs.iter().map(|(fq, data)| {
                Some((r.fq(fq)?, FuncData {
                    loc: r.fq(&data.loc)?,
                    arity: data.arity as usize,
                    ast: expr_from_portable(&data.ast, &r)?,
                }))
            }).collect::<Option<Vec<_>>>()?,
        }),
        Err(e) => Err(e.clone()),
    };
    Some(ResolveEntry { answer, fingerprint: Fingerprint::from_bits(entry.value.fingerprint) })
}

/// `None` means the entry is corrupt — see
/// [`resolve_entry_from_portable`].
pub(crate) fn mono_answer_from_portable(entry: &PortableEntry<PMonoAnswer>, interner: &Interner) -> Option<MonoAnswer> {
    let r = PortableReader::new(&entry.strings, interner);
    Some(match &entry.value {
        Ok((data, instances)) => Ok((
            MonoFuncData {
                key: mono_id_from_portable(&data.key, &r)?,
                arity: data.arity as usize,
                ast: mexpr_from_portable(&data.ast, &r)?,
            },
            instances.iter().map(|id| mono_id_from_portable(id, &r)).collect::<Option<Vec<_>>>()?,
        )),
        Err(e) => Err(e.clone()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::{StableCtx, Fingerprint};
    use crate::types::{ScopeId, VarInfo};

    /// An `Expr` exercising every variant — the fixture the exhaustive
    /// mirror must carry across unchanged.
    fn every_variant_expr(interner: &Interner) -> Expr {
        // Distinct, non-default locators throughout so the round-trip must
        // preserve each `(frame, node)`, not just the shape.
        let e = |frame: u32, node: u32, kind: ExprKind| Expr::new(Loc { frame, node }, kind);
        e(0, 0, ExprKind::Sequence(vec![
            e(0, 1, ExprKind::Number { value: 1, ty: Some(Ty::I32) }),
            e(0, 2, ExprKind::VarRef(VarId(0))),
            e(0, 3, ExprKind::BinaryOp {
                op: BinOp::Add,
                left: Box::new(e(0, 4, ExprKind::Number { value: 2, ty: None })),
                right: Box::new(e(0, 5, ExprKind::Arg(1))),
            }),
            e(0, 6, ExprKind::Let { var: VarId(1), value: Box::new(e(0, 7, ExprKind::Number { value: 3, ty: None })) }),
            e(0, 8, ExprKind::Set { var: VarId(1), value: Box::new(e(0, 9, ExprKind::Number { value: 4, ty: None })) }),
            e(0, 10, ExprKind::If {
                cond: Box::new(e(0, 11, ExprKind::VarRef(VarId(1)))),
                then_branch: Box::new(e(0, 12, ExprKind::Print(Box::new(e(0, 13, ExprKind::Number { value: 5, ty: None }))))),
                else_branch: Box::new(e(0, 14, ExprKind::Return(Box::new(e(0, 15, ExprKind::Number { value: 6, ty: None }))))),
            }),
            e(2, 7, ExprKind::Panic { source_location: "a.telsb::f".to_string() }),
            e(0, 16, ExprKind::Call {
                func: FuncId(FQ::intern(interner, "lib/util.telsb", "helper")),
                args: vec![Box::new(e(0, 17, ExprKind::Arg(2)))],
            }),
        ]))
    }

    fn every_variant_mexpr(interner: &Interner) -> MExpr {
        MExpr::Sequence(vec![
            MExpr::Number(Value::I32(1)),
            MExpr::Number(Value::I64(1)),
            MExpr::VarRef(VarId(0)),
            MExpr::BinaryOp {
                op: BinOp::Mul,
                left: Box::new(MExpr::Number(Value::I64(2))),
                right: Box::new(MExpr::Arg(1)),
            },
            MExpr::Let { var: VarId(1), value: Box::new(MExpr::Number(Value::I64(3))) },
            MExpr::Set { var: VarId(1), value: Box::new(MExpr::Number(Value::I64(4))) },
            MExpr::If {
                cond: Box::new(MExpr::VarRef(VarId(1))),
                then_branch: Box::new(MExpr::Print(Box::new(MExpr::Number(Value::I64(5))))),
                else_branch: Box::new(MExpr::Return(Box::new(MExpr::Number(Value::I64(6))))),
            },
            MExpr::Panic { source_location: "a.telsb::f".to_string(), frame: 2, node: 7 },
            MExpr::Call {
                func: MonoId { func_loc: FQ::intern(interner, "lib/util.telsb", "helper"), ty: Ty::I64 },
                args: vec![Box::new(MExpr::Arg(2))],
            },
        ])
    }

    /// Interner whose indices can't line up with a freshly-built value's —
    /// the order-shuffle trick from keys.rs: if any raw Sym index survived
    /// the portable boundary, fingerprints would diverge.
    fn polluted_interner() -> Interner {
        let interner = Interner::new();
        for dummy in ["zebra", "yak", "xenon", "lib/util.telsb", "helper"] {
            interner.intern(dummy);
        }
        interner
    }

    #[test]
    fn resolve_entry_roundtrips_across_interners() {
        let source = Interner::new();
        let ast = every_variant_expr(&source);
        let entry = ResolveEntry {
            answer: Ok(ResolveAnswer {
                ast: ast.clone(),
                table: SymbolTable { vars: vec![VarInfo { name: "x".to_string(), scope_id: ScopeId(0) }] },
                funcs: vec![(FQ::intern(&source, "lib/util.telsb", "helper"), FuncData {
                    loc: FQ::intern(&source, "lib/util.telsb", "helper"),
                    arity: 2,
                    ast,
                })],
            }),
            fingerprint: Fingerprint::of(&42u64, &StableCtx { interner: &source }),
        };

        let bytes = postcard::to_allocvec(&resolve_entry_to_portable(&entry, &source)).unwrap();
        let decoded: PortableEntry<PResolveEntry> = postcard::from_bytes(&bytes).unwrap();
        let target = polluted_interner();
        let back = resolve_entry_from_portable(&decoded, &target).expect("well-formed entry must decode");

        // Fingerprint equality under each side's own interner is the
        // codebase's semantic-equality oracle: it proves the roundtripped
        // value is *stably* identical, not merely similar-looking.
        let original_fp = Fingerprint::of_ok(&entry.answer.as_ref().unwrap().ast, &StableCtx { interner: &source });
        let back_fp = Fingerprint::of_ok(&back.answer.as_ref().unwrap().ast, &StableCtx { interner: &target });
        assert_eq!(original_fp, back_fp, "roundtrip must preserve stable identity across interners");
        assert_eq!(back.fingerprint, entry.fingerprint, "stored answer fingerprint must survive");
        let back_funcs = &back.answer.as_ref().unwrap().funcs;
        assert_eq!(back_funcs[0].0.path_str(&target), "lib/util.telsb");
        assert_eq!(back_funcs[0].0.name_str(&target), "helper");
        assert_eq!(back_funcs[0].1.arity, 2);
    }

    #[test]
    fn resolve_error_entry_roundtrips() {
        let source = Interner::new();
        let entry = ResolveEntry {
            answer: Err(Located::bare(ResolveError::UndefinedFunction("main".to_string(), "missing".to_string()))),
            fingerprint: Fingerprint::of(&7u64, &StableCtx { interner: &source }),
        };
        let bytes = postcard::to_allocvec(&resolve_entry_to_portable(&entry, &source)).unwrap();
        let decoded: PortableEntry<PResolveEntry> = postcard::from_bytes(&bytes).unwrap();
        let back = resolve_entry_from_portable(&decoded, &polluted_interner()).unwrap();
        let target_ctx_err = match &back.answer {
            Err(Located { error: ResolveError::UndefinedFunction(ctx, name), .. }) => (ctx.clone(), name.clone()),
            other => panic!("wrong variant after roundtrip: {:?}", other),
        };
        assert_eq!(target_ctx_err, ("main".to_string(), "missing".to_string()));
        assert_eq!(back.fingerprint, entry.fingerprint);
    }

    #[test]
    fn mono_answer_roundtrips_across_interners() {
        let source = Interner::new();
        let answer: MonoAnswer = Ok((
            MonoFuncData {
                key: MonoId { func_loc: FQ::intern(&source, "lib/util.telsb", "helper"), ty: Ty::I64 },
                arity: 1,
                ast: every_variant_mexpr(&source),
            },
            vec![MonoId { func_loc: FQ::intern(&source, "main.telsb", "main"), ty: Ty::I32 }],
        ));

        let bytes = postcard::to_allocvec(&mono_answer_to_portable(&answer, &source)).unwrap();
        let decoded: PortableEntry<PMonoAnswer> = postcard::from_bytes(&bytes).unwrap();
        let target = polluted_interner();
        let back = mono_answer_from_portable(&decoded, &target).expect("well-formed entry must decode");

        let original_fp = Fingerprint::of_ok(answer.as_ref().unwrap(), &StableCtx { interner: &source });
        let back_fp = Fingerprint::of_ok(back.as_ref().unwrap(), &StableCtx { interner: &target });
        assert_eq!(original_fp, back_fp, "roundtrip must preserve stable identity across interners");
    }

    #[test]
    fn corrupt_string_index_is_a_miss_not_a_panic() {
        let interner = Interner::new();
        let entry = PortableEntry {
            strings: vec![], // truncated table: any index is out of range
            value: PResolveEntry {
                answer: Ok(PResolveAnswer {
                    ast: PExpr { frame: 0, node: 0, kind: PExprKind::Call { func: PFq { path: 0, name: 1 }, args: vec![] } },
                    table: SymbolTable::new(),
                    funcs: vec![],
                }),
                fingerprint: 0,
            },
        };
        assert!(resolve_entry_from_portable(&entry, &interner).is_none());
    }
}
