//TODO @mark: delete either this if lalrpop turns out better, or delete lalrpop if going this way

use ::std::fmt;
use ::std::path::PathBuf;
use ::std::rc::Rc;
use ::std::sync::LazyLock;

use ::itertools::Itertools;
use ::regex::Regex;

use ::steel_api::log::debug;
use ::steel_api::log::trace;

use crate::ast::{AST, Expr};
use crate::ast::Block;
use crate::ast::OpCode;
use crate::parser::lexer::Token;
use crate::parser::lexer::Token::OpSymbol;
use crate::parser::lexer::tokenize;
use crate::SteelErr;

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
    //TODO @mark:
    let mut blocks = Vec::new();
    //TODO @mark: parse multiple blocks
    match token {
        other => {
            blocks.push(Block::Expression(parse_expression(tokens.fork())?));
            //TODO @mark: fail if no newline or ;
        }
    }
    Ok(blocks)
}

fn parse_expression(mut tokens: Cursor) -> Result<Expr, SteelErr> {
    unimplemented!()
}

