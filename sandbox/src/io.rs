use std::fs;
use std::io;
use crate::qcompiler2::CompilationLog;

pub fn load_file(path: &str, a_log: &mut CompilationLog) -> Result<String, io::Error> {
    a_log.in_read(path, |_log| {
        fs::read_to_string(path)
    })
}
