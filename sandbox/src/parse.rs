use crate::types::{BinOp, ByteSpan, ParseError, PreExpr, SpanTable, Ty};

#[derive(Debug, Clone, PartialEq)]
enum Token {
    LParen,
    RParen,
    Ident(String),
    /// The `Ty` is `Some` only for suffixed literals like `42i32`.
    Number(i64, Option<Ty>),
}

/// A token plus its half-open byte range in the source. The span is carried
/// alongside — never inside the `PreExpr` — so the core AST stays layout-free
/// and only the on-demand span sidecar depends on it (plans/fast-mode.md 2b).
#[derive(Debug, Clone)]
struct Spanned {
    tok: Token,
    span: ByteSpan,
}

/// Consume an optional `i32`/`i64` suffix after the digits of `num_str` and
/// produce the number token. Any other trailing ident characters make the
/// literal invalid (so `12x` is an error rather than two tokens).
fn finish_number(
    chars: &mut std::iter::Peekable<std::str::CharIndices>,
    num_str: String,
) -> Result<Token, ParseError> {
    let mut suffix = String::new();
    while let Some(&(_, next_ch)) = chars.peek() {
        if next_ch.is_alphanumeric() || next_ch == '_' {
            suffix.push(next_ch);
            chars.next();
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

fn tokenize(source: &str) -> Result<Vec<Spanned>, ParseError> {
    let mut tokens = Vec::new();
    let len = source.len();
    let mut chars = source.char_indices().peekable();

    // Every token ends exactly where the next unconsumed char begins (tokens
    // are contiguous once whitespace/comments are stripped), or at EOF — one
    // rule for all token kinds.
    macro_rules! push {
        ($tok:expr, $start:expr) => {{
            let end = chars.peek().map(|&(i, _)| i).unwrap_or(len);
            tokens.push(Spanned { tok: $tok, span: ByteSpan { start: $start as u32, end: end as u32 } });
        }};
    }

    while let Some(&(start, ch)) = chars.peek() {
        match ch {
            '(' => {
                chars.next();
                push!(Token::LParen, start);
            }
            ')' => {
                chars.next();
                push!(Token::RParen, start);
            }
            '#' => {
                while let Some(&(_, next_ch)) = chars.peek() {
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
                if let Some(&(_, next_ch)) = chars.peek() {
                    if next_ch.is_numeric() {
                        let mut num_str = String::from("-");
                        while let Some(&(_, d)) = chars.peek() {
                            if d.is_numeric() {
                                num_str.push(d);
                                chars.next();
                            } else {
                                break;
                            }
                        }
                        let tok = finish_number(&mut chars, num_str)?;
                        push!(tok, start);
                    } else {
                        push!(Token::Ident("-".to_string()), start);
                    }
                } else {
                    push!(Token::Ident("-".to_string()), start);
                }
            }
            c if c.is_numeric() => {
                let mut num_str = String::new();
                while let Some(&(_, d)) = chars.peek() {
                    if d.is_numeric() {
                        num_str.push(d);
                        chars.next();
                    } else {
                        break;
                    }
                }
                let tok = finish_number(&mut chars, num_str)?;
                push!(tok, start);
            }
            _ => {
                let mut ident = String::new();
                while let Some(&(_, c)) = chars.peek() {
                    if c.is_whitespace() || c == '(' || c == ')' {
                        break;
                    }
                    ident.push(c);
                    chars.next();
                }
                push!(Token::Ident(ident), start);
            }
        }
    }

    Ok(tokens)
}

struct Parser {
    tokens: Vec<Spanned>,
    pos: usize,
    /// Preorder node counter per frame (frame = function ordinal: 0 is the
    /// file/implicit body, 1.. are source-order `function` bodies). Grows as
    /// frames are entered.
    node_counters: Vec<u32>,
    /// The frame nodes are currently being numbered in.
    current_frame: u32,
    /// Next frame ordinal to hand out when a `function` body is entered.
    next_frame: u32,
    /// When recording, `(frame, node, span)` for every node — converted to a
    /// dense [`SpanTable`] at the end. `None` on the fast path, so the span
    /// machinery costs only a few counter increments there.
    pending: Option<Vec<(u32, u32, ByteSpan)>>,
    /// Byte length of the source, for the end offset of a node that runs to EOF.
    src_len: u32,
}

impl Parser {
    fn new(tokens: Vec<Spanned>, record: bool, src_len: usize) -> Self {
        Parser {
            tokens,
            pos: 0,
            node_counters: vec![0],
            current_frame: 0,
            next_frame: 1,
            pending: if record { Some(Vec::new()) } else { None },
            src_len: src_len as u32,
        }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos).map(|s| &s.tok)
    }

    fn advance(&mut self) -> Option<Token> {
        if self.pos < self.tokens.len() {
            let tok = self.tokens[self.pos].tok.clone();
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

    /// Byte offset where the next (unconsumed) token starts, or EOF.
    fn cur_start(&self) -> u32 {
        self.tokens.get(self.pos).map(|s| s.span.start).unwrap_or(self.src_len)
    }

    /// Byte offset just past the last consumed token.
    fn prev_end(&self) -> u32 {
        if self.pos == 0 {
            0
        } else {
            self.tokens[self.pos - 1].span.end
        }
    }

    /// Allocate the next preorder node id in `frame`.
    fn alloc(&mut self, frame: u32) -> u32 {
        let f = frame as usize;
        if self.node_counters.len() <= f {
            self.node_counters.resize(f + 1, 0);
        }
        let id = self.node_counters[f];
        self.node_counters[f] += 1;
        id
    }

    /// Parse one expression, assigning it the next preorder locator in the
    /// current frame and (when recording) its byte span. This is the single
    /// chokepoint every `PreExpr` node flows through, so ids assigned here are
    /// identical whether or not spans are recorded — that is what keeps the
    /// core AST's `(frame, node)` locators and the span sidecar in lockstep.
    fn parse_expr(&mut self) -> Result<PreExpr, ParseError> {
        let frame = self.current_frame;
        let node = self.alloc(frame);
        let start = self.cur_start();
        let expr = self.parse_expr_inner(frame, node)?;
        let end = self.prev_end();
        if let Some(pending) = &mut self.pending {
            pending.push((frame, node, ByteSpan { start, end }));
        }
        Ok(expr)
    }

    fn parse_expr_inner(&mut self, frame: u32, node: u32) -> Result<PreExpr, ParseError> {
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
                self.parse_sexpr(frame, node)
            }
            Some(tok) => Err(ParseError::UnexpectedToken(format!("{:?}", tok))),
            None => Err(ParseError::UnexpectedEof),
        }
    }

    /// Parse the body of an S-expression (the `(` already consumed). `frame`
    /// and `node` are the locator of the enclosing expression, so `panic` /
    /// `unreachable` can embed it.
    fn parse_sexpr(&mut self, _frame: u32, node: u32) -> Result<PreExpr, ParseError> {
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
                        // Carries only its layout-independent locator (the
                        // enclosing expr's `(frame, node)`), which is path-free
                        // — the parse answer stays shared across identical
                        // files at different paths; resolve attaches the path,
                        // the span sidecar maps the locator to a byte span.
                        Ok(PreExpr::Panic { frame: _frame, node })
                    }
                    "unreachable" => {
                        self.expect(Token::RParen)?;
                        Ok(PreExpr::Unreachable { frame: _frame, node })
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
                        // The body is a fresh frame: its nodes are numbered
                        // relative to this function, so editing a sibling
                        // function never shifts them (the per-function cutoff
                        // rationale in SpanTable's docs).
                        let body_frame = self.next_frame;
                        self.next_frame += 1;
                        let saved = self.current_frame;
                        self.current_frame = body_frame;
                        let body = Box::new(self.parse_expr()?);
                        self.current_frame = saved;
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
            // The wrapping sequence is the frame-0 root; it carries no locator
            // of its own (nothing references it), so it is not numbered.
            Ok(PreExpr::Sequence(exprs))
        }
    }

    /// Fold the recorded `(frame, node, span)` triples into a dense table.
    /// Node ids per frame are `0..count` and each is recorded exactly once, so
    /// placement by index leaves no holes.
    fn into_span_table(self) -> SpanTable {
        let mut table = SpanTable::new();
        let mut frames: Vec<Vec<ByteSpan>> = self
            .node_counters
            .iter()
            .map(|&n| vec![ByteSpan { start: 0, end: 0 }; n as usize])
            .collect();
        if let Some(pending) = self.pending {
            for (frame, node, span) in pending {
                frames[frame as usize][node as usize] = span;
            }
        }
        for (frame, spans) in frames.into_iter().enumerate() {
            for span in spans {
                table.push(frame as u32, span);
            }
        }
        table
    }
}

/// Fast path: parse to the core AST only. Assigns `(frame, node)` locators (a
/// handful of counter increments) but records no spans — the happy path never
/// pays for the sidecar (plans/fast-mode.md).
pub fn tokenize_and_parse(source: &str) -> Result<PreExpr, ParseError> {
    let tokens = tokenize(source)?;
    let mut parser = Parser::new(tokens, false, source.len());
    parser.parse_all()
}

/// Detail path: parse to the core AST **and** the span sidecar. The AST is
/// bit-identical to [`tokenize_and_parse`]'s (same locators), so a locator
/// minted on the fast path indexes correctly into this table. Called only when
/// a diagnostic or the runtime actually needs a source span.
pub fn parse_with_spans(source: &str) -> Result<(PreExpr, SpanTable), ParseError> {
    let tokens = tokenize(source)?;
    let mut parser = Parser::new(tokens, true, source.len());
    let ast = parser.parse_all()?;
    Ok((ast, parser.into_span_table()))
}
