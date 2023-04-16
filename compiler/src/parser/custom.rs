//TODO @mark: delete either this if lalrpop turns out better, or delete lalrpop if going this way

use ::std::path::PathBuf;
use ::std::sync::LazyLock;

use ::regex::Regex;
use steel_api::log::{debug, trace};

use crate::ast::AST;
use crate::ast::OpCode;
use crate::parser::custom::Token::OpSymbol;
use crate::SteelErr;

pub fn parse_str(src_pth: PathBuf, code: &str) -> Result<AST, SteelErr> {
    let tokens = tokenize(src_pth, code)?;
    unimplemented!()  //TODO @mark:
}

#[derive(Debug, PartialEq)]
pub enum Token {
    ParenthesisOpen,
    ParenthesisClose,
    Number(f64),
    OpSymbol(OpCode),
}

struct TokenizerRegexes {
    parenthesis_open_re: Regex,
    parenthesis_close_re: Regex,
    op_symbol_re: Regex,
}

static RE: LazyLock<TokenizerRegexes> = LazyLock::new(|| {
    debug!("start compilign regexes for tokenizer");
    let re = TokenizerRegexes {
        parenthesis_open_re: Regex::new(r"\s*\(\s*").unwrap(),
        parenthesis_close_re: Regex::new(r"\s*\)[ \t]*").unwrap(),
        op_symbol_re: Regex::new(r"\s*([*+\-/])\s*").unwrap(),
    };
    debug!("finished compilign regexes for tokenizer");
    re
});

pub fn tokenize(src_pth: PathBuf, code: &str) -> Result<Vec<Token>, SteelErr> {
    let mut tokens = Vec::new();
    let mut ix = 0;
    while ix < code.len() {
        eprintln!("ix={ix}");  //TODO @mark: TEMPORARY! REMOVE THIS!
        if let Some(caps) = RE.parenthesis_open_re.captures_iter(&code[ix..]).next() {
            let cap = caps.get(0).unwrap().as_str();
            tokens.push(Token::ParenthesisOpen);
            trace!("match {:?} from {ix} to {}", tokens.last().unwrap(), ix + cap.len());
            ix += cap.len();
            debug_assert!(cap.len() > 0);
            continue;
        }
        if let Some(caps) = RE.parenthesis_close_re.captures_iter(&code[ix..]).next() {
            let cap = caps.get(0).unwrap().as_str();
            tokens.push(Token::ParenthesisClose);
            trace!("match {:?} from {ix} to {}", tokens.last().unwrap(), ix + cap.len());
            ix += cap.len();
            debug_assert!(cap.len() > 0);
            continue;
        }
        if let Some(caps) = RE.op_symbol_re.captures_iter(&code[ix..]).next() {
            let cap = caps.get(0).unwrap().as_str();
            let sym = caps.get(1).unwrap().as_str();
            tokens.push(Token::OpSymbol(match cap {
                "+" => OpCode::Add,
                "-" => OpCode::Sub,
                "*" => OpCode::Mul,
                "/" => OpCode::Div,
                _ => unreachable!(),
            }));
            trace!("match {:?} from {ix} to {}", tokens.last().unwrap(), ix + cap.len());
            ix += cap.len();
            debug_assert!(cap.len() > 0);
            continue;
        }
        unreachable!("unexpected end of input at #{ix} ('{}')", code[ix..].chars().next().unwrap())
    }
    Ok(tokens)
}

#[cfg(test)]
mod tokens {
    use super::*;

    #[test]
    fn allow_whitespace_after_open_parenthesis() {
        let tokens = tokenize(PathBuf::from("test"), "(\n)");
        assert_eq!(tokens, Ok(vec![Token::ParenthesisOpen, Token::ParenthesisClose]));
    }

    #[test]
    fn simple_arithmetic() {
        let tokens = tokenize(PathBuf::from("test"), "(3) + (4 / 2)");
        assert_eq!(tokens, Ok(vec![Token::ParenthesisOpen, Token::ParenthesisClose]));
    }
}
