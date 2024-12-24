use std::{fmt, hash};

trait FileSystem {
    fn read(iden: FileIden) -> String;
}

#[derive(Debug, Clone, Copy)]
enum Error {
    SourceNotFound
}

#[derive(Debug)]
struct Stat<T> {
    value: Result<T, Error>,
    msgs: Vec<String>,
    //TODO @mark: tinyvec
}

#[derive(Debug)]
struct FileIden {
    iden: String,
    //TODO @mark: should be lightweight, maybe convert to nr?
}

#[derive(Debug)]
struct Import {
    file: FileIden,
    name: String,
}

#[derive(Debug)]
struct FileCode {
    text: String,
}

#[derive(Debug)]
struct AST {
    // ...
}

// #[derive(Debug)]
// enum Query<F: FileSystem> {
//     Source(FileIden),
//     Parsed(FileIden),
//     Import(Import),
//     IR(String),
// }
//
// #[derive(Debug)]
// enum Answer {
//     Source(Stat<FileCode>),
//     Parsed(Stat<AST>),
// }

trait Query: fmt::Debug + PartialEq + Eq + hash::Hash {}
//TODO @mark: eq and hash should depend on all dependencies too right?

trait Answer: fmt::Debug + PartialEq + Eq {}

trait CompileStep<Q: Query> {
    type A: Answer;

    fn compile(query: Q) -> Stat<Self::A>;
}

#[derive(Debug)]
struct SourceStep {}

impl CompileStep<FileIden> for SourceStep {
    type A = FileCode;

    fn compile(query: FileIden) -> Stat<FileCode> {
        todo!()
    }
}

trait FullQE {
    fn ir(&mut self, q: TelirQ) {
    }
}

struct QE {}

impl FullQE for QE {}

//TODO @mark: example use
fn compile() {
    let mut qe = QE {};
    qe.ir(TelirQ { name: "my_exe".to_string() });
}
