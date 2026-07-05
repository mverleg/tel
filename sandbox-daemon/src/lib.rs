//! Daemon + CLI host for the sandbox query compiler — design and rationale
//! in sandbox/plans/daemon.md. The engine stays a library (this crate is a
//! thin host); tonic types stay inside this crate's boundary.

pub mod client;
pub mod discovery;
pub mod server;
pub mod version;

pub mod proto {
    tonic::include_proto!("telsb.daemon.v1");
}
