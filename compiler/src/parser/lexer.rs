//TODO @mark: delete either this if lalrpop turns out better, or delete lalrpop if going this way

use ::std::fmt;
use ::std::path::PathBuf;
use ::std::sync::LazyLock;

use ::itertools::Itertools;
use ::regex::Regex;

use ::steel_api::log::debug;
use ::steel_api::log::trace;

use crate::ast::AST;
use crate::ast::OpCode;
use crate::parser::lexer::Token::OpSymbol;
use crate::SteelErr;

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    ParenthesisOpen,
    ParenthesisClose,
    OpSymbol(OpCode),
    Number(f64),
    Text(String),
}

trait Tokenizer: fmt::Debug + Send + Sync {
    #[inline]
    fn regex(&self) -> &Regex;

    #[inline]
    fn token_for(&self, cap_group: Option<&str>) -> Option<Token>;
}

#[derive(Debug)]
struct FixedTokenTokenizer(Regex, Option<Token>);

impl FixedTokenTokenizer {
    fn new_parenthesis_open() -> Box<Self> {
        Box::new(FixedTokenTokenizer(Regex::new(r"^\s*\(\s*").unwrap(), Some(Token::ParenthesisOpen)))
    }

    fn new_parenthesis_close() -> Box<Self> {
        Box::new(FixedTokenTokenizer(Regex::new(r"^\s*\)[ \t]*").unwrap(), Some(Token::ParenthesisClose)))
    }

    fn new_leftover_whitespace() -> Box<Self> {
        Box::new(FixedTokenTokenizer(Regex::new(r"^\s*$").unwrap(), None))
    }
}

impl Tokenizer for FixedTokenTokenizer {
    fn regex(&self) -> &Regex {
        &self.0
    }

    fn token_for(&self, ignored: Option<&str>) -> Option<Token> {
        debug_assert!(ignored.is_none(), "no capture group expected for this tokenizer (got {ignored:?})");
        self.1.clone()
    }
}

#[derive(Debug)]
struct CommentTokenizer(Regex);

impl CommentTokenizer {
    fn new() -> Box<Self> {
        Box::new(CommentTokenizer(Regex::new(r"^\s*#[^\n\r]+(?:$|\n|\r)+").unwrap()))
    }
}

impl Tokenizer for CommentTokenizer {
    fn regex(&self) -> &Regex {
        &self.0
    }

    fn token_for(&self, comment: Option<&str>) -> Option<Token> {
        debug_assert!(comment.is_none(), "no capture group expected for comment");
        None
    }
}

#[derive(Debug)]
struct OpSymbolTokenizer(Regex);

impl OpSymbolTokenizer {
    fn new() -> Box<Self> {
        Box::new(OpSymbolTokenizer(Regex::new(r"^\s*([*+\-/])\s*").unwrap()))
    }
}

impl Tokenizer for OpSymbolTokenizer {
    fn regex(&self) -> &Regex {
        &self.0
    }

    fn token_for(&self, op_sym: Option<&str>) -> Option<Token> {
        Some(Token::OpSymbol(match op_sym.expect("regex group must always capture once") {
            "+" => OpCode::Add,
            "-" => OpCode::Sub,
            "*" => OpCode::Mul,
            "/" => OpCode::Div,
            _ => unreachable!(),
        }))
    }
}

#[derive(Debug)]
struct NumberTokenizer(Regex);

impl NumberTokenizer {
    fn new() -> Box<Self> {
        Box::new(NumberTokenizer(Regex::new(r"^\s*(-?[0-9]+(?:\.[0-9]+)?)[ \t]*").unwrap()))
    }
}

impl Tokenizer for NumberTokenizer {
    fn regex(&self) -> &Regex {
        &self.0
    }

    fn token_for(&self, num_repr: Option<&str>) -> Option<Token> {
        match num_repr.expect("regex group must always capture once").parse() {
            Ok(num) => Some(Token::Number(num)),
            Err(_err) => unimplemented!(),  //TODO @mark: error handling (e.g. too large nr? most invalid input is handled by regex)
        }
    }
}

#[derive(Debug)]
struct TextTokenizer(Regex);

impl TextTokenizer {
    fn new() -> Box<Self> {
        //TODO @mark: quote escaping not allowed yet
        Box::new(TextTokenizer(Regex::new("^\\s*\"([^\"\n]*)\"[ \t]*").unwrap()))
    }
}

impl Tokenizer for TextTokenizer {
    fn regex(&self) -> &Regex {
        &self.0
    }

    fn token_for(&self, text: Option<&str>) -> Option<Token> {
        Some(Token::Text(text.expect("regex group must always capture once").to_owned()))
    }
}

//TODO @mark: pity about dyn, see if it gets optimized
static TOKENIZERS: LazyLock<[Box<dyn Tokenizer>; 7]> = LazyLock::new(|| {
    debug!("start creating tokenizers (compiling regexes)");
    let tokenizers: [Box<dyn Tokenizer>; 7] = [
        CommentTokenizer::new(),
        FixedTokenTokenizer::new_parenthesis_open(),
        FixedTokenTokenizer::new_parenthesis_close(),
        OpSymbolTokenizer::new(),
        NumberTokenizer::new(),
        TextTokenizer::new(),
        FixedTokenTokenizer::new_leftover_whitespace(),
    ];
    debug!("finished creating {} tokenizers (compiling regexes)", tokenizers.len());
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
            if let Some(token) = tokenizer.token_for(grp) {
                eprintln!("match {token:?} in '{mtch}' from {ix} to {}", ix + mtch.len());
                //TODO @mark: change to trace ^
                tokens.push(token);
            } else {
                eprintln!("matched tokenizer {tokenizer:?} from {ix} to {} but it produced no token", ix + mtch.len());
                //TODO @mark: change to trace ^
            }
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
    fn handle_non_ascii_strings_and_comments() {
        let tokens = tokenize(PathBuf::from("test"), "\"你好\"# 你好");
        assert_eq!(tokens, Ok(vec![Token::Text("你好".to_owned())]));
    }

    #[test]
    fn simple_arithmetic() {
        let tokens = tokenize(PathBuf::from("test"), "(3) + (4 / 2)");
        assert_eq!(tokens, Ok(vec![Token::ParenthesisOpen, Token::Number(3.), Token::ParenthesisClose, Token::OpSymbol(OpCode::Add),
                Token::ParenthesisOpen, Token::Number(4.), Token::OpSymbol(OpCode::Div), Token::Number(2.), Token::ParenthesisClose]));
    }

    #[test]
    fn skip_leftover_whitespace_at_end() {
        let tokens = tokenize(PathBuf::from("test"), "0\n\n");
        assert_eq!(tokens, Ok(vec![Token::Number(0.)]));
    }
}
