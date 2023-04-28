//TODO @mark: delete either this if lalrpop turns out better, or delete lalrpop if going this way

use ::std::fmt;
use ::std::fmt::Formatter;
use ::std::path::PathBuf;
use ::std::rc::Rc;
use ::std::sync::LazyLock;

use ::itertools::Itertools;
use ::regex::Regex;

use ::steel_api::log::debug;
use ::steel_api::log::trace;

use crate::ast::Ast;
use crate::ast::Block;
use crate::ast::Block::Expression;
use crate::ast::Expr;
use crate::ast::Identifier;
use crate::ast::OpCode;
use crate::parser::lexer::Token;
use crate::parser::lexer::Token::OpSymbol;
use crate::parser::lexer::tokenize;
use crate::SteelErr;
use crate::SteelErr::ParseErr;

type ParseRes<T> = Result<(T, Cursor), SteelErr>;
//TODO @mark: should I distinguish between ot found and found incorrect? e.g. when parsing a block, it is valid to not find an expression but find "struct" instead, but it is not valid to find "(" without ")"

struct Cursor {
    index: usize,
    tokens: Rc<Vec<Token>>,
}

impl Cursor {
    fn new(tokens: Vec<Token>) -> Self {
        Cursor {
            index: 0,
            tokens: Rc::new(tokens),
        }
    }

    fn fork(&self) -> Self {
        Cursor {
            index: self.index,
            tokens: self.tokens.clone(),
        }
    }

    fn take(&mut self) -> Option<&Token> {
        let prev = self.index;
        self.index += 1;
        self.tokens.get(prev)
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.index)
    }

    fn take_if(&mut self, condition: fn(&Token) -> bool) -> Option<&Token> {
        let Some(val) = self.tokens.get(self.index) else {
            return None
        };
        if !condition(val) {
            return None;
        }
        Some(val)
    }

    fn take_while(&mut self, condition: fn(&Token) -> bool) -> usize {
        let mut cnt = 0;
        loop {
            if self.take_if(condition).is_none() {
                break;
            }
            self.index += 1;
            cnt += 1;
        }
        cnt
    }
}

impl fmt::Debug for Cursor {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let mut i = 0;
        if self.index > 2 {
            i = self.index - 2;
        }
        let mut is_first = true;
        write!(f, "[")?;
        while i < self.index + 3 {
            let Some(elem) = self.tokens.get(i) else {
                break
            };
            if is_first {
                is_first = false;
            } else {
                write!(f, ", ")?;
            }
            if i == self.index {
                write!(f, "<{i}>:{:?}", elem)?;
            } else {
                write!(f, "{i}:{:?}", elem)?;
            }
            i += 1
        }
        write!(f, "]")?;
        Ok(())
    }
}

pub fn parse_str(src_pth: PathBuf, code: &str) -> Result<Ast, SteelErr> {
    let tokens = tokenize(src_pth, code)?;
    dbg!(&tokens); //TODO @mark:
    let tokens = Cursor::new(tokens);
    let prog = Ast {
        blocks: parse_blocks(tokens)?,
    };
    dbg!(&prog); //TODO @mark:
    Ok(prog)
}

#[inline]
fn parse_blocks(mut tokens: Cursor) -> Result<Vec<Block>, SteelErr> {
    let mut blocks = Vec::new();
    loop {
        if let Ok((expr, tok)) = parse_expression(tokens.fork()) {
            tokens = tok;
            blocks.push(Block::Expression(expr));
            let closer_cnt = tokens.take_while(|tok| matches!(tok, Token::Semicolon))
                + tokens.take_while(|tok| matches!(tok, Token::Newline));
            //TODO @mark: only expressions need this right? not e.g. struct declarations, but maybe imports...
            if closer_cnt == 0 {
                todo!("error: no closer (semicolon or newline) after expression at {tokens:?}")
            }
            continue;
        }
        break; //TODO @mark:
    }
    Ok(blocks)
}

fn parse_expression(mut tokens: Cursor) -> ParseRes<Expr> {
    parse_addsub(tokens)
}

//TODO @mark: use:
fn parse_identifier(orig_tokens: Cursor) -> ParseRes<Identifier> {
    let mut tokens = orig_tokens.fork();
    let Some(Token::Identifier(iden)) = tokens.take() else {
        todo!("no match but perhaps not an error") //  orig_tokens
    };
    Ok((iden.clone(), tokens))
}

