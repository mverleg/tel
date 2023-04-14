use steel_api::log::debug;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpCode {
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Identifier {
    name: String,
}

impl Identifier {
    pub fn new(name: impl Into<String>) -> Option<Self> {
        // [a-zA-Z][a-zA-Z0-9_]*
        let name = name.into();
        for ch in name.chars() {
            match ch {
                '0'..='9' => {},
                'a'..='z' => {},
                'A'..='Z' => {},
                '_' => {},
                unexpected => {
                    debug!("reject identifier because '{}' contains '{}'", &name, unexpected);
                    return None
                },
            }
        }
        let first = name.chars().next()?;
        match first {
            'a'..='z' => {},
            'A'..='Z' => {},
            //TODO @mark: allow _ as leading char?
            unexpected => {
                debug!("reject identifier because '{}' starts with '{}'", &name, unexpected);
                return None
            },
        }
        Some(Identifier { name })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Type {
    //TODO @mark:
    iden: Identifier,
}

impl Type {
    pub fn new(iden: Identifier) -> Self {
        Type { iden }
    }
}

#[derive(Debug)]
pub enum Expr {
    Num(f64),
    Text(String),
    BinOp(OpCode, Box<Expr>, Box<Expr>),
    Struct(Identifier, Vec<(Identifier, Type)>),
}

