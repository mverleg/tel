
trait FileSystem {
    fn read(iden: FileIden) -> String;
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
}

#[derive(Debug)]
enum Query<F: FileSystem> {
    Source(FileIden),
    Parsed(FileIden),
    Import(Import),
}

#[derive(Debug)]
enum Answer {
    Source(FileCode),
    Parsed(AST),
}

