
//TODO @mark: delete?

pub mod builtins {
    pub const NEG: &'static str = "Negate.neg";
    pub const MINUS: &'static str = "Minus.minus";
}


macro_rules! make_builtin_constants {
    ($iden: ident, $ex: expr) => {
        pub const $iden: &'static str = $ex;
    };
    (($iden: ident, $ex: expr), $(($idens: ident, $exs: expr)),+) => {
        make_builtin_constants!(($iden, $ex));
    };
}

macro_rules! make_builtins {
    ($(($idens: ident, $exs: expr)),*) => {

    };
}

make_builtins!(
    (NEG, "Negate.neg"),
    (MINUS, "Minus.minus")
);
