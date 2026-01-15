use tel_ast::TelFile;
use tel_ast::Variables;
use tel_ast as ast;
use tel_common::TelErr;
pub use self::scope::Scope;

mod scope;

pub fn ast_to_api(ast: ast::Ast) -> Result<TelFile, TelErr> {
    let ast::Ast { blocks } = ast;
    let mut variables = Variables::new();
    let mut global_scope = Scope::new_root(&mut variables);
    let blocks = blocks.into_vec();  //TODO @mark: TEMPORARY! REMOVE THIS!
    for block in blocks.into_iter() {
        // let block: Block = block;  // enforce that `block` is not borrowed
        //TODO @mark: ^ enable this and remove clones
        match block {
            ast::Block::Assigns(assign) => { assignments_to_api(assign, &mut variables, &mut global_scope)?; }
            ast::Block::Expression(expression) => { expression_to_api(&expression, &mut variables, &mut global_scope)?; }
            ast::Block::Return(_expression) => todo!("Return"),
            //TODO @mark: return ^
            ast::Block::Struct(_struct) => todo!("Struct"),
            ast::Block::Enum(_enum) => todo!("Enum"),
        }
    }
    Ok(TelFile {})
}

fn expression_to_api(
    expr: &ast::Expr,
    variables: &mut Variables,
    scope: &mut Scope,
) -> Result<ast::Expr, TelErr> {
    //TODO @mark: to owned expression?
    Ok(match expr {
        ast::Expr::Num(num) => ast::Expr::Num(*num),
        ast::Expr::Text(_text) => todo!("Text"),
        //ast::Expr::BinOp(op, left, right) => invoke_binary_to_api(*op, left, right, variables, scope)?,
        //ast::Expr::UnaryOp(op, expr) => invoke_unary_to_api(*op, expr, variables, scope)?,
        //ast::Expr::Invoke(invoke) => invoke_to_api(invoke, variables, scope)?,
        ast::Expr::BinOp(op, left, right) => unimplemented!(),
        ast::Expr::UnaryOp(op, expr) => unimplemented!(),
        ast::Expr::Invoke(invoke) => unimplemented!(),
        ast::Expr::Dot(_dot, _) => todo!("Dot"),
        ast::Expr::Closure(_closure) => todo!("Closure"),
        ast::Expr::If(_if, _) => todo!("If"),
        ast::Expr::While(_while, _) => todo!("While"),
        ast::Expr::ForEach(_for_each, _, _) => todo!("ForEach"),
    })
}

fn assignments_to_api(
    assign: ast::Assignments,
    variables: &mut Variables,
    scope: &mut Scope,
) -> Result<Vec<ast::Assignment>, TelErr> {
    //TODO @mark: use more efficient vec
    let ast::Assignments { dest: dests, op, value: ast_value } = assign;
    debug_assert!(dests.len() >= 1);
    if let Some(_op) = op {
        todo!()
    }
    let mut api_assignments = Vec::with_capacity(dests.len());
    let mut value = expression_to_api(&ast_value, variables, scope)?;
    for dest in dests.iter().rev() {
        // let dest: AssignmentDest = dest;  // enforce that `dest` is not borrowed
        //TODO @mark: ^ enable this and pass owned values to scope
        let ast::AssignmentDest { kw, target, typ } = dest;
        let (allow_outer, is_mutable) = match (kw, typ) {
            (ast::AssignmentKw::None, None) => (true, false),
            (ast::AssignmentKw::None, Some(_)) => (false, false),
            (ast::AssignmentKw::Outer, _) => (true, false),
            (ast::AssignmentKw::Local, _) => (false, false),
            (ast::AssignmentKw::Mut, _) => (false, true),
        };
        let binding = if allow_outer {
            scope.declare_in_scope(
                variables,
                &target,
                typ.as_ref(),
                is_mutable,
            )?
        } else {
            scope.assign_or_declare(
                variables,
                target,
                typ.as_ref(),
                is_mutable,
            )
        };
        api_assignments.push(ast::Assignment {
            var: binding,
            value,
        });
        value = ast::Expr::Invoke(ast::Invoke {
            iden: binding.iden(&variables).clone(),
            args: Box::new([]),
        });
    }
    Ok(api_assignments)
}