//TODO @mark: use:
fn parse_type_use(mut tokens: Cursor) -> ParseRes<Identifier> {
    //TODO @mark: for now
    parse_identifier(tokens)
}

#[inline]
fn parse_addsub(orig_tokens: Cursor) -> ParseRes<Expr> {
    eprintln!("start addsub at {:?}", &orig_tokens); //TODO @mark: TEMPORARY! REMOVE THIS!
    let res = parse_binary_op(
        orig_tokens,
        |op| op == OpCode::Add || op == OpCode::Sub,
        parse_muldiv,
    );
    eprintln!("end addsub"); //TODO @mark: TEMPORARY! REMOVE THIS!
    res
}

#[inline]
fn parse_muldiv(orig_tokens: Cursor) -> ParseRes<Expr> {
    parse_binary_op(
        orig_tokens,
        |op| op == OpCode::Mul || op == OpCode::Div,
        parse_scalar,
    )
}

#[inline]
fn parse_binary_op(
    orig_tokens: Cursor,
    is_op: fn(OpCode) -> bool,
    next: impl Fn(Cursor) -> ParseRes<Expr>,
) -> ParseRes<Expr> {
    let (mut expr, mut tokens) = next(orig_tokens)?;
    loop {
        let Some(Token::OpSymbol(op)) = tokens.peek() else {
            trace!("trying to parse operator, instead got {:?}", tokens.peek());
            return Ok((expr, tokens))
        };
        let op = *op;
        if !is_op(op) {
            trace!("got a different operator than expected {:?}", tokens.peek());
            return Ok((expr, tokens));
        }
        tokens.take();
        trace!("parsed operator {:?}", op);
        let (right, mut right_tok) = next(tokens)?;
        //TODO @mark: how to make the error message say something like "expected a muldiv expression because of +" but readable?
        expr = Expr::BinOp(op, Box::new(expr), Box::new(right));
        tokens = right_tok;
    }
}

#[inline]
fn parse_scalar(orig_tokens: Cursor) -> ParseRes<Expr> {
    let mut tokens = orig_tokens.fork();
    match tokens.take() {
        Some(Token::Identifier(iden)) => {
            trace!("parsed identifier {:?}", iden);
            Ok((Expr::Iden(iden.clone()), tokens))
        }
        Some(Token::Number(num)) => {
            trace!("parsed number {:?}", *num);
            Ok((Expr::Num(*num), tokens))
        }
        Some(Token::Text(txt)) => {
            trace!("parsed text '{:?}'", txt);
            Ok((Expr::Text(txt.clone()), tokens))
        }
        _ => parse_parenthesised(orig_tokens),
    }
}

#[inline]
fn parse_parenthesised(orig_tokens: Cursor) -> ParseRes<Expr> {
    let mut tokens = orig_tokens.fork();
    if let Some(Token::ParenthesisOpen) = tokens.take() {
        trace!("start parsing parenthesised group");
        let (expr, mut expr_tokens) = parse_expression(tokens)?;
        if let Some(Token::ParenthesisClose) = expr_tokens.take() {
            trace!("parsed parenthesised group {:?}", &expr);
            return Ok((expr, expr_tokens));
        } else {
            todo!("expected closing parenthesis at {expr_tokens:?}")
        }
    }
    debug!("tried all parsing rules but nothing matched at {tokens:?}");
    Err(SteelErr::ParseErr {
        file: Default::default(),
        line: 0,
        msg: "todo no rules matched".to_string(),
    }) //TODO @mark:
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_arithmetic() {
        let (expr, cur) = parse_expression(Cursor::new(vec![
            Token::ParenthesisOpen,
            Token::Number(1.0),
            Token::ParenthesisClose,
            Token::OpSymbol(OpCode::Add),
            Token::Number(2.0),
            Token::OpSymbol(OpCode::Add),
            Token::Number(3.0),
        ]))
        .unwrap();
        assert_eq!(
            expr,
            Expr::BinOp(
                OpCode::Add,
                Box::new(Expr::BinOp(
                    OpCode::Add,
                    Box::new(Expr::Num(1.0)),
                    Box::new(Expr::Num(2.0))
                )),
                Box::new(Expr::Num(3.0))
            )
        );
        assert_eq!(cur.index, 7);
    }
}
