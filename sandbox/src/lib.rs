pub mod execute;
pub mod io;
pub mod parse;
pub mod resolve;
pub mod types;

use std::fmt;

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Parse(types::ParseError),
    Resolve(types::ResolveError),
    Execute(types::ExecuteError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "IO error: {}", e),
            Error::Parse(e) => write!(f, "Parse error: {}", e),
            Error::Resolve(e) => write!(f, "Resolve error: {}", e),
            Error::Execute(e) => write!(f, "Execute error: {}", e),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<types::ParseError> for Error {
    fn from(e: types::ParseError) -> Self {
        Error::Parse(e)
    }
}

impl From<types::ResolveError> for Error {
    fn from(e: types::ResolveError) -> Self {
        Error::Resolve(e)
    }
}

impl From<types::ExecuteError> for Error {
    fn from(e: types::ExecuteError) -> Self {
        Error::Execute(e)
    }
}

pub fn run_file(path: &str) -> Result<(), Error> {
    let source = io::load_file(path)?;
    let pre_ast = parse::parse(&source)?;
    let (ast, symbols) = resolve::resolve(pre_ast, path)?;
    execute::execute(ast, &symbols)?;
    Ok(())
}

pub fn run_source(source: &str) -> Result<(), Error> {
    let pre_ast = parse::parse(source)?;
    let (ast, symbols) = resolve::resolve(pre_ast, ".")?;
    execute::execute(ast, &symbols)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_number() {
        let result = run_source("42");
        assert!(result.is_ok());
    }

    #[test]
    fn test_arithmetic() {
        let result = run_source("(+ 10 20)");
        assert!(result.is_ok());
    }

    #[test]
    fn test_let_and_print() {
        let result = run_source("(let x 5) (print x)");
        assert!(result.is_ok());
    }

    #[test]
    fn test_if_true_branch() {
        let result = run_source("(let x 10) (let y 20) (if (> y x) (print 1) (print 0))");
        assert!(result.is_ok());
    }

    #[test]
    fn test_if_false_branch() {
        let result = run_source("(let x 10) (let y 5) (if (> y x) (print 1) (print 0))");
        assert!(result.is_ok());
    }

    #[test]
    fn test_undefined_variable() {
        let result = run_source("(print x)");
        assert!(result.is_err());
        match result {
            Err(Error::Resolve(_)) => {}
            _ => panic!("Expected resolve error"),
        }
    }

    #[test]
    fn test_scope_isolation() {
        let result = run_source("(let x 5) (if (> x 0) (let y 10) (let z 20)) (print x)");
        assert!(result.is_ok());
    }

    #[test]
    fn test_comparison_operators() {
        let result = run_source("(print (< 5 10))");
        assert!(result.is_ok());

        let result = run_source("(print (== 5 5))");
        assert!(result.is_ok());
    }

    #[test]
    fn test_logical_operators() {
        let result = run_source("(print (&& 1 1))");
        assert!(result.is_ok());

        let result = run_source("(print (|| 0 1))");
        assert!(result.is_ok());
    }

    #[test]
    fn test_division_by_zero() {
        let result = run_source("(/ 10 0)");
        assert!(result.is_err());
        match result {
            Err(Error::Execute(_)) => {}
            _ => panic!("Expected execute error"),
        }
    }

    #[test]
    fn test_complex_expression() {
        let result = run_source(
            "(let x 5) (let y 10) (if (> y x) (print (+ x y)) (print (- x y)))",
        );
        assert!(result.is_ok());
    }
}
