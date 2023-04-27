//TODO @mark: delete either this if lalrpop turns out better, or delete lalrpop if going this way

use ::std::fmt;
use ::std::path::PathBuf;
use ::std::rc::Rc;
use ::std::sync::LazyLock;
use std::fmt::Formatter;

use ::itertools::Itertools;
use ::regex::Regex;

use ::steel_api::log::debug;
use ::steel_api::log::trace;

use crate::ast::AST;
use crate::ast::Block;
use crate::ast::Block::Expression;
use crate::ast::Expr;
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
        Cursor { index: 0, tokens: Rc::new(tokens) }
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
        if ! condition(val) {
            return None
        }
        Some(val)
    }

    fn take_while(&mut self, condition: fn(&Token) -> bool) -> usize {
        let mut cnt = 0;
        loop {
            if self.take_if(condition).is_none() {
                break
            }
            cnt += 1;
        }
        self.index += cnt;
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

pub fn parse_str(src_pth: PathBuf, code: &str) -> Result<AST, SteelErr> {
    let tokens = Cursor::new(tokenize(src_pth, code)?);
    dbg!(&tokens);  //TODO @mark:
    let prog = AST { blocks: parse_blocks(tokens)? };
    dbg!(&prog);  //TODO @mark:
    Ok(prog)
}

#[inline]
fn parse_blocks(mut tokens: Cursor) -> Result<Vec<Block>, SteelErr> {
    let mut blocks = Vec::new();
    loop {
        if let Ok((expr, tok)) = parse_expression(tokens.fork()) {
            tokens = tok;
            blocks.push(Block::Expression(expr));
            let closer_cnt = tokens.take_while(|tok| matches!(tok, Token::Semicolon)) +
                tokens.take_while(|tok| matches!(tok, Token::Newline));
            //TODO @mark: only expressions need this right? not e.g. struct declarations, but maybe imports...
            if closer_cnt == 0 {
                todo!("error: no closer (semicolon or newline) after expression at {tokens:?}")
            }
        }
        break  //TODO @mark:
    }
    Ok(blocks)
}

fn parse_expression(mut tokens: Cursor) -> ParseRes<Expr> {
    parse_addsub(tokens)
    // match tokens.take() {
    //     Some(Token::ParenthesisOpen) => {
    //         let Ok((expr, mut tok)) = parse_expression(tokens) else {
    //             debug!("tried to parse '('parenthesized')' group but did not find an expression after '('");
    //             todo!("report error about missing )")  //TODO @mark:
    //         };
    //         if Some(&Token::ParenthesisClose) != tok.take() {
    //             debug!("tried to parse '('parenthesized')' group but did not find closing at the end ')'");
    //             todo!("report error about missing )")  //TODO @mark:
    //         }
    //         return Ok((expr, tok))
    //     },
    //     Some(Token::Identifier(iden)) => {
    //         //TODO @mark: probably move this repetition down
    //         let iden = Expr::Iden(iden.clone());
    //         return if let Ok((expr, tok)) = parse_binary_op(tokens.fork(), iden) {
    //             Ok((expr, tok))
    //         } else {
    //             todo!("report error - not sure how to get here, failed binary already falls back to left, which is available")  //TODO @mark:
    //         };
    //     },
    //     Some(Token::Number(num)) => {
    //         let num = Expr::Num(*num);
    //         return if let Ok((expr, tok)) = parse_binary_op(tokens.fork(), num) {
    //             Ok((expr, tok))
    //         } else {
    //             todo!("report error - not sure how to get here, failed binary already falls back to left, which is available")  //TODO @mark:
    //         };
    //     },
    //     Some(Token::Text(txt)) => {
    //         let txt = Expr::Text(txt.clone());
    //         return if let Ok((expr, tok)) = parse_binary_op(tokens.fork(), txt) {
    //             Ok((expr, tok))
    //         } else {
    //             todo!("report error - not sure how to get here, failed binary already falls back to left, which is available")  //TODO @mark:
    //         };
    //     },
    //     Some(other) => todo!("error handling for unknown block start {other:?}"),
    //     None => todo!("handle not finding an expression"),
    // }
    //TODO @mark: fail if there was no semicolon or newline (except ')' or '}' maybe? or maybe just forbid such onelines without ;)
}

#[inline]
fn parse_addsub(orig_tokens: Cursor) -> ParseRes<Expr> {
    parse_binary_op(
        orig_tokens,
        |op| op == OpCode::Add || op == OpCode::Sub,
        parse_muldiv)
}

#[inline]
fn parse_muldiv(orig_tokens: Cursor) -> ParseRes<Expr> {
    parse_binary_op(
        orig_tokens,
        |op| op == OpCode::Mul || op == OpCode::Div,
        parse_scalar)
}

#[inline]
fn parse_binary_op(
    orig_tokens: Cursor,
    is_op: fn(OpCode) -> bool,
    next: impl Fn(Cursor) -> ParseRes<Expr>,
) -> ParseRes<Expr> {
    let (left, mut tokens) = next(orig_tokens)?;
    let Some(Token::OpSymbol(op)) = tokens.peek() else {
        trace!("trying to parse operator, instead got {:?}", tokens.peek());
        return Ok((left, tokens))
    };
    let op = *op;
    if ! is_op(op) {
        trace!("got a different operator than expected {:?}", tokens.peek());
        return Ok((left, tokens))
    }
    tokens.take();
    trace!("parsed operator {:?}", op);
    let (right, mut right_tok) = next(tokens)?;
    //TODO @mark: how to make the error message say something like "expected a muldiv expression because of +" but readable?
    Ok((Expr::BinOp(op, Box::new(left), Box::new(right)), right_tok))
}

#[inline]
fn parse_scalar(orig_tokens: Cursor) -> ParseRes<Expr> {
    let mut tokens = orig_tokens.fork();
    match tokens.take() {
        Some(Token::Identifier(iden)) => {
            trace!("parsed identifier {:?}", iden);
            Ok((Expr::Iden(iden.clone()), tokens))
        },
        Some(Token::Number(num)) => {
            trace!("parsed number {:?}", *num);
            Ok((Expr::Num(*num), tokens))
        },
        Some(Token::Text(txt)) => {
            trace!("parsed text '{:?}'", txt);
            Ok((Expr::Text(txt.clone()), tokens))
        },
        _ => parse_parenthesised(orig_tokens)
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
            return Ok((expr, expr_tokens))
        } else {
            todo!("expected closing parenthesis at {expr_tokens:?}")
        }
    }
    debug!("tried all parsing rules but nothing matched at {tokens:?}");
    Err(SteelErr::ParseErr {
        file: Default::default(),
        line: 0,
        msg: "todo no rules matched".to_string(),
    })  //TODO @mark:
}

