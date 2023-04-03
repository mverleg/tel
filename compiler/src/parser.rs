
use ::pest_derive::Parser;

#[derive(Parser)]
#[grammar = "grammar/struct.pest"]
pub struct Struct;
