#![feature(lazy_cell)]

use ::std::fs;
use ::std::io::BufWriter;
use ::std::io::stdout;
use ::std::io::Write;
use ::std::path::Path;
use ::std::path::PathBuf;

use ::log::debug;
use ::log::warn;
use ::serde::Serialize;
use ::serde_json;

use ::tel_api::Identifier;
use ::tel_api::TelFile;

use crate::parser::parse_str;

mod ast;
mod parser;
mod scoping;

#[derive(Debug)]
pub struct BuildArgs {
    pub path: PathBuf,
    pub verbose: bool,
}

pub fn tel_build(args: &BuildArgs) -> Result<(), TelErr> {
    let path = find_main_file(&args.path)?;
    let source = fs::read_to_string(&path)
        .map_err(|err| TelErr::CouldNotRead(path.clone(), err.to_string()))?;
    tel_build_str(path, source, false)
}

#[derive(Debug, Serialize)]
struct DebugInfo<'a> {
    ast: &'a TelFile,
}

pub fn tel_build_str(path: PathBuf, code: String, debug: bool) -> Result<(), TelErr> {
    let prog = parse_str(path, code)?;
    print_debug(debug, &prog);
    Ok(())
}

fn print_debug(debug: bool, file: &TelFile) {
    if !debug {
        return;
    }
    let mut out = BufWriter::new(stdout().lock());
    serde_json::to_writer_pretty(&mut out, &DebugInfo { ast: file }).unwrap();
    out.write_all(&[b'\n']).unwrap();
    out.flush().unwrap()
}

fn find_main_file(path: &Path) -> Result<PathBuf, TelErr> {
    let path = if path.exists() {
        let pth = path.to_owned();
        debug!("select base path as starting point: '{}'", pth.display());
        pth
    } else {
        let pth_ext = path.with_extension("tel");
        if pth_ext.exists() {
            debug!(
                "select path with '.tel' extension added as starting point: '{}'",
                pth_ext.display()
            );
            pth_ext
        } else {
            warn!(
                "did not find file at starting point path '{}' nor at '{}'",
                path.display(),
                pth_ext.display()
            );
            return Err(TelErr::FileNotFound {
                file: path.to_owned(),
            });
        }
    };
    Ok(path)
}

#[derive(Debug, PartialEq)]
pub enum TelErr {
    FileNotFound {
        file: PathBuf,
    },
    CouldNotRead(PathBuf, String),
    ParseErr {
        file: PathBuf,
        line: usize,
        msg: String,
    },
    ScopeErr {
        // file: PathBuf,
        // line: usize,
        //TODO @mark: ^
        msg: String,
    },
    UnknownIdentifier(Identifier),
}
