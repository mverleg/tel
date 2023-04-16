//TODO @mark: delete either this if lalrpop turns out better, or delete lalrpop if going this way

use ::std::path::PathBuf;
use ::std::sync::LazyLock;

use ::regex::Regex;
use steel_api::log::{debug, trace};

use crate::ast::AST;
use crate::ast::OpCode;
use crate::SteelErr;

pub fn parse_str(src_pth: PathBuf, code: &str) -> Result<AST, SteelErr> {
    let tokens = tokenize(src_pth, code)?;
    unimplemented!()  //TODO @mark:
}

#[derive(Debug)]
pub enum Token {
    ParenthesisOpen,
    ParenthesisClose,
    Number(f64),
    OpSymbol(OpCode),
}

struct TokenizerRegexes {
    parenthesis_open_re: Regex,
    parenthesis_close_re: Regex,
}

static RE: LazyLock<TokenizerRegexes> = LazyLock::new(|| {
    debug!("start compilign regexes for tokenizer");
    let re = TokenizerRegexes {
        parenthesis_open_re: Regex::new(r"\s*\([\s\n\r]*").unwrap(),
        parenthesis_close_re: Regex::new(r"\s*\)[\s\n\r]*").unwrap(),
    };
    debug!("finished compilign regexes for tokenizer");
    re
});

pub fn tokenize(src_pth: PathBuf, code: &str) -> Result<Vec<Token>, SteelErr> {
    let mut tokens = Vec::new();
    let mut ix = 0;
    while ix < code.len() {
        if let Some(cap) = RE.parenthesis_open_re.captures_iter(&code[ix..]).next() {
            tokens.push(Token::ParenthesisOpen);
            trace!("match {:?} from {ix} to {}", tokens.last().unwrap(), ix + cap.len());
            ix += cap.len();
            break;
        }
        if let Some(cap) = RE.parenthesis_close_re.captures_iter(&code[ix..]).next() {
            trace!("match {:?} from {ix} to {}", tokens.last().unwrap(), ix + cap.len());
            tokens.push(Token::ParenthesisClose);
            ix += cap.len();
            break;
        }
        unreachable!("unexpected end of input at #{ix} ('{}')", code[ix..].chars().next().unwrap())
    }
    unimplemented!()
}

#[cfg(test)]
mod tokens {
    use super::*;

    #[test]
    fn allow_whitespace_after_open_parenthesis() {
        let tokens = tokenize(PathBuf::from("test"), "(\n)");
        assert_eq!(tokens, Ok(vec![Tokens::ParenthesisOpen, Tokens::ParenthesisClose]));
    }
}
