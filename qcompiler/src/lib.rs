
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

#[derive(Debug)]
enum Query<F: FileSystem> {
    Source(FileIden),
    Parsed(FileIden),
    Import(Import),
}

#[derive(Debug)]
enum Answer {
    Source(Stat<FileCode>),
    Parsed(Stat<AST>),
}
