#![allow(unused)] //TODO @mark: TEMPORARY! REMOVE THIS!

use ::std::path::PathBuf;

use ::lalrpop_util::lalrpop_mod;

use ::steel_api::log::debug;

use crate::ast::Ast;
use crate::parser::errors::build_error;
use crate::SteelErr;

mod errors;

lalrpop_mod!(gen_parser, "/grammar.rs");
include!(concat!(env!("OUT_DIR"), "/parse_tests.rs"));

pub fn parse_str(src_pth: PathBuf, code: &str) -> Result<Ast, SteelErr> {
    let parser = gen_parser::ProgParser::new();
    let res = parser.parse(code);
    match res {
        Ok(ast) => {
            debug!("ast: {:?}", &ast);
            Ok(ast)
        },
        Err(err) => {
            let (msg, line) = build_error(err, src_pth.to_str().unwrap(), code);
            Err(SteelErr::ParseErr {
                file: src_pth,
                line,
                msg,
            })
        }
    }
    //TODO @mark: no unwrap
}

#[cfg(test)]
mod bugs {
    use super::*;

    #[test]
    fn test_newline_with_indent() {
        parse_str(PathBuf::new(), "(1)+\n 2").unwrap();
    }

    #[test]
    fn test_double_close_parentheses() {
        assert!(parse_str(PathBuf::new(), "(1)+\n 2)").is_err());
    }

    #[test]
    fn empty_struct() {
        parse_str(PathBuf::new(), "struct D {\n}").unwrap();
    }

    #[test]
    fn nullary_function_with_parentheses() {
        parse_str(PathBuf::new(), "f()").unwrap();
    }
}
