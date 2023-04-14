use ::lalrpop_util::lalrpop_mod;

use crate::ast::Expr;
use crate::SteelErr;

lalrpop_mod!(gen_parser, "/grammar/struct_decl.rs");

pub fn parse_str(code: &str) -> Result<Expr, SteelErr> {
    let parser = gen_parser::ProgParser::new();
    let res = parser.parse(code);
    dbg!(&res);
    Ok(res.unwrap())
    //TODO @mark: no unwrap
}
