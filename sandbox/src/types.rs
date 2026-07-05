use std::fmt;
use crate::common::FQ;
use crate::graph::MonoId;
use serde::{Deserialize, Serialize};

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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PreExpr {
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
    /// No source location here: the parse answer is shared across paths (its
    /// content key hashes the bytes only), so it must embed nothing
    /// path-derived. Resolve — whose key pins the FQ — attaches the location.
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VarId(pub usize);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FuncId(pub FQ);

#[derive(Debug, Clone)]
pub struct FuncData {
    pub loc: FQ,
    pub arity: usize,
    pub ast: Expr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScopeId(pub usize);

#[derive(Debug, Clone)]
pub enum Expr {
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
    Panic { source_location: String },
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

#[derive(Debug, Clone)]
pub struct VarInfo {
    pub name: String,
    pub scope_id: ScopeId,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FuncSignature {
    pub loc: FQ,
    pub arity: usize,
}

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
pub enum ResolveError {
    UndefinedVariable(String, String),
    UndefinedFunction(String, String),
    InvalidImportPath(String, String),
    VariableAlreadyDefined(String, String),
    ArgOutsideFunction(String),
    InvalidArgNumber(String, u8),
    ImportNotAtTop(String),
    FunctionDefNotAfterImports(String),
    FunctionAlreadyDefined(String, String),
    FunctionOverload { loc: String, existing_arity: usize, new_arity: usize },
    ArityMismatch { context: String, func_name: String, expected: usize, got: usize },
    ArityGap { context: String, func_name: String, max_arg: usize },
    UnreachableCode { context: String, source_location: String },
    CyclicDependency { cycle: Vec<String> },
    IoError(String, String),
    ParseError(String, ParseError),
    JoinError(String),
}

impl fmt::Display for ResolveError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ResolveError::UndefinedVariable(ctx, name) => write!(f, "Undefined variable in {}: {}", ctx, name),
            ResolveError::UndefinedFunction(ctx, name) => write!(f, "Undefined function in {}: {}", ctx, name),
            ResolveError::InvalidImportPath(ctx, name) => write!(f, "Invalid import in {}: {}", ctx, name),
            ResolveError::VariableAlreadyDefined(ctx, name) => write!(f, "Variable already defined in {}: {}", ctx, name),
            ResolveError::ArgOutsideFunction(ctx) => write!(f, "Arg used outside of function in {}", ctx),
            ResolveError::InvalidArgNumber(ctx, n) => write!(f, "Invalid arg number in {}: {}", ctx, n),
            ResolveError::ImportNotAtTop(ctx) => write!(f, "Import statements must be at the top of the file in {}", ctx),
            ResolveError::FunctionDefNotAfterImports(ctx) => write!(f, "Function definitions must be after imports and before other code in {}", ctx),
            ResolveError::FunctionAlreadyDefined(ctx, name) => write!(f, "Function already defined in {}: {}", ctx, name),
            ResolveError::FunctionOverload { loc, existing_arity, new_arity } => write!(f, "Function overloading not allowed: {} has arity {} but trying to define with arity {}", loc, existing_arity, new_arity),
            ResolveError::ArityMismatch { context, func_name, expected, got } => write!(f, "Function '{}' in {} expects {} arguments, but {} were provided", func_name, context, expected, got),
            ResolveError::ArityGap { context, func_name, max_arg } => write!(f, "Function '{}' in {} has gaps in argument numbers (highest arg is {} but not all args 1..{} are used)", func_name, context, max_arg, max_arg),
            ResolveError::UnreachableCode { context, source_location } => write!(f, "Unreachable code in {} at {}", context, source_location),
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
        }
    }
}

impl std::error::Error for ResolveError {}

/// Errors from the type check / monomorphisation phase. Payloads carry
/// already-resolved context strings, like `ResolveError`.
#[derive(Debug, Clone)]
pub enum TypeError {
    Mismatch { context: String, expected: Ty, found: Ty },
    TraitNotSatisfied { context: String, ty: Ty, tr: Trait },
    LiteralOutOfRange { context: String, value: i64, ty: Ty },
    FunctionNotResolved { context: String },
}

impl fmt::Display for TypeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            TypeError::Mismatch { context, expected, found } => write!(f, "Type mismatch in {}: expected {}, found {} (no implicit conversions)", context, expected, found),
            TypeError::TraitNotSatisfied { context, ty, tr } => write!(f, "Type {} does not implement {} in {}", ty, tr, context),
            TypeError::LiteralOutOfRange { context, value, ty } => write!(f, "Literal {} does not fit in {} in {}", value, ty, context),
            TypeError::FunctionNotResolved { context } => write!(f, "Function {} was not resolved before monomorphisation", context),
        }
    }
}

impl std::error::Error for TypeError {}

#[derive(Debug)]
pub enum ExecuteError {
    DivisionByZero,
    ArgNotProvided(u8),
    Panic { source_location: String },
    ResolveError(Box<ResolveError>),
    Type(TypeError),
}

impl fmt::Display for ExecuteError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ExecuteError::DivisionByZero => write!(f, "Division by zero"),
            ExecuteError::ArgNotProvided(n) => write!(f, "Argument {} not provided", n),
            ExecuteError::Panic { source_location } => write!(f, "panic at {}", source_location),
            ExecuteError::ResolveError(e) => write!(f, "{}", e),
            ExecuteError::Type(e) => write!(f, "{}", e),
        }
    }
}

impl std::error::Error for ExecuteError {}

impl From<ResolveError> for ExecuteError {
    fn from(err: ResolveError) -> Self {
        ExecuteError::ResolveError(Box::new(err))
    }
}

impl From<TypeError> for ExecuteError {
    fn from(err: TypeError) -> Self {
        ExecuteError::Type(err)
    }
}
