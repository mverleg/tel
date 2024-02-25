use ::tel_api::TelFile;

use crate::ast::Ast;
use crate::ast::Assignments;
use crate::ast::Block;
use crate::scoping::util::LinearScope;
use crate::TelErr;

pub type Scope = LinearScope;

pub mod util;

pub fn ast_to_api(ast: &Ast) -> Result<TelFile, TelErr> {
    let Ast { blocks } = ast;
    for block in blocks.into_iter() {
        match block {
            Block::Assigns(assign) => assignments_to_api(assign),
            Block::Expression(_expression) => todo!(),
            Block::Struct(_struct) => todo!(),
            Block::Enum(_enum) => todo!(),
        }
    }
    Ok(TelFile {})
}

fn assignments_to_api(assign: &Assignments) {
    todo!()
}
