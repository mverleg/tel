use ::lalrpop_util::lalrpop_mod;

use crate::SteelErr;

lalrpop_mod!(gen_parser, "/grammar/struct_decl.rs");

//TODO @mark:
pub fn parse_str(code: &str) -> Result<(), SteelErr> {
    let gp = gen_parser::ProgParser::new();
    let res = gp.parse(code);
    dbg!(res);
    // let calc = StructParser::parse(Rule::calculation, code).unwrap();
    // dbg!(&calc);  //TODO @mark: TEMPORARY! REMOVE THIS!
    Ok(())
}
