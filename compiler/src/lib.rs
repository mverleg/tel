use ::std::fs;
use ::std::path::Path;
use ::std::path::PathBuf;

use ::steel_api::log::debug;
use ::steel_api::log::warn;

use crate::parser::{parse_str, StructParser};

mod parser;

#[derive(Debug)]
pub struct BuildArgs {
    pub path: PathBuf,
    pub verbose: bool,
}

pub fn steel_build(args: &BuildArgs) -> Result<(), SteelErr> {
    let path = find_main_file(&args.path)?;
    let source = fs::read_to_string(&path)
        .map_err(|err| SteelErr::CouldNotRead(path, err.to_string()))?;
    let ast = parse_str(&source)?;
    unimplemented!();  //TODO @mark: TEMPORARY! REMOVE THIS!
    Ok(())
}

fn find_main_file(path: &Path) -> Result<PathBuf, SteelErr> {
    let path = if path.exists() {
        let pth = path.to_owned();
        debug!("select base path as starting point: '{}'", pth.display());
        pth
    } else {
        let pth_ext = path.with_extension("steel");
        if pth_ext.exists() {
            debug!("select path with '.steel' extension added as starting point: '{}'", pth_ext.display());
            pth_ext
        } else {
            warn!("did not find file at starting point path '{}' nor at '{}'",
                path.display(), pth_ext.display());
            return Err(SteelErr::FileNotFound(path.to_owned()))
        }
    };
    Ok(path)
}

#[derive(Debug)]
pub enum SteelErr {
    FileNotFound(PathBuf),
    CouldNotRead(PathBuf, String),
}
