
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpCode {
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Debug)]
pub enum Expr {
    Num(f64),
    Text(String),
    BinOp(OpCode, Box<Expr>, Box<Expr>),
}

