#![allow(unused)] //TODO @mark: TEMPORARY! REMOVE THIS!

use ::std::path::Path;
use ::std::path::PathBuf;

use ::lalrpop_util::lalrpop_mod;

use ::tel_api::log::debug;
use ::tel_api::TelFile;

use crate::ast::Ast;
use crate::parser::errors::build_error;
use crate::scoping::ast_to_api;
use crate::TelErr;

mod errors;

lalrpop_mod!(#[allow(clippy::all)] gen_parser, "/grammar.rs");
include!(concat!(env!("OUT_DIR"), "/parse_tests.rs"));

pub fn parse_str(src_pth: PathBuf, mut code: String) -> Result<TelFile, TelErr> {
    let ast = str_to_ast(src_pth, code)?;
    ast_to_api(ast)
}

pub fn str_to_ast(src_pth: PathBuf, mut code: String) -> Result<Ast, TelErr> {
    if count_empty_lines_at_end(&code) == 0 {
        code.push('\n')
    }
    fail_if_no_newline_at_end(&src_pth, &code)?;
    let parser = gen_parser::ProgParser::new();
    let res = parser.parse(&code);
    match res {
        Ok(ast) => {
            debug!("ast: {:?}", &ast);
            Ok(ast)
        }
        Err(err) => {
            let (msg, line) = build_error(err, src_pth.to_str().unwrap(), &code);
            Err(TelErr::ParseErr {
                file: src_pth,
                line,
                msg,
            })
        }
    }
    //TODO @mark: no unwrap
}

fn fail_if_no_newline_at_end(src_pth: &Path, code: &str) -> Result<(), TelErr> {
    if count_empty_lines_at_end(code) == 0 {
        return Err(TelErr::ParseErr {
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
    let mut i = text.len().saturating_sub(1);
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
mod internal {
    use super::*;

    #[test]
    fn test_count_empty_lines_at_end() {
        assert_eq!(0, count_empty_lines_at_end(""));
        assert_eq!(0, count_empty_lines_at_end("\n"));
        assert_eq!(1, count_empty_lines_at_end("\n\n"));
    }
}

#[cfg(test)]
mod bugs {
    use super::*;

    fn parse(code: &str) -> TelFile {
        match parse_str(PathBuf::new(), code.to_owned()) {
            Ok(ast) => ast,
            Err(TelErr::ParseErr { msg, .. }) => {
                println!("{}", msg);
                panic!()
            }
            Err(_) => panic!(),
        }
    }

    #[test]
    fn test_newline_with_indent() {
        parse("(1)+\n 2");
    }

    #[test]
    fn test_double_close_parentheses() {
        let code = "(1)+\n 2)";
        assert!(parse_str(PathBuf::new(), code.to_owned()).is_err());
    }

    #[test]
    fn empty_struct() {
        parse("struct D {\n}");
    }

    #[test]
    fn nullary_function_with_parentheses() {
        parse("f()");
    }

    #[test]
    fn semicolon_at_end_of_file() {
        parse("a=1;");
    }

    #[test]
    fn semicolon_between_statements_no_newline() {
        parse("a=1;b");
    }

    // Disabled until custom lexer or similar solution, see https://github.com/mverleg/lalrpop_close_block_and_statement
    // #[test]
    // fn short_closure_no_newline_at_eof() {
    //     parse("x\\0").unwrap();
    // }
    //
    // #[test]
    // fn short_closure_assign() {
    //     parse("a=\\2*it;b=x\\7\nc=y\\-it").unwrap();
    // }
    //
    // #[test]
    // fn short_closure_end_statement() {
    //     let ast = parse("a=f()\\1\nb=1").unwrap();
    //     assert!(ast.blocks.len() == 2);
    // }
    //
    // #[test]
    // fn short_closure_period_newline() {
    //     parse("x.a\\1\n.b;f").unwrap();
    // }

    #[test]
    fn works_without_trailing_newline() {
        parse("5 +\n5");
    }

    #[test]
    fn reject_arithmetic_without_spacing() {
        assert!(parse_str(PathBuf::new(), "1+\n1".to_owned()).is_err());
        assert!(parse_str(PathBuf::new(), "1 /1".to_owned()).is_err());
    }

    #[test]
    fn line_continuation() {
        let code = "x = 1 ...\n\n+ 2";
        parse(code);
    }
}
