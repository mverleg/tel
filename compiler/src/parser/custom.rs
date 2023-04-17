//TODO @mark: delete either this if lalrpop turns out better, or delete lalrpop if going this way

use ::std::fmt;
use ::std::path::PathBuf;
use ::std::rc::Rc;
use ::std::sync::LazyLock;

use ::itertools::Itertools;
use ::regex::Regex;

use ::steel_api::log::debug;
use ::steel_api::log::trace;

use crate::ast::AST;
use crate::ast::Expr;
use crate::ast::Block;
use crate::ast::OpCode;
use crate::parser::lexer::Token;
use crate::parser::lexer::Token::OpSymbol;
use crate::parser::lexer::tokenize;
use crate::SteelErr;
use crate::SteelErr::ParseErr;

#[derive(Debug)]
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

    fn take_while(&mut self, condition: impl Fn(&Token) -> bool) -> usize {
        let mut cnt = 0;
        loop {
            let Some(val) = self.tokens.get(self.index) else {
                break
            };
            if ! condition(val) {
                break
            }
            cnt += 1;
        }
        self.index += cnt;
        cnt
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
        if let Some((expr, tok)) = parse_expression(tokens.fork()) {
            tokens = tok;
            blocks.push(Block::Expression(expr));
            let closer_cnt = tokens.take_while(|tok| matches!(tok, Token::Semicolon)) +
                tokens.take_while(|tok| matches!(tok, Token::Newline));
            //TODO @mark: only expressions need this right? not e.g. struct declarations, but maybe imports...
            if closer_cnt == 0 {
                todo!("error: no closer (semicolon or newline) after expression")
            }
        }
        todo!()
    }
    Ok(blocks)
}

fn parse_expression(mut tokens: Cursor) -> Result<(Expr, Tokens), SteelErr> {
    match tokens.take() {
        Some(Token::ParenthesisOpen) => {
            let Ok((expr, tok)) = parse_expression(tokens) else {
                debug!("tried to parse '('parenthesized')' group but did not find an expression after '('");
                todo!("report error about missing )")  //TODO @mark:
            };
            if Ok(&Token::ParenthesisClose) != tokens.take() {
                debug!("tried to parse '('parenthesized')' group but did not find closing at the end ')'");
                todo!("report error about missing )")  //TODO @mark:
            }
            return Ok((expr, tok))
        },
        Some(Token::Identifier(arg)) => { blocks.push(Block::Expression(parse_expression(&mut tokens)?)); },
        Some(Token::Number(arg)) => { blocks.push(Block::Expression(parse_expression(&mut tokens)?)); },
        Some(Token::Text(arg)) => { blocks.push(Block::Expression(parse_expression(&mut tokens)?)); },
        Some(other) => todo!("error handling for unknown block start {other:?}"),
        None => return None,
    }
    //TODO @mark: fail if there was no semicolon or newline (except ')' or '}' maybe? or maybe just forbid such onelines without ;)
}

