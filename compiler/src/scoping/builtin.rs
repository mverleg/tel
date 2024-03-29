use tel_api::VariableData;
//TODO @mark: delete?
use crate::scoping::api::Variable;
use crate::scoping::scope::Scope;

pub const NEG: &'static str = "Negate.neg";
pub const MINUS: &'static str = "Minus.minus";
pub const TEST: &'static str = "Test.test";

pub fn builtin_var_data(ix: usize) -> VariableData {
    debug_assert!(ix < 0, "built-ins should be indexed from -1 downwards");
    VariableData {
        ix,
        iden: (),
        type_annotation: None,
        mutable: false,
    }
    
}

fn make_builtin_scope() -> Scope {
    Scope {
        parent: None,
        items: (Variable { ix: 0, }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn implement_test() {
        unimplemented!();  //TODO @mark
    }
}
