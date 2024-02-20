use ::serde::Serialize;

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
