mod step;
mod engine;
mod source;
mod parse;

trait FileSystem {
    fn read(iden: FileIden) -> String;
}

#[derive(Debug, Clone, Copy)]
enum Error {
    SourceNotFound
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

//TODO @mark: example use
fn compile() {
    let mut qe = QE {};
    qe.ir(TelirQ { name: "my_exe".to_string() });
}
