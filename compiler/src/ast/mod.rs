use ::serde::Serialize;
use ::serde::Serializer;
use ::smallvec::SmallVec;
use ::smartstring::alias::String as SString;
use ::smallvec;

use ::steel_api::log::debug;

#[derive(Debug, Serialize)]
pub struct Ast {
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum BinOpCode {
    Add,
    Sub,
    Mul,
    Div,
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

#[derive(Debug, Clone, PartialEq, Eq)]
//TODO @mark: serialize as string
pub struct Identifier {
    name: SString,
}

impl Serialize for Identifier {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        serializer.serialize_str(&self.name)
    }
}

impl Identifier {
    pub fn new(name: impl Into<SString>) -> Option<Self> {
        // [a-zA-Z][a-zA-Z0-9_]*
        let name = name.into();
        for ch in name.chars() {
            match ch {
                '0'..='9' => {}
                'a'..='z' => {}
                'A'..='Z' => {}
                '_' => {}
                unexpected => {
                    debug!(
                        "reject identifier because '{}' contains '{}'",
                        &name, unexpected
                    );
                    return None;
                }
            }
        }
        let first = name.chars().next()?;
        match first {
            'a'..='z' => {}
            'A'..='Z' => {}
            '_' => {}
            //TODO @mark: allow _ as leading char?
            unexpected => {
                debug!(
                    "reject identifier because '{}' starts with '{}'",
                    &name, unexpected
                );
                return None;
            }
        }
        Some(Identifier { name })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Type {
    pub iden: Identifier,
    /// no SmallVec because it would require boxing every Type
    pub generics: Vec<[Type; 1]>,
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
    pub dest: SmallVec<[AssignmentDest; 1]>,
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

// only exists because of TinyVec
impl Default for AssignmentDest {
    fn default() -> Self {
        AssignmentDest {
            kw: AssignmentKw::None,
            target: Identifier::new("$default$").unwrap(),
            typ: None,
        }
    }
}

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
}

/// Can be a variable read or a function call. A function call without () cannot be differentiated from
/// a function call by the parser, this must be done later.
#[derive(Debug, PartialEq, Serialize)]
pub struct Invoke {
    pub iden: Identifier,
    /// no SmallVec because it would require boxing every Expr
    pub args: Vec<Expr>,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct Closure {
    pub blocks: SmallVec<[Box<Block>; 2]>,
    pub params: SmallVec<[AssignmentDest; 1]>,
    /// Caching is only possible for zero-param closures, including no 'it'
    pub is_cache: bool,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct Struct {
    pub iden: Identifier,
    pub fields: SmallVec<[Field; 2]>,
    pub generics: Vec<AssignmentDest>,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct Field {
    iden: Identifier,
    typ: Type,
    //TODO @mark: optional type, for inference? ^
}

#[derive(Debug, PartialEq, Serialize)]
pub struct Enum {
    pub iden: Identifier,
    pub variants: Vec<EnumVariant>,
    pub generics: Vec<AssignmentDest>,
}

#[derive(Debug, PartialEq, Serialize)]
pub enum EnumVariant {
    Struct(Struct),
    Enum(Enum),
    Existing(Type),
}

pub fn vec_and<T: smallvec::Array>(mut items: SmallVec<T>, addition: Option<T>) -> SmallVec<T> {
    if let Some(addition) = addition {
        items.push(addition);
    }
    items
}
