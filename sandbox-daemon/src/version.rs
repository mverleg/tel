//! The exact-version handshake fingerprint (plans/daemon.md "Versioning").
//!
//! Client and daemon must be the exact same build. The fingerprint folds in
//! the cache schema version — the engine's own "do these builds agree on
//! data layout" token — plus the package version and a per-build id minted
//! by build.rs, so even a rebuild at the same version counts as a different
//! build (which is what "exact" has to mean while messages can change
//! freely).

pub fn build_fingerprint() -> String {
    format!(
        "{}+schema{}+{}",
        env!("CARGO_PKG_VERSION"),
        sandbox::SCHEMA_VERSION,
        env!("TELSB_BUILD_ID"),
    )
}
