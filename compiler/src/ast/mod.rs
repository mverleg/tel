
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Debug)]
pub enum Expr {
    Num(f64),
    Text(String),
    BinOp(Op, Box<Expr>, Box<Expr>),
}

