
use ::tel_api as api;
use ::tel_api::TelFile;
use ::tel_api::Variables;

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
            Block::Assigns(assign) => { assignments_to_api(assign, &mut variables, &mut global_scope)?; }
            Block::Expression(expression) => { expression_to_api(&expression, &mut variables, &mut global_scope)?; }
            Block::Return(_expression) => todo!("Return"),
            //TODO @mark: return ^
            Block::Struct(_struct) => todo!("Struct"),
            Block::Enum(_enum) => todo!("Enum"),
        }
    }
    Ok(TelFile {})
}

fn expression_to_api(
    expr: &ast::Expr,
    variables: &mut Variables,
    scope: &mut Scope,
) -> Result<api::Expr, TelErr> {
    //TODO @mark: to owned expression?
    Ok(match expr {
        ast::Expr::Num(num) => api::Expr::Num(*num),
        ast::Expr::Text(_text) => todo!("Text"),
        ast::Expr::BinOp(_bin_op, _, _) => todo!("BinOp"),
        ast::Expr::UnaryOp(_unary_op, _) => todo!("UnaryOp"),
        ast::Expr::Invoke(invoke) => invoke_to_api(invoke, variables, scope),
        ast::Expr::Dot(_dot, _) => todo!("Dot"),
        ast::Expr::Closure(_closure) => todo!("Closure"),
        ast::Expr::If(_if, _) => todo!("If"),
        ast::Expr::While(_while, _) => todo!("While"),
        ast::Expr::ForEach(_for_each, _, _) => todo!("ForEach"),
    })
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
            (AssignmentKw::None, None) => (true, false),
            (AssignmentKw::None, Some(_)) => (false, false),
            (AssignmentKw::Outer, _) => (true, false),
            (AssignmentKw::Local, _) => (false, false),
            (AssignmentKw::Mut, _) => (false, true),
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
        value = api::Expr::Read(binding);
    }
    Ok(api_assignments)
}

fn invoke_to_api(
    invoke: &ast::Invoke,
    variables: &mut Variables,
    scope: &mut Scope,
) -> Result<api::Expr, TelErr> {
    //TODO @mark: remove borrow ^
    let ast::Invoke { iden: ast_iden, args } = invoke;
    let Some(api_iden) = scope.lookup(variables, ast_iden) else {
        return Err(TelErr::UnknownIdentifier(ast_iden.clone()))
    };
    Ok(api::Expr::Invoke { iden: api_iden, args: vec![] })
}

#[cfg(test)]
mod tests {
    use ::tel_api::Identifier;

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
        let api::Assignment { var: var1, value: value1 } = &res[0];
        assert_eq!(var1.iden(&variables).to_string(), "b");
        assert!(matches!(value1, api::Expr::Num(1.0)));
        let api::Assignment { var: var2, value: value2 } = &res[1];
        assert_eq!(var2.iden(&variables).to_string(), "a");
        let api::Expr::Read(read_var) = value2 else { panic!() };
        assert_eq!(read_var, var1);
        //res[0].value
    }
}
