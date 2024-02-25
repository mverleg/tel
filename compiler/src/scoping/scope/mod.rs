use ::tel_api::VarRead;
use ::tel_api::Identifier;
use ::tel_api::Type;

pub use self::linear::LinearScope;

mod linear;
mod hash;

pub trait Scope {
    fn new() -> LinearScope {
        //TODO @mark: compare impl performances
        return LinearScope::new_root()
    }

    fn get_or_insert(
        &mut self,
        iden: &Identifier,
        type_annotation: Option<&Type>,
        is_mutable: bool,
    ) -> VarRead;
}
