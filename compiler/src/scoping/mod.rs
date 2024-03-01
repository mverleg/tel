use ::tel_api::TelFile;
use ::tel_api::Variables;
use ::tel_api as api;

use crate::ast;
use crate::ast::AssignmentDest;
use crate::ast::AssignmentKw;
use crate::ast::Assignments;
use crate::ast::Ast;
use crate::ast::Block;
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
            Block::Assigns(assign) => { assignments_to_api(assign, &mut variables, &mut global_scope)?; },
            Block::Expression(expression) => { expression_to_api(&expression)?; },
            //TODO @mark: return ^
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
) -> Result<Vec<api::Assignment>, TelErr> {
    //TODO @mark: use more efficient vec
    let Assignments { dest: dests, op, value: ast_value } = assign;
    debug_assert!(dests.len() >= 1);
    if let Some(_op) = op {
        todo!()
    }
    let mut api_assignments = Vec::with_capacity(dests.len());
    let mut value = expression_to_api(&ast_value)?;
    for dest in dests.into_iter().rev() {
        // let dest: AssignmentDest = dest;  // enforce that `dest` is not borrowed
        //TODO @mark: ^ enable this and pass owned values to scope
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
        api_assignments.push(api::Assignment {
            var: binding,
            value,
        });
        todo!("create assignment binding=value");
        value = api::Expr::Read(binding);
    }
    Ok(api_assignments)
}

fn expression_to_api(expr: &ast::Expr) -> Result<api::Expr, TelErr> {
    //TODO @mark: to owned expression?
    todo!()

}

#[cfg(test)]
mod tests {
    use ::tel_api::Identifier;

    use crate::ast;

    use super::*;

    #[test]
    fn repeated_assign() {
        let mut variables = Variables::new();
        let mut global_scope = Scope::new_root();
        let assign = Assignments {
            dest: Box::new([AssignmentDest {
                kw: AssignmentKw::None,
                target: Identifier::new("a").unwrap(),
                typ: None,
            }, AssignmentDest {
                kw: AssignmentKw::None,
                target: Identifier::new("b").unwrap(),
                typ: None,
            }]),
            op: None,
            value: Box::new(ast::Expr::Num(1.0)),
        };
        let res = assignments_to_api(assign, &mut variables, &mut global_scope).unwrap();
        assert_eq!(res.len(), 2);
        todo!("check that res is double assignment")
    }
}
