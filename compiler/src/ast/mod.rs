
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
        let name = name.into();
        for ch in name.chars() {
            match ch {
                '0'..='9' => {},
                'a'..='z' => {},
                'Z'..='Z' => {},
                '_' => {},
                _ => return None,
            }
        }
        let first = name.chars().next()?;
        match first {
            'a'..='z' => {},
            'A'..='Z' => {},
            //TODO @mark: allow _ as leading char?
            _ => return None,
        }
        Some(Identifier { name })
    }
}

#[derive(Debug)]
pub enum Expr {
    Num(f64),
    Text(String),
    BinOp(OpCode, Box<Expr>, Box<Expr>),
}

