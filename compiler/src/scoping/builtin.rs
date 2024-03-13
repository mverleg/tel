
//TODO @mark: delete?
use crate::scoping::api::Variable;
use crate::scoping::scope::Scope;

// pub mod builtins {
//     pub const NEG: &'static str = "Negate.neg";
//     pub const MINUS: &'static str = "Minus.minus";
// }

macro_rules! make_builtin_constants {
    (($iden: ident, $ex: expr)) => {
        pub const $iden: &'static str = $ex;
    };
    (($iden: ident, $ex: expr), $(($idens: ident, $exs: expr)),+) => {
        make_builtin_constants!(($iden, $ex));
        make_builtin_constants!($(($idens, $exs)),+);
    };
}

// macro_rules! make_var_ref {
    // (($iden: ident, $ex: expr)) => {
    //     Variable { ix: 0 }
    // };
    // (($iden: ident, $ex: expr), $(($idens: ident, $exs: expr)),+) => {
    //     make_var_ref!(($iden, $ex));
    //     make_var_ref!($(($idens, $exs)),+);
    // };
// }

macro_rules! make_var {
    ($($iden: ident),*) => {
        Variable { ix: 0 }
    }
}

macro_rules! make_builtins {
    ($(($idens: ident, $exs: expr)),*) => {
        make_builtin_constants!($(($idens, $exs)),+);
        fn make_builtin_scope() -> Scope {
            Scope {
                parent: None,
                items: vec![
                    $(make_var!($idens),)+
                    //make_var_ref!($(($idens, $exs)),+);
                ],
            }
        }
    };
}

make_builtins!(
    (NEG, "Negate.neg"),
    (MINUS, "Minus.minus"),
    (TEST, "Test.test")
);
