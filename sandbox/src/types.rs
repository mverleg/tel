use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[derive(Debug, Clone)]
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
    Import(String),
    FunctionDef {
        name: String,
        body: Box<PreExpr>,
    },
    Call {
        func: String,
        arg1: Box<PreExpr>,
        arg2: Box<PreExpr>,
    },
    Arg(u8),
    Sequence(Vec<PreExpr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VarId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FuncId(pub usize);

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
    Call {
        func: FuncId,
        arg1: Box<Expr>,
        arg2: Box<Expr>,
    },
    Arg(u8),
    Sequence(Vec<Expr>),
}

#[derive(Debug, Clone)]
pub struct VarInfo {
    pub name: String,
    pub scope_id: ScopeId,
}

#[derive(Debug, Clone)]
pub struct FuncInfo {
    pub name: String,
    pub ast: Expr,
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

    pub fn add_func(&mut self, name: String, ast: Expr) -> FuncId {
        let id = FuncId(self.funcs.len());
        self.funcs.push(FuncInfo { name, ast });
        id
    }
}

#[derive(Debug)]
pub enum ParseError {
    UnexpectedEof,
    UnexpectedToken(String),
    InvalidNumber(String),
    EmptyExpression,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ParseError::UnexpectedEof => write!(f, "Unexpected end of input"),
            ParseError::UnexpectedToken(tok) => write!(f, "Unexpected token: {}", tok),
            ParseError::InvalidNumber(s) => write!(f, "Invalid number: {}", s),
            ParseError::EmptyExpression => write!(f, "Empty expression"),
        }
    }
}

impl std::error::Error for ParseError {}

#[derive(Debug)]
pub enum ResolveError {
    UndefinedVariable(String),
    UndefinedFunction(String),
    InvalidImportPath(String),
    VariableAlreadyDefined(String),
    ArgOutsideFunction,
    InvalidArgNumber(u8),
    ImportNotAtTop,
    FunctionDefNotAfterImports,
    FunctionAlreadyDefined(String),
}

impl fmt::Display for ResolveError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ResolveError::UndefinedVariable(name) => write!(f, "Undefined variable: {}", name),
            ResolveError::UndefinedFunction(name) => write!(f, "Undefined function: {}", name),
            ResolveError::InvalidImportPath(name) => write!(f, "Invalid import: {}", name),
            ResolveError::VariableAlreadyDefined(name) => write!(f, "Variable already defined: {}", name),
            ResolveError::ArgOutsideFunction => write!(f, "Arg used outside of function"),
            ResolveError::InvalidArgNumber(n) => write!(f, "Invalid arg number: {} (must be 1 or 2)", n),
            ResolveError::ImportNotAtTop => write!(f, "Import statements must be at the top of the file"),
            ResolveError::FunctionDefNotAfterImports => write!(f, "Function definitions must be after imports and before other code"),
            ResolveError::FunctionAlreadyDefined(name) => write!(f, "Function already defined: {}", name),
        }
    }
}

impl std::error::Error for ResolveError {}

#[derive(Debug)]
pub enum ExecuteError {
    DivisionByZero,
    ArgNotProvided(u8),
}

impl fmt::Display for ExecuteError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ExecuteError::DivisionByZero => write!(f, "Division by zero"),
            ExecuteError::ArgNotProvided(n) => write!(f, "Argument {} not provided", n),
        }
    }
}

impl std::error::Error for ExecuteError {}
