//! Type check + monomorphisation tests.
//!
//! Each function has a single type parameter `T: Number` covering all its
//! arguments and its return value, so it monomorphises to at most an i32 and
//! an i64 instance. There are no implicit conversions between numeric types.

use sandbox::{run_file_with_printer, Printer};
use std::fs;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

struct RecordingPrinter {
    out: Arc<Mutex<Vec<String>>>,
}

impl Printer for RecordingPrinter {
    fn print(&self, message: &str) {
        self.out.lock().unwrap().push(message.to_string());
    }
}

/// Write `files` into a temp dir and run `main.telsb`, returning the error (if
/// any) rendered to a string, and everything that was printed.
async fn run_program(files: &[(&str, &str)]) -> (Result<(), String>, Vec<String>) {
    let dir = TempDir::new().unwrap();
    for (name, content) in files {
        fs::write(dir.path().join(name), content).unwrap();
    }
    let out = Arc::new(Mutex::new(Vec::new()));
    let printer: Arc<dyn Printer> = Arc::new(RecordingPrinter { out: out.clone() });
    let main = dir.path().join("main.telsb");
    let result = run_file_with_printer(main.to_str().unwrap(), false, printer)
        .await
        .map_err(|e| e.to_string());
    let printed = out.lock().unwrap().clone();
    (result, printed)
}

#[tokio::test]
async fn generic_function_monomorphises_to_both_types() {
    let (result, printed) = run_program(&[
        ("main.telsb", "(import double)\n(print (call double 21))\n(print (call double 21i32))\n"),
        ("double.telsb", "(* (arg 1) 2)\n"),
    ]).await;
    assert!(result.is_ok(), "expected success, got: {:?}", result.err());
    assert_eq!(printed, vec!["42", "42"]);
}

#[tokio::test]
async fn recursion_works_in_both_instances() {
    let (result, printed) = run_program(&[
        ("main.telsb", "(import fact)\n(print (call fact 5))\n(print (call fact 5i32))\n"),
        ("fact.telsb", "(let n (arg 1))\n(if (< n 2) (return n) (return (* n (call fact (- n 1)))))\n"),
    ]).await;
    assert!(result.is_ok(), "expected success, got: {:?}", result.err());
    assert_eq!(printed, vec!["120", "120"]);
}

#[tokio::test]
async fn i64_only_value_works_with_suffix() {
    let (result, printed) = run_program(&[
        ("main.telsb", "(let big 1000000000000i64)\n(print (/ big 1000000))\n"),
    ]).await;
    assert!(result.is_ok(), "expected success, got: {:?}", result.err());
    assert_eq!(printed, vec!["1000000"]);
}

#[tokio::test]
async fn mixing_types_in_binary_op_is_rejected() {
    let (result, _) = run_program(&[
        ("main.telsb", "(print (+ 1i32 2i64))\n"),
    ]).await;
    let err = result.unwrap_err();
    assert!(err.contains("Type mismatch"), "unexpected error: {}", err);
}

#[tokio::test]
async fn mixing_types_across_call_arguments_is_rejected() {
    // All arguments of a call share the callee's single type parameter.
    let (result, _) = run_program(&[
        ("main.telsb", "(function f (+ (arg 1) (arg 2)))\n(print (call f 1i32 2i64))\n"),
    ]).await;
    let err = result.unwrap_err();
    assert!(err.contains("Type mismatch"), "unexpected error: {}", err);
}

#[tokio::test]
async fn reassignment_must_keep_the_variable_type() {
    let (result, _) = run_program(&[
        ("main.telsb", "(let x 1i64)\n(set x 2i32)\n(print x)\n"),
    ]).await;
    let err = result.unwrap_err();
    assert!(err.contains("Type mismatch"), "unexpected error: {}", err);
}

#[tokio::test]
async fn if_branches_must_agree_on_type() {
    let (result, _) = run_program(&[
        ("main.telsb", "(let y (if (> 1 0) 1i32 2i64))\n(print y)\n"),
    ]).await;
    let err = result.unwrap_err();
    assert!(err.contains("Type mismatch"), "unexpected error: {}", err);
}

#[tokio::test]
async fn call_result_type_propagates_to_caller() {
    // r is i32 because ident was instantiated at i32; adding an i64 must fail.
    let (result, _) = run_program(&[
        ("main.telsb", "(function ident (arg 1))\n(let r (call ident 3i32))\n(print (+ r 1i64))\n"),
    ]).await;
    let err = result.unwrap_err();
    assert!(err.contains("Type mismatch"), "unexpected error: {}", err);
}

#[tokio::test]
async fn unsuffixed_literal_must_fit_its_inferred_type() {
    // 5000000000 is forced to i32 by unification but does not fit.
    let (result, _) = run_program(&[
        ("main.telsb", "(print (+ 5000000000 1i32))\n"),
    ]).await;
    let err = result.unwrap_err();
    assert!(err.contains("does not fit"), "unexpected error: {}", err);
}

#[tokio::test]
async fn suffixed_literal_out_of_range_is_a_parse_error() {
    let (result, _) = run_program(&[
        ("main.telsb", "(print 5000000000i32)\n"),
    ]).await;
    let err = result.unwrap_err();
    assert!(err.contains("out of range for i32"), "unexpected error: {}", err);
}

#[tokio::test]
async fn unknown_numeric_suffix_is_a_parse_error() {
    let (result, _) = run_program(&[
        ("main.telsb", "(print 5u8)\n"),
    ]).await;
    let err = result.unwrap_err();
    assert!(err.contains("Invalid number"), "unexpected error: {}", err);
}

#[tokio::test]
async fn zero_arg_call_defaults_to_the_enclosing_instance_type() {
    // With no arguments to pin it, `(call five)` is instantiated at the
    // enclosing instance's type (i64 in main), so mixing with i32 fails...
    let (result, _) = run_program(&[
        ("main.telsb", "(function five 5)\n(print (+ (call five) 1i32))\n"),
    ]).await;
    let err = result.unwrap_err();
    assert!(err.contains("Type mismatch"), "unexpected error: {}", err);

    // ...while the i64 default works.
    let (result, printed) = run_program(&[
        ("main.telsb", "(function five 5)\n(print (+ (call five) 1i64))\n"),
    ]).await;
    assert!(result.is_ok(), "expected success, got: {:?}", result.err());
    assert_eq!(printed, vec!["6"]);
}

#[tokio::test]
async fn division_by_zero_is_still_caught_per_type() {
    let (result, _) = run_program(&[
        ("main.telsb", "(print (/ 1i32 0i32))\n"),
    ]).await;
    let err = result.unwrap_err();
    assert!(err.contains("Division by zero"), "unexpected error: {}", err);
}
