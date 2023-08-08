//TODO @mark: delete either this if lalrpop turns out better, or delete lalrpop if going this way

use ::std::fmt;
use ::std::path::PathBuf;
use ::std::sync::LazyLock;

use ::itertools::Itertools;
use ::regex::Regex;
use ::smartstring::alias::String as SString;

use ::steel_api::log::debug;
use ::steel_api::log::trace;

use crate::ast::Identifier;
use crate::ast::Keyword;
use crate::ast::OpCode;
use crate::SteelErr;

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    ParenthesisOpen,
    ParenthesisClose,
    Newline,
    Semicolon,
    Colon,
    Keyword(Keyword),
    Identifier(Identifier),
    Assignment(Option<OpCode>),
    OpSymbol(OpCode),
    Number(f64),
    Text(SString),
}

trait Tokenizer: fmt::Debug + Send + Sync {
    fn regex(&self) -> &Regex;

    fn token_for(&self, cap_group: Option<&str>) -> Option<Token>;
}

#[derive(Debug)]
struct FixedTokenTokenizer(Regex, Option<Token>);

impl FixedTokenTokenizer {
    fn new_parenthesis_open() -> Box<Self> {
        Box::new(FixedTokenTokenizer(
            Regex::new(r"^[ \t]*\(\s*").unwrap(),
            Some(Token::ParenthesisOpen),
        ))
    }

    fn new_parenthesis_close() -> Box<Self> {
        Box::new(FixedTokenTokenizer(
            Regex::new(r"^[ \t]*\)[ \t]*").unwrap(),
            Some(Token::ParenthesisClose),
        ))
    }

    fn new_newline() -> Box<Self> {
        Box::new(FixedTokenTokenizer(
            Regex::new(r"^[ \t]*[\n\r]+[ \t]*").unwrap(),
            Some(Token::Newline),
        ))
    }

    fn new_semicolon() -> Box<Self> {
        Box::new(FixedTokenTokenizer(
            Regex::new(r"^[ \t]*;[ \t]*").unwrap(),
            Some(Token::Semicolon),
        ))
    }

    fn new_colon() -> Box<Self> {
        Box::new(FixedTokenTokenizer(
            Regex::new(r"^[ \t]*:[ \t]*").unwrap(),
            Some(Token::Colon),
        ))
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
        debug_assert!(
            ignored.is_none(),
            "no capture group expected for this tokenizer (got {ignored:?})"
        );
        self.1.clone()
    }
}

#[derive(Debug)]
struct CommentTokenizer(Regex);

