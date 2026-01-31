use std::fmt;
use crate::common::{Name, Path, FQ};
use serde::{Deserialize, Serialize};

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
    Number(i64),
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
    Panic { source_location: String },
    Unreachable { source_location: String },
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
    pub arity: usize,
    pub ast: Expr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScopeId(pub usize);

#[derive(Debug, Clone)]
pub enum Expr {
    Number(i64),
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
pub struct FuncInfo {
    pub loc: FQ,
    pub ast: Expr,
    pub arity: usize,
}

#[derive(Debug, Clone)]
pub struct SymbolTable {
    pub vars: Vec<VarInfo>,
    pub funcs: Vec<FuncInfo>,
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
            funcs: Vec::new(),
        }
    }

    pub fn add_var(&mut self, name: String, scope_id: ScopeId) -> VarId {
        let id = VarId(self.vars.len());
        self.vars.push(VarInfo { name, scope_id });
        id
    }

    pub fn add_func(&mut self, loc: FQ, ast: Expr, arity: usize) -> FuncId {
        let id = FuncId(self.funcs.len());
        self.funcs.push(FuncInfo { loc, ast, arity });
        id
    }
}

#[derive(Debug)]
pub enum ParseError {
    UnexpectedEof,
    UnexpectedToken(String),
    InvalidNumber(String),
    EmptyExpression,
    IoError(std::io::Error),
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
        ParseError::IoError(err)
    }
}

#[derive(Debug)]
pub enum ResolveError {
    UndefinedVariable(Name, String),
    UndefinedFunction(Name, String),
    InvalidImportPath(Name, String),
    VariableAlreadyDefined(Name, String),
    ArgOutsideFunction(Name),
    InvalidArgNumber(Name, u8),
    ImportNotAtTop(Name),
    FunctionDefNotAfterImports(Name),
    FunctionAlreadyDefined(Name, String),
    FunctionOverload { loc: FQ, existing_arity: usize, new_arity: usize },
    ArityMismatch { context: Name, func_name: String, expected: usize, got: usize },
    ArityGap { context: Name, func_name: String, max_arg: usize },
    UnreachableCode { context: Name, source_location: String },
    IoError(Path, std::io::Error),
    ParseError(Path, ParseError),
    JoinError(String),
}

impl fmt::Display for ResolveError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ResolveError::UndefinedVariable(ctx, name) => write!(f, "Undefined variable in {:?}: {}", ctx, name),
            ResolveError::UndefinedFunction(ctx, name) => write!(f, "Undefined function in {:?}: {}", ctx, name),
            ResolveError::InvalidImportPath(ctx, name) => write!(f, "Invalid import in {:?}: {}", ctx, name),
            ResolveError::VariableAlreadyDefined(ctx, name) => write!(f, "Variable already defined in {:?}: {}", ctx, name),
            ResolveError::ArgOutsideFunction(ctx) => write!(f, "Arg used outside of function in {:?}", ctx),
            ResolveError::InvalidArgNumber(ctx, n) => write!(f, "Invalid arg number in {:?}: {}", ctx, n),
            ResolveError::ImportNotAtTop(ctx) => write!(f, "Import statements must be at the top of the file in {:?}", ctx),
            ResolveError::FunctionDefNotAfterImports(ctx) => write!(f, "Function definitions must be after imports and before other code in {:?}", ctx),
            ResolveError::FunctionAlreadyDefined(ctx, name) => write!(f, "Function already defined in {:?}: {}", ctx, name),
            ResolveError::FunctionOverload { loc, existing_arity, new_arity } => write!(f, "Function overloading not allowed: {}::{} has arity {} but trying to define with arity {}", loc.as_str(), loc.name_str(), existing_arity, new_arity),
            ResolveError::ArityMismatch { context, func_name, expected, got } => write!(f, "Function '{}' in {:?} expects {} arguments, but {} were provided", func_name, context, expected, got),
            ResolveError::ArityGap { context, func_name, max_arg } => write!(f, "Function '{}' in {:?} has gaps in argument numbers (highest arg is {} but not all args 1..{} are used)", func_name, context, max_arg, max_arg),
            ResolveError::UnreachableCode { context, source_location } => write!(f, "Unreachable code in {:?} at {}", context, source_location),
            ResolveError::IoError(path, e) => write!(f, "IO error in {:?}: {}", path, e),
            ResolveError::ParseError(path, e) => write!(f, "Parse error in {:?}: {}", path, e),
            ResolveError::JoinError(msg) => write!(f, "Join error: {}", msg),
        }
    }
}

impl std::error::Error for ResolveError {}

#[derive(Debug)]
pub enum ExecuteError {
    DivisionByZero,
    ArgNotProvided(u8),
    Panic { source_location: String },
    ResolveError(Box<ResolveError>),
}

impl fmt::Display for ExecuteError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ExecuteError::DivisionByZero => write!(f, "Division by zero"),
            ExecuteError::ArgNotProvided(n) => write!(f, "Argument {} not provided", n),
            ExecuteError::Panic { source_location } => write!(f, "panic at {}", source_location),
            ExecuteError::ResolveError(e) => write!(f, "{}", e),
        }
    }
}

impl std::error::Error for ExecuteError {}

impl From<ResolveError> for ExecuteError {
    fn from(err: ResolveError) -> Self {
        ExecuteError::ResolveError(Box::new(err))
    }
}
