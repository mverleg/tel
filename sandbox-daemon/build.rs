use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    // Only proto changes and this file trigger a rerun, so TELSB_BUILD_ID is
    // stable across unrelated rebuilds — but any change to the protocol or
    // build script mints a new id, which the exact-version handshake turns
    // into a self-healing daemon restart (plans/daemon.md "Versioning").
    println!("cargo:rerun-if-changed=proto/daemon.proto");
    println!("cargo:rerun-if-changed=build.rs");

    let build_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before 1970")
        .as_nanos();
    println!("cargo:rustc-env=TELSB_BUILD_ID={:x}", build_id);

    tonic_build::configure()
        .build_client(true)
        .build_server(true)
        .compile_protos(&["proto/daemon.proto"], &["proto"])
        .expect("failed to compile proto/daemon.proto (is protoc installed?)");
}
