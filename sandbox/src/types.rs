use std::fmt;
use crate::common::FQ;
use crate::graph::MonoId;
use serde::{Deserialize, Serialize};

/// A half-open byte range `[start, end)` into a source file. The layout half of
/// the identity/layout split (plans/fast-mode.md, approach 2b): spans live only
/// in the on-demand span sidecar ([`SpanTable`]), never in the core AST, so a
/// whitespace edit that shifts every offset leaves the fingerprint-stable core
/// untouched and only the (lazily recomputed) sidecar goes stale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ByteSpan {
    pub start: u32,
    pub end: u32,
}

/// The parse **span sidecar**: byte spans for every AST node, grouped by
/// *frame* (a per-file function ordinal — 0 is the file/implicit-`main` body,
/// 1.. are the source-order `function` definitions) and indexed within a frame
/// by the node's preorder position. The identical `(frame, node)` locator the
/// core AST carries (on `Panic`/`Unreachable`) indexes straight into here.
///
/// Per-frame numbering (not per-file) is deliberate: a node's locator must not
/// shift when a *sibling* function is edited, or the resolve→mono early cutoff
/// (which keys mono on the resolve output fingerprint) would miss for any
/// function containing a `panic`. Editing one function's body leaves every
/// other frame's ordinals and node indices bit-identical.
///
/// Recomputable from bytes at leaf-query cost, so it is content-addressed and
/// evicted aggressively (plans/fast-mode.md approach 2d); it feeds no
/// mid-pipeline key, so it can never cascade into a recompile.
#[derive(Debug, Clone, Default)]
pub struct SpanTable {
    frames: Vec<Vec<ByteSpan>>,
}

impl SpanTable {
    pub fn new() -> SpanTable {
        SpanTable { frames: Vec::new() }
    }

    /// Record `span` as the next preorder node of `frame`, growing the frame
    /// list as needed. Callers push in preorder, so the pushed index is the
    /// node's locator id.
    pub fn push(&mut self, frame: u32, span: ByteSpan) -> u32 {
        let f = frame as usize;
        if self.frames.len() <= f {
            self.frames.resize_with(f + 1, Vec::new);
        }
        let node = self.frames[f].len() as u32;
        self.frames[f].push(span);
        node
    }

    /// The span of node `node` in `frame`, if the locator is in range.
    pub fn get(&self, frame: u32, node: u32) -> Option<ByteSpan> {
        self.frames.get(frame as usize)?.get(node as usize).copied()
    }

    /// Approximate in-memory footprint in bytes — the eviction denominator for
    /// span sidecars (plans/concurrency-and-eviction.md Decision 6). Spans are
    /// memory-only, so there is no serialized length to reuse; a structural
    /// estimate (per-frame `Vec` headers + the packed `ByteSpan`s) is enough for
    /// an approximate size-aware LRU.
    pub(crate) fn approx_bytes(&self) -> u32 {
        let spans: usize = self.frames.iter().map(|f| f.len()).sum();
        let bytes = self.frames.len() * std::mem::size_of::<Vec<ByteSpan>>()
            + spans * std::mem::size_of::<ByteSpan>();
        bytes as u32
    }
}

/// 1-based `(line, column)` of byte offset `off` in `source`. Column counts
/// Unicode scalar values (chars), which is enough for the sandbox's ASCII
/// programs and avoids depending on any grapheme library.
pub fn line_col(source: &str, off: u32) -> (u32, u32) {
    let off = (off as usize).min(source.len());
    let mut line = 1u32;
    let mut col = 1u32;
    for (i, ch) in source.char_indices() {
        if i >= off {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

/// A concrete numeric type. Every value in the language is one of these; there
/// are no implicit conversions between them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Ty {
    I32,
    I64,
}

impl Ty {
    pub fn implements(&self, tr: Trait) -> bool {
        match tr {
            Trait::Number => matches!(self, Ty::I32 | Ty::I64),
        }
    }
}

impl fmt::Display for Ty {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Ty::I32 => write!(f, "i32"),
            Ty::I64 => write!(f, "i64"),
        }
    }
}

/// Built-in traits. `Number` is the implicit bound on every function's type
/// parameter and on operands of binary operators. All current types implement
/// it; a future non-numeric type (e.g. strings) would not, and would then be
/// rejected wherever this bound is checked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Trait {
    Number,
}

