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

pub fn tokenize(src_pth: PathBuf, full_code: &str) -> Result<Vec<Token>, SteelErr> {
    let mut tokens = Vec::new();
    let mut ix = 0;
    while ix < full_code.len() {
        //TODO @mark: drop '...\n' continuations
        let code = &full_code[ix..];
        eprintln!("ix={ix} ch='{}'", code.chars().next().unwrap());  //TODO @mark: TEMPORARY! REMOVE THIS!
        if let Some(caps) = RE.parenthesis_open_re.captures_iter(code).next() {
            let cap = caps.get(0).unwrap().as_str();
            let token = Token::ParenthesisOpen;
            eprintln!("match {token:?} in '{cap}' from {ix} to {}", ix + cap.len());
            //TODO @mark: change to trace ^
            tokens.push(token);
            ix += cap.len();
            debug_assert!(cap.len() > 0);
            continue;
        }
        if let Some(caps) = RE.parenthesis_close_re.captures_iter(code).next() {
            let cap = caps.get(0).unwrap().as_str();
            let token = Token::ParenthesisClose;
            eprintln!("match {token:?} in '{cap}' from {ix} to {}", ix + cap.len());
            //TODO @mark: change to trace ^
            tokens.push(token);
            ix += cap.len();
            debug_assert!(cap.len() > 0);
            continue;
        }
        if let Some(caps) = RE.op_symbol_re.captures_iter(code).next() {
            let cap = caps.get(0).unwrap().as_str();
            eprintln!("sym cap = '{cap}'");  //TODO @mark: TEMPORARY! REMOVE THIS!
            let sym = caps.get(1).unwrap().as_str();
            let token = Token::OpSymbol(match cap {
                "+" => OpCode::Add,
                "-" => OpCode::Sub,
                "*" => OpCode::Mul,
                "/" => OpCode::Div,
                _ => unreachable!(),
            });
            eprintln!("match {token:?} in '{cap}' from {ix} to {}", ix + cap.len());
            tokens.push(token);
            //TODO @mark: change to trace ^
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
    fn handle_non_ascii_strings() {
        let tokens = tokenize(PathBuf::from("test"), "\"你好\"");
        assert_eq!(tokens, Ok(vec![Token::ParenthesisOpen, Token::ParenthesisClose]));
    }

    #[test]
    fn simple_arithmetic() {
        let tokens = tokenize(PathBuf::from("test"), "(3) + (4 / 2)");
        assert_eq!(tokens, Ok(vec![Token::ParenthesisOpen, Token::ParenthesisClose]));
    }
}
