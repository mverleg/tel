use std::fs;
use std::io;
use crate::qcompiler2::Context;

pub fn load_file(path: &str, ctx: &mut Context) -> Result<String, io::Error> {
    ctx.in_read(path, |_ctx| {
        fs::read_to_string(path)
    })
}
