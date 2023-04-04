use crate::SteelErr;

#[derive(Parser)]
#[grammar = "grammar/struct.pest"]
pub struct StructParser;

//TODO @mark:
pub fn parse_str(code: &str) -> Result<(), SteelErr> {
    let calc = StructParser::parse(Rule::calculation, code).unwrap();
    dbg!(&calc);  //TODO @mark: TEMPORARY! REMOVE THIS!
    unimplemented!() //TODO @mark:
}
