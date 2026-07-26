//! Spec anchors — the code half of the docs↔implementation link.
//!
//! A *spec anchor* is a `SCREAMING_SNAKE_CASE` id that names one rule in the
//! Tel language documentation (`doc/book/src`). The documentation marks the
//! rule with `{{#spec THE_ID}}`; every piece of code that implements it marks
//! itself with [`spec!`]. `scripts/spec_links.py` then checks that the two
//! sides agree, and can feed the code locations back into the book so a rule
//! links to its implementation and back.
//!
//! ```ignore
//! use tel_common::spec;
//!
//! spec!(SAME_SCOPE_REDECLARATION);
//! spec!(IDENTIFIER_SHAPE, "leading character is checked separately below");
//! ```
//!
//! The macro expands to nothing: an anchor costs nothing at runtime, imposes
//! no naming on the code around it, and only the checker ever reads it. Use it
//! in item or statement position — it is not an expression.
//!
//! One id may be claimed by any number of code sites (a rule is usually split
//! over parser, resolver and error reporting); the checker collects them all.
//!
//! Where a Rust macro cannot go — `.lalrpop` grammars, `.tel` sources, shell
//! or TOML — write the comment form instead, which the checker treats
//! identically:
//!
//! ```text
//! // spec: SAME_SCOPE_REDECLARATION — rejected here, reported in resolve
//! ```
//!
//! spec-links: ignore — the ids above are illustrations rather than real
//! claims, so the checker skips this whole file (any file containing that
//! marker is skipped).

/// Mark the surrounding code as implementing documentation rule `$id`.
///
/// See the [module docs](self) for the convention. Expands to nothing; the
/// id is validated by `scripts/spec_links.py`, not by the compiler.
#[macro_export]
macro_rules! spec {
    ($id:ident $(,)?) => {};
    ($id:ident, $note:literal $(,)?) => {};
}
