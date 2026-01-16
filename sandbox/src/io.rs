use std::fs;
use std::io;

pub fn load_file(path: &str) -> Result<String, io::Error> {
    fs::read_to_string(path)
}