impl fmt::Display for Trait {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Trait::Number => write!(f, "Number"),
        }
    }
}

/// A runtime value. The variant is fixed at compile time by monomorphisation;
/// binary operations only ever see two values of the same variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Value {
    I32(i32),
    I64(i64),
}

impl Value {
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::I32(v) => *v != 0,
            Value::I64(v) => *v != 0,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Value::I32(v) => write!(f, "{}", v),
            Value::I64(v) => write!(f, "{}", v),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Greater,
    Less,
    Equal,
    And,
    Or,
}

/// A layout-independent node locator: which function (`frame` = per-file
/// function ordinal, 0 = the implicit file/`main` body) and which preorder
/// node within it (`node`). It is *structural*, not a byte/line offset, so
/// reformatting leaves it identical (no recompute); it indexes the on-demand
/// span sidecar ([`SpanTable`]) to recover `line:col`. Path-free, so the parse
/// answer stays shareable across identical files at different paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Loc {
    pub frame: u32,
    pub node: u32,
}

impl Loc {
    /// The locator of a synthetic node that has no source (e.g. resolve's
    /// zero-fill for an empty body): frame 0, node 0.
    pub const SYNTHETIC: Loc = Loc { frame: 0, node: 0 };
}

/// A source location carried on a *cached* compile error: the file plus the
/// layout-free `(frame, node)` locator. The driver upgrades it to
/// `path:line:col` lazily, via the span sidecar, on the error path — so a
/// sibling edit that shifts byte offsets yields the *current* line without
/// re-checking the (unchanged, cached) function that erred.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SrcLoc {
    pub file: String,
    pub frame: u32,
    pub node: u32,
}

impl SrcLoc {
    pub fn new(file: String, loc: Loc) -> SrcLoc {
        SrcLoc { file, frame: loc.frame, node: loc.node }
    }
}

/// A compile error together with the source location it points at. The error
/// enums stay pure (no per-variant location plumbing); the location is separate
/// metadata a producer attaches when it knows the failing node, and a `None`
/// covers whole-file errors (IO, cycles) that no single node owns. Cached as
/// the answer's error, so its rendering (via the span sidecar) stays lazy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Located<E> {
    pub error: E,
    pub loc: Option<SrcLoc>,
}

impl<E> Located<E> {
    pub fn at(error: E, loc: SrcLoc) -> Located<E> {
        Located { error, loc: Some(loc) }
    }

    pub fn bare(error: E) -> Located<E> {
        Located { error, loc: None }
    }
}

impl<E: fmt::Display> fmt::Display for Located<E> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // The coarse form; the driver upgrades `loc` to path:line:col via the
        // span sidecar before display when it can.
        self.error.fmt(f)
    }
}

impl<E: std::error::Error + 'static> std::error::Error for Located<E> {}

impl<E> From<E> for Located<E> {
    fn from(error: E) -> Located<E> {
        Located { error, loc: None }
    }
}

/// A parsed node: a structural [`Loc`] plus its [`PreExprKind`]. The core AST
/// carries no byte spans — only the locator — so it is fingerprint-stable
/// under reformatting (plans/fast-mode.md 2b).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreExpr {
    pub loc: Loc,
    pub kind: PreExprKind,
}

