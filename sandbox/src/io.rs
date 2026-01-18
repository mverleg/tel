use std::fs;
use std::io;
use crate::qcompiler2::Context;

pub fn load_file(path: &str, a_ctx: &Context) -> Result<String, io::Error> {
    a_ctx.in_read(path, |_ctx| {
        fs::read_to_string(path)
    })
}
