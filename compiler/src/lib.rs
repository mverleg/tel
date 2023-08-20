#![feature(lazy_cell)]

use ::std::fs;
use ::std::path::Path;
use ::std::path::PathBuf;
use std::io::{stdout, BufWriter, Write};

use crate::ast::Ast;
use ::serde::Serialize;
use ::serde_json;
use ::steel_api::log::debug;
use ::steel_api::log::warn;

use crate::parser::parse_str;

mod ast;
mod parser;

#[derive(Debug)]
pub struct BuildArgs {
    pub path: PathBuf,
    pub verbose: bool,
}

pub fn steel_build(args: &BuildArgs) -> Result<(), SteelErr> {
    let path = find_main_file(&args.path)?;
    let source = fs::read_to_string(&path)
        .map_err(|err| SteelErr::CouldNotRead(path.clone(), err.to_string()))?;
    steel_build_str(path, &source, false)
}

#[derive(Debug, Serialize)]
struct DebugInfo<'a> {
    ast: &'a Ast,
}

pub fn steel_build_str(path: PathBuf, source: &str, debug: bool) -> Result<(), SteelErr> {
    let ast = parse_str(path, source)?;
    debug!("{:?}", ast);
    print_debug(debug, &ast);
    Ok(())
}

fn print_debug(debug: bool, ast: &Ast) {
    if !debug {
        return;
    }
    let mut out = BufWriter::new(stdout().lock());
    serde_json::to_writer_pretty(&mut out, &DebugInfo { ast }).unwrap();
    out.write_all(&[b'\n']).unwrap();
    out.flush().unwrap()
}

fn find_main_file(path: &Path) -> Result<PathBuf, SteelErr> {
    let path = if path.exists() {
        let pth = path.to_owned();
        debug!("select base path as starting point: '{}'", pth.display());
        pth
    } else {
        let pth_ext = path.with_extension("steel");
        if pth_ext.exists() {
            debug!(
                "select path with '.steel' extension added as starting point: '{}'",
                pth_ext.display()
            );
            pth_ext
        } else {
            warn!(
                "did not find file at starting point path '{}' nor at '{}'",
                path.display(),
                pth_ext.display()
            );
            return Err(SteelErr::FileNotFound {
                file: path.to_owned(),
            });
        }
    };
    Ok(path)
}

#[derive(Debug, PartialEq)]
pub enum SteelErr {
    FileNotFound {
        file: PathBuf,
    },
    CouldNotRead(PathBuf, String),
    ParseErr {
        file: PathBuf,
        line: usize,
        msg: String,
    },
}
