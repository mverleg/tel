use crate::types::{BinOp, ParseError, PreExpr, Ty};

#[derive(Debug, Clone, PartialEq)]
enum Token {
    LParen,
    RParen,
    Ident(String),
    /// The `Ty` is `Some` only for suffixed literals like `42i32`.
    Number(i64, Option<Ty>),
}

/// Consume an optional `i32`/`i64` suffix after the digits of `num_str` and
/// produce the number token. Any other trailing ident characters make the
/// literal invalid (so `12x` is an error rather than two tokens).
fn finish_number(
    chars: &mut std::iter::Peekable<std::str::Chars>,
    num_str: String,
) -> Result<Token, ParseError> {
    let mut suffix = String::new();
    while let Some(&next_ch) = chars.peek() {
        if next_ch.is_alphanumeric() || next_ch == '_' {
            suffix.push(chars.next().unwrap());
        } else {
            break;
        }
    }

    let num = num_str
        .parse::<i64>()
        .map_err(|_| ParseError::InvalidNumber(num_str.clone()))?;

    let ty = match suffix.as_str() {
        "" => None,
        "i64" => Some(Ty::I64),
        "i32" => {
            if i32::try_from(num).is_err() {
                return Err(ParseError::InvalidNumber(format!("{}{} (out of range for i32)", num_str, suffix)));
            }
            Some(Ty::I32)
        }
        _ => return Err(ParseError::InvalidNumber(format!("{}{}", num_str, suffix))),
    };

    Ok(Token::Number(num, ty))
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
                if let Some(&next_ch) = chars.peek() {
                    if next_ch.is_numeric() {
                        let mut num_str = String::from("-");
                        while let Some(&next_ch) = chars.peek() {
                            if next_ch.is_numeric() {
                                num_str.push(chars.next().unwrap());
                            } else {
                            break;
                        }
                    }
                        tokens.push(finish_number(&mut chars, num_str)?);
                    } else {
                        tokens.push(Token::Ident("-".to_string()));
                    }
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

                tokens.push(finish_number(&mut chars, num_str)?);
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
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, pos: 0 }
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
            Some(Token::Number(n, ty)) => {
                let (num, num_ty) = (*n, *ty);
                self.advance();
                Ok(PreExpr::Number { value: num, ty: num_ty })
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
                        // Location-free on purpose: the parse answer is
                        // shared across identical files at different paths, so
                        // it must embed nothing path-derived; resolve attaches
                        // the location (its key pins the FQ).
                        Ok(PreExpr::Panic)
                    }
                    "unreachable" => {
                        self.expect(Token::RParen)?;
                        Ok(PreExpr::Unreachable)
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
                            Some(Token::Number(n, None)) if n > 0 => n as u8,
                            Some(Token::Number(n, None)) => return Err(ParseError::UnexpectedToken(format!("arg number must be positive, got {}", n))),
                            Some(Token::Number(n, Some(_))) => return Err(ParseError::UnexpectedToken(format!("arg number must not have a type suffix, got {}", n))),
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


pub fn tokenize_and_parse(source: &str) -> Result<PreExpr, ParseError> {
    let tokens = tokenize(source)?;
    let mut parser = Parser::new(tokens);
    parser.parse_all()
}
