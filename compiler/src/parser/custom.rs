
//TODO @mark: delete either this if lalrpop turns out better, or delete lalrpop if going this way

use ::std::path::PathBuf;

use crate::ast::{AST, OpCode};
use crate::SteelErr;

pub fn parse_str(src_pth: PathBuf, code: &str) -> Result<AST, SteelErr> {
    let tokens = tokenize(src_pth)?;
    unimplemented!()  //TODO @mark:
}

#[derive(Debug)]
pub enum Token {
    ParenthesisOpen,
    ParenthesisClose,
    Number(f64),
    OpSymbol(OpCode),
}

pub fn tokenize(src_pth: PathBuf, code: &str) -> Result<Vec<Token>, SteelErr> {
    unimplemented!()
}