impl PreExpr {
    pub fn new(loc: Loc, kind: PreExprKind) -> PreExpr {
        PreExpr { loc, kind }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PreExprKind {
    /// `ty` is `Some` only for suffixed literals (`42i32`); unsuffixed
    /// literals stay polymorphic until type inference.
    Number {
        value: i64,
        ty: Option<Ty>,
    },
    Ident(String),
    BinaryOp {
        op: BinOp,
        left: Box<PreExpr>,
        right: Box<PreExpr>,
    },
    Let {
        name: String,
        value: Box<PreExpr>,
    },
    Set {
        name: String,
        value: Box<PreExpr>,
    },
    If {
        cond: Box<PreExpr>,
        then_branch: Box<PreExpr>,
        else_branch: Box<PreExpr>,
    },
    Print(Box<PreExpr>),
    Return(Box<PreExpr>),
    /// The location rides on the enclosing [`PreExpr::loc`] (path-free); the
    /// span sidecar maps it to a byte span, resolve attaches the file path.
    Panic,
    Unreachable,
    Import(String),
    FunctionDef {
        name: String,
        body: Box<PreExpr>,
    },
    Call {
        func: String,
        args: Vec<Box<PreExpr>>,
    },
    Arg(u8),
    Sequence(Vec<PreExpr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VarId(pub usize);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FuncId(pub FQ);

#[derive(Debug, Clone)]
pub struct FuncData {
    pub loc: FQ,
    pub arity: usize,
    pub ast: Expr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopeId(pub usize);

/// A resolved node: the same structural [`Loc`] carried over from parse (so it
/// still indexes the span sidecar), plus its [`ExprKind`]. Copying the parse
/// locator here is what lets typecheck point a `TypeError` at an exact
/// sub-expression without re-deriving positions.
#[derive(Debug, Clone)]
pub struct Expr {
    pub loc: Loc,
    pub kind: ExprKind,
}

impl Expr {
    pub fn new(loc: Loc, kind: ExprKind) -> Expr {
        Expr { loc, kind }
    }
}

#[derive(Debug, Clone)]
pub enum ExprKind {
    Number {
        value: i64,
        ty: Option<Ty>,
    },
    VarRef(VarId),
    BinaryOp {
        op: BinOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Let {
        var: VarId,
        value: Box<Expr>,
    },
    Set {
        var: VarId,
        value: Box<Expr>,
    },
    If {
        cond: Box<Expr>,
        then_branch: Box<Expr>,
        else_branch: Box<Expr>,
    },
    Print(Box<Expr>),
    Return(Box<Expr>),
    /// `source_location` is the coarse fallback (the file path); the
    /// span-accurate location comes from the enclosing [`Expr::loc`] via the
    /// sidecar (plans/fast-mode.md).
    Panic { source_location: String },
    Call {
        func: FuncId,
        args: Vec<Box<Expr>>,
    },
    Arg(u8),
    Sequence(Vec<Expr>),
}

/// A monomorphised expression: literals carry concrete values and calls point
/// at a concrete `(function, type)` instance. This is what the interpreter runs.
#[derive(Debug, Clone)]
pub enum MExpr {
    Number(Value),
    VarRef(VarId),
    BinaryOp {
        op: BinOp,
        left: Box<MExpr>,
        right: Box<MExpr>,
    },
    Let {
        var: VarId,
        value: Box<MExpr>,
    },
    Set {
        var: VarId,
        value: Box<MExpr>,
    },
    If {
        cond: Box<MExpr>,
        then_branch: Box<MExpr>,
        else_branch: Box<MExpr>,
    },
    Print(Box<MExpr>),
    Return(Box<MExpr>),
    Panic { source_location: String, frame: u32, node: u32 },
    Call {
        func: MonoId,
        args: Vec<Box<MExpr>>,
    },
    Arg(u8),
    Sequence(Vec<MExpr>),
}

/// One monomorphised instance of a function: its body specialised to the
/// instance's numeric type.
#[derive(Debug, Clone)]
pub struct MonoFuncData {
    pub key: MonoId,
    pub arity: usize,
    pub ast: MExpr,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VarInfo {
    pub name: String,
    pub scope_id: ScopeId,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FuncSignature {
    pub loc: FQ,
    pub arity: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolTable {
    pub vars: Vec<VarInfo>,
    // funcs now stored in Global.func_registry
}

impl Default for SymbolTable {
    fn default() -> Self {
        Self::new()
    }
}

impl SymbolTable {
    pub fn new() -> Self {
        SymbolTable {
            vars: Vec::new(),
        }
    }

    pub fn add_var(&mut self, name: String, scope_id: ScopeId) -> VarId {
        let id = VarId(self.vars.len());
        self.vars.push(VarInfo { name, scope_id });
        id
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ParseError {
    UnexpectedEof,
    UnexpectedToken(String),
    InvalidNumber(String),
    EmptyExpression,
    IoError(String),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ParseError::UnexpectedEof => write!(f, "Unexpected end of input"),
            ParseError::UnexpectedToken(tok) => write!(f, "Unexpected token: {}", tok),
            ParseError::InvalidNumber(s) => write!(f, "Invalid number: {}", s),
            ParseError::EmptyExpression => write!(f, "Empty expression"),
            ParseError::IoError(e) => write!(f, "IO error: {}", e),
        }
    }
}

impl std::error::Error for ParseError {}

impl From<std::io::Error> for ParseError {
    fn from(err: std::io::Error) -> Self {
        ParseError::IoError(err.to_string())
    }
}

/// Error payloads carry already-resolved strings (interned names/paths are
/// resolved at the throw site), so `Display` needs no `Interner`.
///
/// `Clone` because a deterministic resolve error is a terminal answer that
/// lives in the content store and is cloned out on every re-demand (the
/// `IoError` payload is a rendered string for the same reason —
/// `std::io::Error` is not `Clone`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResolveError {
    UndefinedVariable(String, String),
    UndefinedFunction(String, String),
    InvalidImportPath(String, String),
    /// The same import path listed twice in one file (context, path).
    DuplicateImport(String, String),
    /// Two different import paths whose last segment — the name the file is
    /// callable by — is the same (context, name, first path, second path).
    /// An error rather than a precedence rule: see `check_import_path`.
    ImportNameCollision(String, String, String, String),
    /// The lockfile could not be read, so no import can be resolved through
    /// it (context, lockfile path, rendered error).
    BadLockfile(String, String, String),
    /// An import whose first segment names a locked package *and* which also
    /// exists as a file under the project root (context, import path, local
    /// file, package). Neither wins — precedence is the shadowing rule this
    /// design does not have.
    AmbiguousImport(String, String, String, String),
    VariableAlreadyDefined(String, String),
    ArgOutsideFunction(String),
    InvalidArgNumber(String, u8),
    ImportNotAtTop(String),
    FunctionDefNotAfterImports(String),
    FunctionAlreadyDefined(String, String),
    FunctionOverload { loc: String, existing_arity: usize, new_arity: usize },
    ArityMismatch { context: String, func_name: String, expected: usize, got: usize },
    ArityGap { context: String, func_name: String, max_arg: usize },
    UnreachableCode { context: String, source_location: String, frame: u32, node: u32 },
    CyclicDependency { cycle: Vec<String> },
    IoError(String, String),
    ParseError(String, ParseError),
    JoinError(String),
    /// A Rust panic caught at the recompute boundary. Non-terminal by
    /// definition (invariant 6 of doc/book/src/19a-compiler-internals/09-invariants.md): a panic is
    /// an accident of the run, not a function of the content key, so this
    /// variant is never cached, never fingerprinted, and the panicking node's
    /// binding is left untouched (dirty stays dirty).
    Panicked(String),
}

impl fmt::Display for ResolveError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ResolveError::UndefinedVariable(ctx, name) => write!(f, "Undefined variable in {}: {}", ctx, name),
            ResolveError::UndefinedFunction(ctx, name) => write!(f, "Undefined function in {}: {}", ctx, name),
            ResolveError::InvalidImportPath(ctx, name) => write!(f, "Invalid import in {}: {}", ctx, name),
            ResolveError::DuplicateImport(ctx, path) => write!(f, "Duplicate import in {}: {}", ctx, path),
            ResolveError::BadLockfile(ctx, path, err) => write!(
                f, "Cannot resolve imports in {}: lockfile {} is unusable: {}", ctx, path, err),
            ResolveError::AmbiguousImport(ctx, path, local, package) => write!(
                f, "Ambiguous import {} in {}: it names both the local file {} and a file in locked package '{}'",
                path, ctx, local, package),
            ResolveError::ImportNameCollision(ctx, name, first, second) => write!(
                f, "Imports {} and {} in {} are both callable as '{}'", first, second, ctx, name),
            ResolveError::VariableAlreadyDefined(ctx, name) => write!(f, "Variable already defined in {}: {}", ctx, name),
            ResolveError::ArgOutsideFunction(ctx) => write!(f, "Arg used outside of function in {}", ctx),
            ResolveError::InvalidArgNumber(ctx, n) => write!(f, "Invalid arg number in {}: {}", ctx, n),
            ResolveError::ImportNotAtTop(ctx) => write!(f, "Import statements must be at the top of the file in {}", ctx),
            ResolveError::FunctionDefNotAfterImports(ctx) => write!(f, "Function definitions must be after imports and before other code in {}", ctx),
            ResolveError::FunctionAlreadyDefined(ctx, name) => write!(f, "Function already defined in {}: {}", ctx, name),
            ResolveError::FunctionOverload { loc, existing_arity, new_arity } => write!(f, "Function overloading not allowed: {} has arity {} but trying to define with arity {}", loc, existing_arity, new_arity),
            ResolveError::ArityMismatch { context, func_name, expected, got } => write!(f, "Function '{}' in {} expects {} arguments, but {} were provided", func_name, context, expected, got),
            ResolveError::ArityGap { context, func_name, max_arg } => write!(f, "Function '{}' in {} has gaps in argument numbers (highest arg is {} but not all args 1..{} are used)", func_name, context, max_arg, max_arg),
            ResolveError::UnreachableCode { context, source_location, .. } => write!(f, "Unreachable code in {} at {}", context, source_location),
            ResolveError::CyclicDependency { cycle } => {
                writeln!(f, "Cyclic dependency detected\n")?;
                writeln!(f, "Cycle:")?;
                for (i, location) in cycle.iter().enumerate() {
                    if i == cycle.len() - 1 {
                        writeln!(f, "  {}. {} <- cycle completes here", i + 1, location)?;
                    } else {
                        writeln!(f, "  {}. {}", i + 1, location)?;
                    }
                }
                write!(f, "\nTo fix: Remove one of the import dependencies above.")
            }
            ResolveError::IoError(path, e) => write!(f, "IO error in {}: {}", path, e),
            ResolveError::ParseError(path, e) => write!(f, "Parse error in {}: {}", path, e),
            ResolveError::JoinError(msg) => write!(f, "Join error: {}", msg),
            ResolveError::Panicked(msg) => write!(f, "Internal compiler panic during resolve: {}", msg),
        }
    }
}

impl std::error::Error for ResolveError {}

/// Errors from the type check / monomorphisation phase. Payloads carry
/// already-resolved context strings, like `ResolveError`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TypeError {
    Mismatch { context: String, expected: Ty, found: Ty },
    TraitNotSatisfied { context: String, ty: Ty, tr: Trait },
    LiteralOutOfRange { context: String, value: i64, ty: Ty },
    FunctionNotResolved { context: String },
    /// A Rust panic caught at the recompute boundary — see
    /// `ResolveError::Panicked`: non-terminal, never cached or fingerprinted.
    Panicked(String),
}

impl fmt::Display for TypeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            TypeError::Mismatch { context, expected, found } => write!(f, "Type mismatch in {}: expected {}, found {} (no implicit conversions)", context, expected, found),
            TypeError::TraitNotSatisfied { context, ty, tr } => write!(f, "Type {} does not implement {} in {}", ty, tr, context),
            TypeError::LiteralOutOfRange { context, value, ty } => write!(f, "Literal {} does not fit in {} in {}", value, ty, context),
            TypeError::FunctionNotResolved { context } => write!(f, "Function {} was not resolved before monomorphisation", context),
            TypeError::Panicked(msg) => write!(f, "Internal compiler panic during type check: {}", msg),
        }
    }
}

impl std::error::Error for TypeError {}

#[derive(Debug)]
pub enum ExecuteError {
    DivisionByZero,
    ArgNotProvided(u8),
    /// A source-level `(panic)`. `source_location` starts as the coarse file
    /// path and is upgraded in-session to `path:line:col` by the driver, which
    /// demands the span sidecar for `(frame, node)` (plans/fast-mode.md, "the
    /// runtime story"). The locator is kept so the upgrade can happen after the
    /// interpreter has unwound.
    Panic { source_location: String, frame: u32, node: u32 },
    /// An internal invariant failure (not a source-level panic), e.g. a
    /// monomorphised instance the resolver should have produced is missing.
    /// Distinct from `Panic` so it is never sent through span rendering.
    Internal(String),
    /// A deterministic compile error (resolve or type check) surfaced while
    /// driving a run, already rendered by the driver to prepend its
    /// `path:line:col` when the cached error carried a locator
    /// (plans/fast-mode.md, "upgrade on error"). Held as a rendered string
    /// because the upgrade consumes the span sidecar, which the driver — not
    /// `Display` — has access to.
    Compile(String),
}

impl fmt::Display for ExecuteError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ExecuteError::DivisionByZero => write!(f, "Division by zero"),
            ExecuteError::ArgNotProvided(n) => write!(f, "Argument {} not provided", n),
            ExecuteError::Panic { source_location, .. } => write!(f, "panic at {}", source_location),
            ExecuteError::Internal(msg) => write!(f, "{}", msg),
            ExecuteError::Compile(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for ExecuteError {}
