//! Query-flavor wiring (roadmap item 15): opt-level threads from the public
//! `Compiler` API down to the backend-analog (`execute`) without disturbing
//! the front-end. The key-level Option C property (declared flavors key
//! apart, undeclared ones don't) is unit-tested in src/keys.rs; this test
//! proves the setting reaches a real compile and doesn't change results.

use sandbox::{Compiler, OptLevel, Printer};
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

/// A non-default opt-level threads through construction to `flavors()`, and
/// the compile still produces the same result — opt-level is a backend
/// flavor that no cached front-end query depends on, so it changes nothing
/// observable in the sandbox interpreter today.
#[tokio::test]
async fn opt_level_threads_through_and_is_result_neutral() {
    let dir = TempDir::new().unwrap();
    let main = dir.path().join("main.telsb");
    fs::write(&main, "(print (+ 20 22))\n").unwrap();
    let path = main.to_str().unwrap();

    for opt in [OptLevel::Debug, OptLevel::Release] {
        let out = Arc::new(Mutex::new(Vec::new()));
        let printer: Arc<dyn Printer> = Arc::new(RecordingPrinter { out: out.clone() });
        let mut compiler = Compiler::new_with_flavors(printer, sandbox::Flavors { opt });
        assert_eq!(compiler.flavors().opt, opt, "the opt-level must thread to the compiler");
        compiler.run(path, false).await.unwrap();
        assert_eq!(
            out.lock().unwrap().last().cloned().unwrap_or_default(),
            "42",
            "opt-level is result-neutral in the interpreter",
        );
    }
}
