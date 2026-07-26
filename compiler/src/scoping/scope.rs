//! Scope contains the variables in a block, like a file, function or loop.
//! It is a tree structure, where each scope refers to the parent.
//!
//! This implementation stores the values linearly. There are alternative ways,
//! most obviously a hashmap for fast lookup, but also a mix of the two.

use tel_ast::Type;
use tel_ast::Variable;
use tel_ast::Variables;
use tel_common::spec;
use tel_common::{Identifier, TelErr};

#[derive(Debug)]
pub struct Scope {
    //TODO @mark: scopes have their own tree structure, even though it matches the AST
    //TODO @mark: for now it seems ot make the code easier (and possibly faster), but might reconsider
    parent: Option<Box<Scope>>,
    items: Vec<Variable>,
    //TODO @mark: smallvec
}

impl Scope {
    //TODO @mark: reconsider Rc here (won't be able to add variables if it's Rc anyway)
    pub fn new_root(variables: &mut Variables) -> Self {
        Scope {
            parent: Some(Box::new(Self::new_builtin(variables))),
            items: vec![]
        }
    }

    fn new_builtin(_variables: &mut Variables) -> Self {
        //TODO @mark: perhaps not the best representation, could be enum or map
        
        // scope.declare_in_scope(variables, &Identifier::new("Negate.neg").unwrap(), Some(&Type { iden: Identifier::new("bool -> bool").unwrap(), generics: Box::new([]) }), false);
        // for builtin_iden in BUILTINS {
        //     scope.declare_in_scope(variables, &Identifier::new(builtin_iden).unwrap(), None, false);
        // }
        Scope {
            parent: None,
            items: vec![]
        }
    }

    pub fn declare_in_scope(
        &mut self,
        variables: &mut Variables,
        iden: &Identifier,
        type_annotation: Option<&Type>,
        mutable: bool
    ) -> Result<Variable, TelErr> {
        spec!(SAME_SCOPE_REDECLARATION, "declaration sites only; `let` across a scope boundary is not implemented yet");
        for &known in &self.items {
            if known.iden(variables) == iden {
                // the docs settle this: shadowing needs a scope boundary, a repeat in one scope is an error
                return Err(TelErr::ScopeErr {
                    msg: format!("variable '{iden}' declared twice in this scope")
                })
            }
        }
        let new_var = variables.add(
            iden.clone(),
            type_annotation.cloned(),
            mutable,
        );
        self.items.push(new_var);
        Ok(*self.items.last().expect("just added, cannot fail"))
    }

    pub fn assign_or_declare(
        &mut self,
        variables: &mut Variables,
        iden: &Identifier,
        type_annotation: Option<&Type>,
        mutable: bool
    ) -> Variable {
        if let Some(_parent) = &self.parent {
            todo!("nested scopes not yet implemented")
        }
        for &known in &self.items {
            if known.iden(variables) == iden {
                return known
            }
        }
        let new_var = variables.add(
            iden.clone(),
            type_annotation.cloned(),
            mutable,
        );
        self.items.push(new_var);
        *self.items.last().expect("just added, cannot fail")
    }

    pub fn lookup(
        &mut self,
        variables: &mut Variables,
        iden: &Identifier,
    ) -> Option<Variable> {
        for &known in &self.items {
            if known.iden(variables) == iden {
                return Some(known)
            }
        }
        if let Some(_parent) = &self.parent {
            todo!("nested scopes not yet implemented")
        }
        None
    }
}
