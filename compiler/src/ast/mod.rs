use ::serde::Serialize;
use ::smartstring::alias::String as SString;

pub use self::identifier::Identifier;

mod identifier;

#[derive(Debug, PartialEq, Serialize)]
pub struct Ast {
    pub blocks: Box<[Block]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum BinOpCode {
    Add,
    Sub,
    Mul,
    Div,
    Modulo,
    Eq,
    Neq,
    Lt,
    Gt,
    Le,
    Ge,
    And,
    Or,
    Xor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum UnaryOpCode {
    Not,
    Min,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Type {
    //TODO @mark:
    pub iden: Identifier,
    pub generics: Box<[Type]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum AssignmentKw {
    None,
    Mut,
    /// This forces the assignment to be a declaration even if it has no explicit type,
    /// which only really matters when the name already exists in the outer scope but
    /// you do not want to reassign it (but shadow it in the local scope instead).
    Local,
}

#[derive(Debug, PartialEq, Serialize)]
pub enum Block {
    Assigns(Assignments),
    Expression(Expr),
    Struct(Struct),
    Enum(Enum),
}

// even without mut and type, it can be a declaration (with inferred type)
#[derive(Debug, PartialEq, Serialize)]
pub struct Assignments {
    pub dest: Box<[AssignmentDest]>,
    pub op: Option<BinOpCode>,
    pub value: Box<Expr>,
}

// even without mut and type, it can be a declaration (with inferred type)
#[derive(Debug, PartialEq, Serialize)]
pub struct AssignmentDest {
    pub kw: AssignmentKw,
    pub target: Identifier,
    pub typ: Option<Type>,
}
//TODO @mark: type cannot be combined with operation, should I create separate types?

#[derive(Debug, PartialEq, Serialize)]
pub enum Expr {
    Num(f64),
    Text(SString),
    /// Binary operation, e.g. 'x+y', 'x==y', 'x or y'. Parser handled precedence.
    BinOp(BinOpCode, Box<Expr>, Box<Expr>),
    /// Unary operation, '!x' or '-x'
    UnaryOp(UnaryOpCode, Box<Expr>),
    /// Variable read or function call.
    Invoke(Invoke),
    /// Dot-access a field or method, like x.a or x.f(a)
    Dot(Box<Expr>, Invoke),
    Closure(Closure),
    /// If, then, else (empty else is same as no else)
    If(Box<Expr>, Box<[Block]>, Box<[Block]>),
    While(Box<Expr>, Box<[Block]>),
    ForEach(AssignmentDest, Box<Expr>, Box<[Block]>),
}

/// Can be a variable read or a function call. A function call without () cannot be differentiated from
/// a function call by the parser, this must be done later.
#[derive(Debug, PartialEq, Serialize)]
pub struct Invoke {
    pub iden: Identifier,
    //TODO @mark: to smallvec or something:
    pub args: Box<[Expr]>,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct Closure {
    pub blocks: Box<[Block]>,
    pub params: Box<[AssignmentDest]>,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct Struct {
    pub iden: Identifier,
    pub fields: Vec<(Identifier, Type)>,
    pub generics: Box<[AssignmentDest]>,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct Enum {
    pub iden: Identifier,
    pub variants: Box<[EnumVariant]>,
    pub generics: Box<[AssignmentDest]>,
}

#[derive(Debug, PartialEq, Serialize)]
pub enum EnumVariant {
    Struct(Struct),
    Enum(Enum),
    Existing(Type),
}

pub fn vec_and<T>(mut items: Vec<T>, addition: Option<T>) -> Vec<T> {
    if let Some(addition) = addition {
        items.push(addition);
    }
    items
}
