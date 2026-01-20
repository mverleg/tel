use crate::qcompiler2::Context;
use crate::types::{BinOp, ParseError, PreExpr};

#[derive(Debug, Clone, PartialEq)]
enum Token {
    LParen,
    RParen,
    Ident(String),
    Number(i64),
}

fn tokenize(source: &str) -> Result<Vec<Token>, ParseError> {
    let mut tokens = Vec::new();
    let mut chars = source.chars().peekable();

    while let Some(&ch) = chars.peek() {
        match ch {
            '(' => {
                tokens.push(Token::LParen);
                chars.next();
            }
            ')' => {
                tokens.push(Token::RParen);
                chars.next();
            }
            '#' => {
                while let Some(&next_ch) = chars.peek() {
                    chars.next();
                    if next_ch == '\n' {
                        break;
                    }
                }
            }
            c if c.is_whitespace() => {
                chars.next();
            }
            '-' => {
                chars.next();
                if let Some(&next_ch) = chars.peek() && next_ch.is_numeric() {
                    let mut num_str = String::from("-");
                    while let Some(&next_ch) = chars.peek() {
                        if next_ch.is_numeric() {
                            num_str.push(chars.next().unwrap());
                        } else {
                            break;
                        }
                    }
                    let num = num_str
                        .parse::<i64>()
                        .map_err(|_| ParseError::InvalidNumber(num_str.clone()))?;
                    tokens.push(Token::Number(num));
                } else {
                    tokens.push(Token::Ident("-".to_string()));
                }
            }
            c if c.is_numeric() => {
                let mut num_str = String::new();
                num_str.push(chars.next().unwrap());

                while let Some(&next_ch) = chars.peek() {
                    if next_ch.is_numeric() {
                        num_str.push(chars.next().unwrap());
                    } else {
                        break;
                    }
                }

                let num = num_str
                    .parse::<i64>()
                    .map_err(|_| ParseError::InvalidNumber(num_str.clone()))?;
                tokens.push(Token::Number(num));
            }
            _ => {
                let mut ident = String::new();
                while let Some(&next_ch) = chars.peek() {
                    if next_ch.is_whitespace() || next_ch == '(' || next_ch == ')' {
                        break;
                    }
                    ident.push(chars.next().unwrap());
                }
                tokens.push(Token::Ident(ident));
            }
        }
    }

    Ok(tokens)
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    file_path: String,
}

