use crate::scoping::ast_to_api;
use std::fs;
use std::io::stdout;
use std::io::BufWriter;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use log::debug;
use log::warn;
use serde::Serialize;
use tel_ast::{ParseErr, TelFile};
use tel_common::TelErr;
use tel_parser::str_to_ast;

mod scoping;
mod examples;

pub fn parse_str(src_pth: PathBuf, code: String) -> Result<TelFile, TelErr> {
    let ast = str_to_ast(src_pth, code).map_err(parse_err_to_tel_err)?;
    ast_to_api(ast)
}

fn parse_err_to_tel_err(err: ParseErr) -> TelErr {
    match err {
        ParseErr::FileNotFound { file } => TelErr::FileNotFound { file },
        ParseErr::CouldNotRead(path, msg) => TelErr::CouldNotRead(path, msg),
        ParseErr::ParseErr { file, line, msg } => TelErr::ParseErr { file, line, msg },
        ParseErr::ScopeErr { msg } => TelErr::ScopeErr { msg },
        ParseErr::UnknownIdentifier(iden) => TelErr::UnknownIdentifier(iden),
    }
}

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
    out.write_all(b"\n").unwrap();
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
