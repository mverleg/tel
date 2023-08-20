#![allow(unused)] //TODO @mark: TEMPORARY! REMOVE THIS!

use ::std::path::PathBuf;
use std::path::Path;

use ::lalrpop_util::lalrpop_mod;

use ::steel_api::log::debug;

use crate::ast::Ast;
use crate::parser::errors::build_error;
use crate::SteelErr;

mod errors;

lalrpop_mod!(#[allow(clippy::all)] gen_parser, "/grammar.rs");
include!(concat!(env!("OUT_DIR"), "/parse_tests.rs"));

pub fn parse_str(src_pth: PathBuf, code: &str) -> Result<Ast, SteelErr> {
    fail_if_no_newline_at_end(&src_pth, code)?;
    let parser = gen_parser::ProgParser::new();
    let res = parser.parse(code);
    match res {
        Ok(ast) => {
            debug!("ast: {:?}", &ast);
            Ok(ast)
        }
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

fn fail_if_no_newline_at_end(src_pth: &Path, code: &str) -> Result<(), SteelErr> {
    if count_empty_lines_at_end(code) == 0 {
        return Err(SteelErr::ParseErr {
            file: src_pth.to_owned(),
            line: code.lines().count(),
            //TODO @mark:  test ^
            msg: "Source files must have an empty line at the end".to_string(),
        })
    }
    Ok(())
}

fn count_empty_lines_at_end(text: &str) -> usize {
    let text = text.as_bytes();
    let mut lines = 0;
    let mut i = text.len() - 1;
    while i > 0 {
        while i > 0 && [b' ', b'\t'].contains(&text[i]) {
            i -= 1;
        }
        if text[i] == b'\n' {
            lines += 1;
        } else if text[i] == b'\r' {
            lines += 1;
            if i > 0 && text[i - 1] == b'\n' {
                i -= 1;
            }
        } else {
            break
        }
        i -= 1;
    }
    lines
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

    #[test]
    fn semicolon_at_end_of_file() {
        parse_str(PathBuf::new(), "a=1;").unwrap();
    }

    #[test]
    fn semicolon_between_statements_no_newline() {
        parse_str(PathBuf::new(), "a=1;b").unwrap();
    }

    #[test]
    fn short_closure_no_newline_at_eof() {
        parse_str(PathBuf::new(), "x\\0").unwrap();
    }

    #[test]
    fn short_closure_assign() {
        parse_str(PathBuf::new(), "a=\\2*it;b=x\\7\nc=y\\-it").unwrap();
    }
}