// //TODO @mark: this is addsub, rename?
// fn parse_binary_op(mut tokens: Cursor, left: Expr) -> ParseRes<Expr> {
//     let mut expr = left;
//     loop {
//         let op_code;
//         match tokens.peek() {
//             Some(Token::OpSymbol(op)) if op_code == OpCode::Add || op_code == OpCode::Sub => op_code = *op,
//             _ => return parse_bin_muldiv(tokens.fork(), expr),
//         }
//         tokens.take();
//         let (right, right_tokens) = parse_bin_muldiv(tokens.fork(), expr)?;
//         tokens = right_tokens;
//         expr = Expr::BinOp(op_code, Box::new(expr.clone()), Box::new(right));
//         //TODO @mark: is there a way to avoid cloning?
//     }
// }
//
// fn parse_bin_muldiv(mut tokens: Cursor, left: Expr) -> ParseRes<Expr> {
//     //TODO @mark: TEMPORARY! REMOVE THIS!
//     let mut expr = left;
//     loop {
//         let op_code;
//         match tokens.peek() {
//             Some(Token::OpSymbol(op)) if op_code == OpCode::Mul || op_code == OpCode::Div => op_code = *op,
//             _ => return parse_bin_muldiv(tokens.fork(), &expr),
//         }
//         tokens.take();
//         let (right, right_tokens) = parse_bin_muldiv(tokens.fork(), &expr)?;
//         tokens = right_tokens;
//         expr = Expr::BinOp(op_code, Box::new(expr.clone()), Box::new(right));
//         //TODO @mark: is there a way to avoid cloning?
//     }
// }
//
// fn parse_binary_op_with(mut tokens: Cursor, left: Expr, op_condition: fn(&OpCode) -> bool, right_parser: impl FnMut(Cursor, Expr) -> ParseRes<Expr>) -> ParseRes<Expr> {
//     //TODO @mark: TEMPORARY! REMOVE THIS!
//     let mut expr = left;
//     loop {
//         let op_code;
//         match tokens.peek() {
//             Some(Token::OpSymbol(op)) if op_condition(op_code) => op_code = *op,
//             _ => return parse_bin_muldiv(tokens.fork(), &expr),
//         }
//         tokens.take();
//         let (right, right_tokens) = parse_bin_muldiv(tokens.fork(), &expr)?;
//         tokens = right_tokens;
//         expr = Expr::BinOp(op_code, Box::new(expr.clone()), Box::new(right));
//         //TODO @mark: is there a way to avoid cloning?
//     }
// }
