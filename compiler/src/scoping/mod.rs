use ::tel_api::TelFile;

use crate::ast::AssignmentKw;
use crate::ast::AssignmentDest;
use crate::ast::Expr;
use crate::ast::Ast;
use crate::ast::Assignments;
use crate::ast::Block;
use crate::scoping::util::LinearScope;
use crate::TelErr;

pub type Scope = LinearScope;

pub mod util;

pub fn ast_to_api(ast: &Ast) -> Result<TelFile, TelErr> {
    let Ast { blocks } = ast;
    let global_scope = Scope::new();
    for block in blocks.into_iter() {
        match block {
            Block::Assigns(assign) => assignments_to_api(assign)?,
            Block::Expression(_expression) => todo!(),
            Block::Struct(_struct) => todo!(),
            Block::Enum(_enum) => todo!(),
        }
    }
    Ok(TelFile {})
}

fn assignments_to_api(
    assign: &Assignments,
    scopes: &mut Vec<&mut Scope>,
) -> Result<(), TelErr> {
    let Assignments { dest: dests, op, value } = assign;
    debug_assert!(dests.len() >= 1);
    for dest in dests.into_iter() {
        let AssignmentDest { kw, target, typ } = dest;
        let mutable = match kw {
            AssignmentKw::None => false,
            AssignmentKw::Outer => todo!(),
            AssignmentKw::Local => todo!(),
            AssignmentKw::Mut => true,
        };
        todo!();
        let expr = expression_to_api(value)?;
    }
    todo!()
}

fn expression_to_api(expr: &Expr) -> Result<(), TelErr> {
    todo!()

}