impl CommentTokenizer {
    fn new() -> Box<Self> {
        Box::new(CommentTokenizer(
            Regex::new(r"^[ \t]*#[^\n\r]+(?:$|\n|\r)+").unwrap(),
        ))
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

static OP_RE: &'static str = r"[*+\-/]";

impl OpSymbolTokenizer {
    fn new() -> Box<Self> {
        Box::new(OpSymbolTokenizer(
            Regex::new(&format!(r"^[ \t]*({})\s*", OP_RE)).unwrap(),
        ))
    }

    fn op_for(op_sym: &str) -> OpCode {
        match op_sym {
            "+" => OpCode::Add,
            "-" => OpCode::Sub,
            "*" => OpCode::Mul,
            "/" => OpCode::Div,
            _ => unreachable!(),
        }
    }
}

impl Tokenizer for OpSymbolTokenizer {
    fn regex(&self) -> &Regex {
        &self.0
    }

    fn token_for(&self, op_sym: Option<&str>) -> Option<Token> {
        Some(Token::OpSymbol(OpSymbolTokenizer::op_for(
            op_sym.expect("regex group must always capture once"))))
    }
}

#[derive(Debug)]
struct AssignmentTokenizer(Regex);

impl AssignmentTokenizer {
    fn new() -> Box<Self> {
        Box::new(AssignmentTokenizer(
            Regex::new(&format!(r"^[ \t]*({})?=\s*", OP_RE)).unwrap(),
        ))
    }
}

impl Tokenizer for AssignmentTokenizer {
    fn regex(&self) -> &Regex {
        &self.0
    }

    fn token_for(&self, op_sym: Option<&str>) -> Option<Token> {
        Some(Token::Assignment(match op_sym {
            None => None,
            Some(op_txt) => Some(OpSymbolTokenizer::op_for(op_txt))
        }))
    }
}

#[derive(Debug)]
struct NumberTokenizer(Regex);

impl NumberTokenizer {
    fn new() -> Box<Self> {
        Box::new(NumberTokenizer(
            Regex::new(r"^[ \t]*(-?[0-9]+(?:\.[0-9]+)?)[ \t]*").unwrap(),
        ))
    }
}

impl Tokenizer for NumberTokenizer {
    fn regex(&self) -> &Regex {
        &self.0
    }

    fn token_for(&self, num_repr: Option<&str>) -> Option<Token> {
        match num_repr
            .expect("regex group must always capture once")
            .parse()
        {
            Ok(num) => Some(Token::Number(num)),
            Err(_err) => unimplemented!(), //TODO @mark: error handling (e.g. too large nr? most invalid input is handled by regex)
        }
    }
}

#[derive(Debug)]
struct TextTokenizer(Regex);

impl TextTokenizer {
    fn new() -> Box<Self> {
        //TODO @mark: quote escaping not allowed yet
        Box::new(TextTokenizer(
            Regex::new("^[ \t]*\"([^\"\n]*)\"[ \t]*").unwrap(),
        ))
    }
}

impl Tokenizer for TextTokenizer {
    fn regex(&self) -> &Regex {
        &self.0
    }

    fn token_for(&self, text: Option<&str>) -> Option<Token> {
        let txt: SString = text.expect("regex group must always capture once").to_owned().into();
        Some(Token::Text(txt))
    }
}

#[derive(Debug)]
struct IdentifierTokenizer(Regex);

impl IdentifierTokenizer {
    fn new() -> Box<Self> {
        Box::new(IdentifierTokenizer(
            Regex::new("^[ \t]*((?:[a-zA-Z]|_[a-zA-Z0-9])[a-zA-Z0-9]*)[ \t]*").unwrap(),
        ))
    }
}

impl Tokenizer for IdentifierTokenizer {
    fn regex(&self) -> &Regex {
        &self.0
    }

    fn token_for(&self, name: Option<&str>) -> Option<Token> {
        Some(Token::Identifier(
            Identifier::new(name.expect("regex group must always capture once")).unwrap(),
        ))
        //TODO @mark: unwrap is safe here right?
    }
}

#[derive(Debug)]
struct KeywordTokenizer(Regex);

impl KeywordTokenizer {
    fn new() -> Box<Self> {
        Box::new(KeywordTokenizer(
            Regex::new("^[ \t]*(local|mut)[ \t]+").unwrap(),
        ))
    }
}

impl Tokenizer for KeywordTokenizer {
    fn regex(&self) -> &Regex {
        &self.0
    }

    fn token_for(&self, name: Option<&str>) -> Option<Token> {
        Some(Token::Keyword(match name.expect("regex group must always capture once") {
            "local" => Keyword::Local,
            "mut" => Keyword::Mut,
            _ => return None,
        }))
    }
}

//TODO @mark: pity about dyn, see if it gets optimized
static TOKENIZERS: LazyLock<[Box<dyn Tokenizer>; 13]> = LazyLock::new(|| {
    debug!("start creating tokenizers (compiling regexes)");
    let tokenizers: [Box<dyn Tokenizer>; 13] = [
        CommentTokenizer::new(),
        FixedTokenTokenizer::new_parenthesis_open(),
        FixedTokenTokenizer::new_parenthesis_close(),
        FixedTokenTokenizer::new_newline(),
        FixedTokenTokenizer::new_semicolon(),
        FixedTokenTokenizer::new_colon(),
        OpSymbolTokenizer::new(),
        AssignmentTokenizer::new(),
        NumberTokenizer::new(),
        KeywordTokenizer::new(),
        TextTokenizer::new(),
        IdentifierTokenizer::new(),
        FixedTokenTokenizer::new_leftover_whitespace(),
    ];
    debug!(
        "finished creating {} tokenizers (compiling regexes)",
        tokenizers.len()
    );
    tokenizers
});

pub fn tokenize(src_pth: PathBuf, full_code: &str) -> Result<Vec<Token>, SteelErr> {
    let mut tokens = Vec::new();
    let mut ix = 0;
    'outer: while ix < full_code.len() {
        //TODO @mark: drop '...\n' continuations
        let code = &full_code[ix..];
        //TODO @mark: unroll:
        for tokenizer in &*TOKENIZERS {
            let Some(caps) = tokenizer.regex().captures_iter(code).next() else {
                continue;
            };
            let mtch = caps
                .get(0)
                .expect("regex group 0 should always match")
                .as_str();
            let grp = caps.get(1).map(|g| g.as_str());
            if let Some(token) = tokenizer.token_for(grp) {
                trace!(
                    "match {token:?} in '{mtch}' from {ix} to {}",
                    ix + mtch.len()
                );
                tokens.push(token);
            } else {
                trace!(
                    "matched tokenizer {tokenizer:?} from {ix} to {} but it produced no token",
                    ix + mtch.len()
                );
            }
            ix += mtch.len();
            debug_assert!(!mtch.is_empty());
            continue 'outer;
        }
        unreachable!(
            "unexpected end of input at #{ix} ('{}') after {} tokenizers",
            code.chars().next().unwrap(),
            TOKENIZERS.len()
        )
    }
    Ok(tokens)
}

#[cfg(test)]
mod tokens {
    use super::*;

    #[test]
    fn allow_whitespace_after_open_parenthesis() {
        let tokens = tokenize(PathBuf::from("test"), "(\n)");
        assert_eq!(
            tokens,
            Ok(vec![Token::ParenthesisOpen, Token::ParenthesisClose])
        );
    }

    #[test]
    fn handle_non_ascii_strings_and_comments() {
        let tokens = tokenize(PathBuf::from("test"), "\"你好\"# 你好");
        assert_eq!(tokens, Ok(vec![Token::Text("你好".to_owned())]));
    }

    #[test]
    fn simple_arithmetic() {
        let tokens = tokenize(PathBuf::from("test"), "(3) + (4 / 2)");
        assert_eq!(
            tokens,
            Ok(vec![
                Token::ParenthesisOpen,
                Token::Number(3.),
                Token::ParenthesisClose,
                Token::OpSymbol(OpCode::Add),
                Token::ParenthesisOpen,
                Token::Number(4.),
                Token::OpSymbol(OpCode::Div),
                Token::Number(2.),
                Token::ParenthesisClose
            ])
        );
    }

    #[test]
    fn skip_leftover_whitespace_at_end() {
        let tokens = tokenize(PathBuf::from("test"), "0\n\n");
        assert_eq!(tokens, Ok(vec![Token::Number(0.)]));
    }
}
