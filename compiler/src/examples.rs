use std::fs::read_to_string;
use std::path::PathBuf;
use tel_ast::ParseErr;
use tel_common::TelErr;
use tel_parser::str_to_ast;
use crate::ast_to_api;

include!(concat!(env!("OUT_DIR"), "/parse_tests.rs"));

#[derive(Debug, Clone, Copy)]
struct Mode {
    parse_only: bool,  // TODO @mark: this one is temporary
    should_fail: bool,
}

fn get_test_modes(code: &str) -> Mode {
    let mut mode = Mode { parse_only: false, should_fail: false };
    if !code.starts_with("#") {
        return mode
    }
    for part in code.lines().next().expect("there is at least one line").split(" ") {
        match part.trim() {
            "#" | "tel-test:" => {},
            "parse-only" => mode.parse_only = true,
            "should-fail" => mode.should_fail = true,
            unknown => panic!("unknown mode comment at top of test: {unknown}"),
        }
    }
    mode
}