fn invoke_to_api(
    invoke: &ast::Invoke,
    variables: &mut Variables,
    scope: &mut Scope,
) -> Result<ast::Expr, TelErr> {
    //TODO @mark: remove borrow ^
    let ast::Invoke { iden: ast_iden, args: ast_args } = invoke;
    let Some(api_iden) = scope.lookup(variables, ast_iden) else {
        return Err(TelErr::UnknownIdentifier(ast_iden.clone()))
    };
    let api_args: Box<[ast::Expr]> = ast_args.into_iter()
        .map(|e| expression_to_api(e, variables, scope))
        .collect::<Result<Vec<_>, _>>()?
        .into_boxed_slice();
    Ok(ast::Expr::Invoke(ast::Invoke { iden: api_iden.iden(&variables).clone(), args: api_args }))
}

fn invoke_unary_to_api(
    op: ast::UnaryOpCode,
    ast_expr: &Box<ast::Expr>,
    variables: &mut Variables,
    scope: &mut Scope,
) -> Result<ast::Expr, TelErr> {
    let _builtin_iden = match op {
        //UnaryOpCode::Not => Identifier::new(builtins.NEG).expect("built-in must be valid"),
        //TODO @mark:
        ast::UnaryOpCode::Not => {},
        ast::UnaryOpCode::Min => {},
        //TODO @mark: how to impl preamble? always add to root scope? should have some constants, not lookup each time
    };
    let _api_expr = expression_to_api(ast_expr, variables, scope)?;
    todo!("invoke_unary_to_api not yet implemented")
}

fn invoke_binary_to_api(
    op: ast::BinOpCode,
    ast_left: &Box<ast::Expr>,
    ast_right: &Box<ast::Expr>,
    variables: &mut Variables,
    scope: &mut Scope,
) -> Result<ast::Expr, TelErr> {
    let _builtin_iden = match op {
        ast::BinOpCode::Add => {}
        ast::BinOpCode::Sub => {}
        ast::BinOpCode::Mul => {}
        ast::BinOpCode::Div => {}
        ast::BinOpCode::Modulo => {}
        ast::BinOpCode::Eq => {}
        ast::BinOpCode::Neq => {}
        ast::BinOpCode::Lt => {}
        ast::BinOpCode::Gt => {}
        ast::BinOpCode::Le => {}
        ast::BinOpCode::Ge => {}
        ast::BinOpCode::And => {}
        ast::BinOpCode::Or => {}
        ast::BinOpCode::Xor => {}
    };
    let _api_left = expression_to_api(ast_left, variables, scope)?;
    let _api_right = expression_to_api(ast_right, variables, scope)?;
    todo!("invoke_binary_to_api not yet implemented")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tel_common::Identifier;

    #[test]
    fn repeated_assign() {
        let mut variables = Variables::new();
        let mut global_scope = Scope::new_root(&mut variables);
        let assign = ast::Assignments {
            dest: Box::new([ast::AssignmentDest {
                kw: ast::AssignmentKw::None,
                target: Identifier::new("a").unwrap(),
                typ: None,
            }, ast::AssignmentDest {
                kw: ast::AssignmentKw::None,
                target: Identifier::new("b").unwrap(),
                typ: None,
            }]),
            op: None,
            value: Box::new(ast::Expr::Num(1.0)),
        };
        let res = assignments_to_api(assign, &mut variables, &mut global_scope).unwrap();
        assert_eq!(res.len(), 2);
        let ast::Assignment { var: var1, value: value1 } = &res[0];
        assert_eq!(var1.iden(&variables).to_string(), "b");
        assert!(matches!(value1, ast::Expr::Num(1.0)));
        let ast::Assignment { var: var2, value: value2 } = &res[1];
        assert_eq!(var2.iden(&variables).to_string(), "a");
        let ast::Expr::Invoke(ast::Invoke { iden: read_iden, args: read_args }) = value2 else { panic!() };
        assert_eq!(read_iden, var1.iden(&variables));
        assert_eq!(read_args.len(), 0);
        //res[0].value
    }
}
