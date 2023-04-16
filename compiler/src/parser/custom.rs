//TODO @mark: delete either this if lalrpop turns out better, or delete lalrpop if going this way

use ::std::path::PathBuf;
use ::std::sync::LazyLock;

use ::regex::Regex;
use itertools::Itertools;
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
    OpSymbol(OpCode),
    Number(f64),
}

struct ParenthesisOpenTokenizer(Regex);

trait Tokenizer {
    fn regex(&self) -> &Regex;

    fn capture_handler(&self, cap_group: Option<&str>) -> Token;
}

impl Tokenizer for ParenthesisOpenTokenizer {
    fn regex(&self) -> &Regex {
        &self.0
    }

    fn capture_handler(&self, cap_group: Option<&str>) -> Token {
        todo!()
    }
}

//TODO @mark: pity about dyn, see if it gets optimized
static RE: LazyLock<[dyn Tokenizer; 1]> = LazyLock::new(|| {
    debug!("start creating tokenizers (compiling regexes)");
    let tokenizers = [
        ParenthesisOpenTokenizer(Regex::new(r"^\s*\(\s*").unwrap()),
    ];
    // let re = Tokenizers {
    //     parenthesis_open_re: Regex::new(r"^\s*\(\s*").unwrap(),
    //     parenthesis_close_re: Regex::new(r"^\s*\)[ \t]*").unwrap(),
    //     op_symbol_re: Regex::new(r"^\s*([*+\-/])\s*").unwrap(),
    //     number_re: Regex::new(r"^\s*(-?[0-9]+(?:\.[0-9]+)?)[ \t]*").unwrap(),
    //     //TODO @mark: no exponential notation yet
    // };
    debug!("finished creating tokenizers (compiling regexes)");
    tokenizers
});

pub fn tokenize(src_pth: PathBuf, full_code: &str) -> Result<Vec<Token>, SteelErr> {
    let mut tokens = Vec::new();
    let mut ix = 0;
    while ix < full_code.len() {
        //TODO @mark: drop '...\n' continuations
        let code = &full_code[ix..];
        eprintln!("ix={ix} code='{}'", code.chars().take(40).join(""));  //TODO @mark: TEMPORARY! REMOVE THIS!
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
            tokens.push(token);
            ix += cap.len();
            debug_assert!(cap.len() > 0);
            continue;
        }
        if let Some(caps) = RE.op_symbol_re.captures_iter(code).next() {
            let cap = caps.get(0).unwrap().as_str();
            let sym = caps.get(1).unwrap().as_str();
            eprintln!("sym cap = '{sym}'");  //TODO @mark: TEMPORARY! REMOVE THIS!
            let token = Token::OpSymbol(match sym {
                "+" => OpCode::Add,
                "-" => OpCode::Sub,
                "*" => OpCode::Mul,
                "/" => OpCode::Div,
                _ => unreachable!(),
            });
            eprintln!("match {token:?} in '{cap}' from {ix} to {}", ix + cap.len());
            tokens.push(token);
            ix += cap.len();
            debug_assert!(cap.len() > 0);
            continue;
        }
        if let Some(caps) = RE.number_re.captures_iter(code).next() {
            let cap = caps.get(0).unwrap().as_str();
            let num_repr = caps.get(1).unwrap().as_str();
            eprintln!("num cap = '{num_repr}'");  //TODO @mark: TEMPORARY! REMOVE THIS!
            let token = match num_repr.parse() {
                Ok(num) => Token::Number(num),
                Err(_err) => unimplemented!(),  //TODO @mark: error handling (e.g. too large nr? most invalid input is handled by regex)
            };
            eprintln!("match {token:?} in '{cap}' from {ix} to {}", ix + cap.len());
            tokens.push(token);
            ix += cap.len();
            debug_assert!(cap.len() > 0);
            continue;
        }
        unreachable!("unexpected end of input at #{ix} ('{}')", code.chars().next().unwrap())
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
        assert_eq!(tokens, Ok(vec![Token::ParenthesisOpen, Token::Number(3.), Token::ParenthesisClose, Token::OpSymbol(OpCode::Add),
                Token::ParenthesisOpen, Token::Number(4.), Token::OpSymbol(OpCode::Div), Token::Number(2.), Token::ParenthesisClose]));
    }
}
