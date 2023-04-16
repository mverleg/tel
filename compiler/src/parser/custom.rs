//TODO @mark: delete either this if lalrpop turns out better, or delete lalrpop if going this way

use ::std::path::PathBuf;
use ::std::sync::LazyLock;
use std::fmt;

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

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    ParenthesisOpen,
    ParenthesisClose,
    OpSymbol(OpCode),
    Number(f64),
}

trait Tokenizer: fmt::Debug + Send + Sync {
    #[inline]
    fn regex(&self) -> &Regex;

    #[inline]
    fn token_for(&self, cap_group: Option<&str>) -> Token;
}

#[derive(Debug)]
struct FixedTokenTokenizer(Regex, Token);

impl Tokenizer for FixedTokenTokenizer {
    fn regex(&self) -> &Regex {
        &self.0
    }

    fn token_for(&self, ignored: Option<&str>) -> Token {
        debug_assert!(ignored.is_none(), "no capture group expected for this tokenizer (got {ignored:?})");
        self.1.clone()
    }
}

#[derive(Debug)]
struct OpSymbolTokenizer(Regex);

impl Tokenizer for OpSymbolTokenizer {
    fn regex(&self) -> &Regex {
        &self.0
    }

    fn token_for(&self, op_sym: Option<&str>) -> Token {
        Token::OpSymbol(match op_sym.expect("regex group must always capture once") {
            "+" => OpCode::Add,
            "-" => OpCode::Sub,
            "*" => OpCode::Mul,
            "/" => OpCode::Div,
            _ => unreachable!(),
        })
    }
}

#[derive(Debug)]
struct NumberTokenizer(Regex);

impl Tokenizer for NumberTokenizer {
    fn regex(&self) -> &Regex {
        &self.0
    }

    fn token_for(&self, num_repr: Option<&str>) -> Token {
        match num_repr.expect("regex group must always capture once").parse() {
            Ok(num) => Token::Number(num),
            Err(_err) => unimplemented!(),  //TODO @mark: error handling (e.g. too large nr? most invalid input is handled by regex)
        }
    }
}

//TODO @mark: pity about dyn, see if it gets optimized
static TOKENIZERS: LazyLock<[Box<dyn Tokenizer>; 4]> = LazyLock::new(|| {
    debug!("start creating tokenizers (compiling regexes)");
    let tokenizers: [Box<dyn Tokenizer>; 4] = [
        Box::new(FixedTokenTokenizer(Regex::new(r"^\s*\(\s*").unwrap(), Token::ParenthesisOpen)),
        Box::new(FixedTokenTokenizer(Regex::new(r"^\s*\)[ \t]*").unwrap(), Token::ParenthesisClose)),
        Box::new(OpSymbolTokenizer(Regex::new(r"^\s*([*+\-/])\s*").unwrap())),
        Box::new(NumberTokenizer(Regex::new(r"^\s*\(\s*").unwrap())),
    ];
    debug!("finished creating tokenizers (compiling regexes)");
    tokenizers
});

pub fn tokenize(src_pth: PathBuf, full_code: &str) -> Result<Vec<Token>, SteelErr> {
    let mut tokens = Vec::new();
    let mut ix = 0;
    'outer: while ix < full_code.len() {
        //TODO @mark: drop '...\n' continuations
        let code = &full_code[ix..];
        eprintln!("ix={ix} code='{}'", code.chars().take(40).join(""));  //TODO @mark: TEMPORARY! REMOVE THIS!
        //TODO @mark: unroll:
        for tokenizer in &*TOKENIZERS {
            eprintln!("ix={ix} tokenizer={tokenizer:?}");  //TODO @mark: TEMPORARY! REMOVE THIS!
            let Some(caps) = tokenizer.regex().captures_iter(code).next() else {
                continue;
            };
            let mtch = caps.get(0).expect("regex group 0 should always match").as_str();
            let grp = caps.get(1).map(|g| g.as_str());
            let token = tokenizer.token_for(grp);
            eprintln!("match {token:?} in '{mtch}' from {ix} to {}", ix + mtch.len());
            //TODO @mark: change to trace ^
            tokens.push(token);
            ix += mtch.len();
            debug_assert!(mtch.len() > 0);
            continue 'outer;
        }
        unreachable!("unexpected end of input at #{ix} ('{}') after {} tokenizers",
                code.chars().next().unwrap(), TOKENIZERS.len())
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
