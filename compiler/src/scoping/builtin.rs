
//TODO @mark: delete?
use crate::scoping::api::Variable;
use crate::scoping::scope::Scope;

// pub mod builtins {
//     pub const NEG: &'static str = "Negate.neg";
//     pub const MINUS: &'static str = "Minus.minus";
// }

macro_rules! make_builtin_constants {
    (($iden: ident, $text: expr)) => {
        pub const $iden: &'static str = $text;
    };
    (($iden: ident, $text: expr), $(($idens: ident, $texts: expr)),+) => {
        make_builtin_constants!(($iden, $text));
        make_builtin_constants!($(($idens, $texts)),+);
    };
}

macro_rules! make_var {
    (($nr: expr, $iden: ident)) => {
        Variable { ix: $nr + 0, name: stringify!($iden) }
    };
    (($nr: expr, $iden: ident), $(($nrs: expr, $idens: ident)),+) => {
        make_var!(($nr + 1, $iden)),
        make_var!($(($nrs, $idens)),+);
    };
}

macro_rules! make_builtins {
    ($(($idens: ident, $texts: expr)),*) => {
        make_builtin_constants!($(($idens, $texts)),+);
        fn make_builtin_scope() -> Scope {
            Scope {
                parent: None,
                items: (
                    make_var!($((0, $idens)),+)
                    //make_var_ref!($(($idens, $exs)),+);
                ),
            }
        }
    };
}

make_builtins!(
    (NEG, "Negate.neg"),
    (MINUS, "Minus.minus"),
    (TEST, "Test.test")
);
