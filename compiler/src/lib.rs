use ::std::fs;
use ::std::path::PathBuf;

#[derive(Debug)]
pub struct BuildArgs {
    pub path: PathBuf,
    pub verbose: bool,
}

pub fn steel_build(args: &BuildArgs) -> Result<(), SteelErr> {
    if ! args.path.exists() {
        return Err(SteelErr::FileNotFound(args.path.clone()))
    }
    let source = fs::read_to_string(&args.path)
        .map_err(|err| SteelErr::CouldNotRead(args.path.clone(), err.to_string()))?;
    let ast = parse_str(&source)?;
    unimplemented!();  //TODO @mark: TEMPORARY! REMOVE THIS!
    Ok(())
}

#[derive(Debug)]
enum SteelErr {
    FileNotFound(PathBuf),
    CouldNotRead(PathBuf, String),
}

//TODO @mark:
pub fn parse_str(code: &str) -> Result<AST, String> {
    todo!()
}