impl Parser {
    fn new(tokens: Vec<Token>, file_path: String) -> Self {
        Parser { tokens, pos: 0, file_path }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<Token> {
        if self.pos < self.tokens.len() {
            let tok = self.tokens[self.pos].clone();
            self.pos += 1;
            Some(tok)
        } else {
            None
        }
    }

    fn expect(&mut self, expected: Token) -> Result<(), ParseError> {
        match self.advance() {
            Some(tok) if tok == expected => Ok(()),
            Some(tok) => Err(ParseError::UnexpectedToken(format!("{:?}", tok))),
            None => Err(ParseError::UnexpectedEof),
        }
    }

    fn parse_expr(&mut self) -> Result<PreExpr, ParseError> {
        match self.peek() {
            Some(Token::Number(n)) => {
                let num = *n;
                self.advance();
                Ok(PreExpr::Number(num))
            }
            Some(Token::Ident(s)) => {
                let ident = s.clone();
                self.advance();
                Ok(PreExpr::Ident(ident))
            }
            Some(Token::LParen) => {
                self.advance();
                self.parse_sexpr()
            }
            Some(tok) => Err(ParseError::UnexpectedToken(format!("{:?}", tok))),
            None => Err(ParseError::UnexpectedEof),
        }
    }

    fn parse_sexpr(&mut self) -> Result<PreExpr, ParseError> {
        match self.peek() {
            Some(Token::Ident(op)) => {
                let op_str = op.clone();
                self.advance();

                match op_str.as_str() {
                    "+" | "-" | "*" | "/" | ">" | "<" | "==" | "&&" | "||" => {
                        let left = Box::new(self.parse_expr()?);
                        let right = Box::new(self.parse_expr()?);
                        self.expect(Token::RParen)?;

                        let bin_op = match op_str.as_str() {
                            "+" => BinOp::Add,
                            "-" => BinOp::Sub,
                            "*" => BinOp::Mul,
                            "/" => BinOp::Div,
                            ">" => BinOp::Greater,
                            "<" => BinOp::Less,
                            "==" => BinOp::Equal,
                            "&&" => BinOp::And,
                            "||" => BinOp::Or,
                            _ => unreachable!(),
                        };

                        Ok(PreExpr::BinaryOp {
                            op: bin_op,
                            left,
                            right,
                        })
                    }
                    "let" => {
                        let name = match self.advance() {
                            Some(Token::Ident(s)) => s,
                            _ => return Err(ParseError::UnexpectedToken("expected identifier".to_string())),
                        };
                        let value = Box::new(self.parse_expr()?);
                        self.expect(Token::RParen)?;

                        Ok(PreExpr::Let { name, value })
                    }
                    "set" => {
                        let name = match self.advance() {
                            Some(Token::Ident(s)) => s,
                            _ => return Err(ParseError::UnexpectedToken("expected identifier".to_string())),
                        };
                        let value = Box::new(self.parse_expr()?);
                        self.expect(Token::RParen)?;

                        Ok(PreExpr::Set { name, value })
                    }
                    "if" => {
                        let cond = Box::new(self.parse_expr()?);
                        let then_branch = Box::new(self.parse_expr()?);
                        let else_branch = Box::new(self.parse_expr()?);
                        self.expect(Token::RParen)?;

                        Ok(PreExpr::If {
                            cond,
                            then_branch,
                            else_branch,
                        })
                    }
                    "print" => {
                        let expr = Box::new(self.parse_expr()?);
                        self.expect(Token::RParen)?;
                        Ok(PreExpr::Print(expr))
                    }
                    "return" => {
                        let expr = Box::new(self.parse_expr()?);
                        self.expect(Token::RParen)?;
                        Ok(PreExpr::Return(expr))
                    }
                    "panic" => {
                        self.expect(Token::RParen)?;
                        Ok(PreExpr::Panic { source_location: self.file_path.clone() })
                    }
                    "unreachable" => {
                        self.expect(Token::RParen)?;
                        Ok(PreExpr::Unreachable { source_location: self.file_path.clone() })
                    }
                    "import" => {
                        let path = match self.advance() {
                            Some(Token::Ident(s)) => s,
                            _ => return Err(ParseError::UnexpectedToken("expected file path".to_string())),
                        };
                        self.expect(Token::RParen)?;
                        Ok(PreExpr::Import(path))
                    }
                    "function" => {
                        let name = match self.advance() {
                            Some(Token::Ident(s)) => s,
                            _ => return Err(ParseError::UnexpectedToken("expected function name".to_string())),
                        };
                        let body = Box::new(self.parse_expr()?);
                        self.expect(Token::RParen)?;
                        Ok(PreExpr::FunctionDef { name, body })
                    }
                    "call" => {
                        let func = match self.advance() {
                            Some(Token::Ident(s)) => s,
                            _ => return Err(ParseError::UnexpectedToken("expected function name".to_string())),
                        };
                        let mut args = Vec::new();
                        while !matches!(self.peek(), Some(Token::RParen)) {
                            args.push(Box::new(self.parse_expr()?));
                        }
                        self.expect(Token::RParen)?;
                        Ok(PreExpr::Call { func, args })
                    }
                    "arg" => {
                        let num = match self.advance() {
                            Some(Token::Number(n)) if n > 0 => n as u8,
                            Some(Token::Number(n)) => return Err(ParseError::UnexpectedToken(format!("arg number must be positive, got {}", n))),
                            _ => return Err(ParseError::UnexpectedToken("expected positive arg number".to_string())),
                        };
                        self.expect(Token::RParen)?;
                        Ok(PreExpr::Arg(num))
                    }
                    _ => Err(ParseError::UnexpectedToken(format!("unknown operator: {}", op_str))),
                }
            }
            _ => Err(ParseError::EmptyExpression),
        }
    }

    fn parse_all(&mut self) -> Result<PreExpr, ParseError> {
        let mut exprs = Vec::new();

        while self.peek().is_some() {
            exprs.push(self.parse_expr()?);
        }

        if exprs.is_empty() {
            Err(ParseError::EmptyExpression)
        } else if exprs.len() == 1 {
            Ok(exprs.into_iter().next().unwrap())
        } else {
            Ok(PreExpr::Sequence(exprs))
        }
    }
}

pub fn parse(source: &str, file_path: &str, a_ctx: &Context) -> Result<PreExpr, ParseError> {
    a_ctx.in_parse(file_path, |_ctx| {
        let tokens = tokenize(source)?;
        let mut parser = Parser::new(tokens, file_path.to_string());
        parser.parse_all()
    })
}
