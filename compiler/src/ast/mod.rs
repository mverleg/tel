use ::smartstring::alias::String as SString;

use ::steel_api::log::debug;

#[derive(Debug)]
pub struct Ast {
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpCode {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identifier {
    name: SString,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Type {
    //TODO @mark:
    iden: Identifier,
}

impl Type {
    pub fn new(iden: Identifier) -> Self {
        Type { iden }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignmentKw {
    None,
    Mut,
    /// This forces the assignment to be a declaration even if it has no explicit type,
    /// which only really matters when the name already exists in the outer scope but
    /// you do not want to reassign it (but shadow it in the local scope instead).
    Local,
}

#[derive(Debug, PartialEq)]
pub enum Block {
    Assigns(Assignments),
    Expression(Expr),
    Struct(Struct),
}

// even without mut and type, it can be a declaration (with inferred type)
#[derive(Debug, PartialEq)]
pub struct Assignments {
    //pub dest: TinyVec<[AssignmentDest; 1]>,
    //TODO @mark: ^
    pub dest: Vec<AssignmentDest>,
    pub op: Option<OpCode>,
    pub value: Box<Expr>,
}

// even without mut and type, it can be a declaration (with inferred type)
#[derive(Debug, PartialEq)]
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

#[derive(Debug, PartialEq)]
pub enum Expr {
    Num(f64),
    Text(SString),
    /// Binary operation, i.e. '+', '==', 'or'. Parser handled precedence.
    BinOp(OpCode, Box<Expr>, Box<Expr>),
    /// Variable read or function call.
    Invoke(Invoke),
    /// Dot-access a field or method, like x.a or x.f(a)
    Dot(Box<Expr>, Invoke),
    Closure(Closure),
}

/// Can be a variable read or a function call. A function call without () cannot be differentiated from
/// a function call by the parser, this must be done later.
#[derive(Debug, PartialEq)]
pub struct Invoke {
    pub iden: Identifier,
    //TODO @mark: to smallvec or something:
    pub args: Vec<Expr>,
}

#[derive(Debug, PartialEq)]
pub struct Closure {
    pub blocks: Vec<Block>,
    pub params: Vec<AssignmentDest>,
}

#[derive(Debug, PartialEq)]
pub struct Struct {
    pub iden: Identifier,
    pub fields: Vec<(Identifier, Type)>,
}
