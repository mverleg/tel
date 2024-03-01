use ::tel_api::TelFile;
use tel_api::Variables;

use crate::ast::AssignmentDest;
use crate::ast::AssignmentKw;
use crate::ast::Assignments;
use crate::ast::Ast;
use crate::ast::Block;
use crate::ast::Expr;
use crate::TelErr;

pub use self::scope::Scope;

mod scope;

pub fn ast_to_api(ast: Ast) -> Result<TelFile, TelErr> {
    let Ast { blocks } = ast;
    let mut variables = Variables::new();
    let mut global_scope = Scope::new_root();
    let blocks = blocks.into_vec();  //TODO @mark: TEMPORARY! REMOVE THIS!
    for block in blocks.into_iter() {
        // let block: Block = block;  // enforce that `block` is not borrowed
        //TODO @mark: ^ enable this and remove clones
        match block {
            Block::Assigns(assign) => assignments_to_api(assign, &mut variables, &mut global_scope)?,
            Block::Expression(_expression) => todo!(),
            Block::Struct(_struct) => todo!(),
            Block::Enum(_enum) => todo!(),
        }
    }
    Ok(TelFile {})
}

fn assignments_to_api(
    assign: Assignments,
    variables: &mut Variables,
    scopes: &mut Scope,
) -> Result<(), TelErr> {
    let Assignments { dest: dests, op, value } = assign;
    debug_assert!(dests.len() >= 1);
    if let Some(_op) = op {
        todo!()
    }
    for dest in dests.into_iter() {
        // let dest: AssignmentDest = dest;  // enforce that `dest` is not borrowed
        //TODO @mark: ^ enable this and pass owned values to get_or_insert
        let AssignmentDest { kw, target, typ } = dest;
        let (allow_outer, is_mutable) = match (kw, typ) {
            (AssignmentKw::None, None) =>    (true,  false),
            (AssignmentKw::None, Some(_)) => (false, false),
            (AssignmentKw::Outer, _) =>      (true,  false),
            (AssignmentKw::Local, _) =>      (false, false),
            (AssignmentKw::Mut, _) =>        (false, true),
        };
        let binding = if allow_outer {
            scopes.declare_in_scope(
                variables,
                target,
                typ.as_ref(),
                is_mutable,
            )?
        } else {
            scopes.assign_or_declare(
                variables,
                target,
                typ.as_ref(),
                is_mutable,
            )
        };
        todo!();
        let expr = expression_to_api(value)?;
    }
    todo!()
}

fn expression_to_api(expr: Box<Expr>) -> Result<(), TelErr> {
    todo!()

}
