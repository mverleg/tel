
#[derive(Debug)]
pub struct Engine {
}

impl Engine {
    /// Load the source as text
    fn source(&mut self, module: ()) {}

    /// Parse into syntax tree
    fn parse(&mut self, module: ()) {}

    /// Resolve all references in the function or datatype
    fn resolve(&mut self, code_unit: ()) {}

    /// Infer and check the types for a function or datatype
    fn generic_typ(&mut self, code_unit: ()) {}

    /// Based on a generic type, create a concrete impl based on given usage,
    /// where all types are concrete. E.g. add(T, T) with T=int gives add(int, int)
    //TODO @mark: canonical representation, e.g. unwrap wrappers, sort args canonically, ...
    //TODO @mark: how to find duplicate code? does that happen here or in optimization?
    fn monomorph(&mut self, code_unit: (), usage_types: ()) {}

    /// Generate (unoptimized) IR code for a function or datatype
    fn ir(&mut self, code_unit: ()) {}

    /// Generate optimized IR code for a function or datatype (as far as possible
    /// without knowing about usage).
    fn optimize(&mut self, code_unit: ()) {}

    /// Create full program IR.
    fn executable(&mut self, iden: (), is_opt: ()) {}

    //TODO @mark: also tests, documentation, etc.
}
