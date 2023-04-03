use ::std::fs;
use ::std::path::PathBuf;

mod parser;

#[derive(Debug)]
pub struct BuildArgs {
    pub path: PathBuf,
    pub verbose: bool,
}

pub fn steel_build(args: &BuildArgs) -> Result<(), SteelErr> {
    let path = if args.path.exists() {
        args.path.clone()
    } else {
        let pth_ext = args.path.with_extension(".pest");
        if pth_ext.exists() {
            pth_ext
        } else {
            return Err(SteelErr::FileNotFound(args.path.clone()))
        }
    };
    let source = fs::read_to_string(&path)
        .map_err(|err| SteelErr::CouldNotRead(path, err.to_string()))?;
    let ast = parse_str(&source)?;
    unimplemented!();  //TODO @mark: TEMPORARY! REMOVE THIS!
    Ok(())
}

#[derive(Debug)]
pub enum SteelErr {
    FileNotFound(PathBuf),
    CouldNotRead(PathBuf, String),
}

//TODO @mark:
pub fn parse_str(code: &str) -> Result<(), SteelErr> {
    todo!()
}
