use ::std::path::PathBuf;

use ::lalrpop_util::lalrpop_mod;

use crate::ast::Expr;
use crate::parser::errors::build_error;
use crate::SteelErr;

mod errors;

lalrpop_mod!(gen_parser, "/grammar/struct_decl.rs");

pub fn parse_str(src_pth: PathBuf, code: &str) -> Result<Vec<Expr>, SteelErr> {
    let parser = gen_parser::ProgParser::new();
    let res = parser.parse(code);
    dbg!(&res);
    match res {
        Ok(ast) => Ok(ast),
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
