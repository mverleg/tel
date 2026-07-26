//! Query flavors (plans/flavors.md, roadmap item 15).
//!
//! A **flavor** is a dimension of a query's *cache key* that isn't the
//! query's subject (the file or function) but still affects its result — an
//! ambient config the query is answered under. The model is **Option C** of
//! plans/flavors.md: each query kind folds in only the flavors it *declares*
//! a dependence on, so a flavor-independent step (parse, resolve,
//! typecheck/mono) keeps one key across every configuration and never
//! fragments the cache. An ambient env hashed into *all* keys (Option B) was
//! rejected for exactly that fragmentation.
//!
//! Today the only flavor is [`OptLevel`], and — per plans/flavors.md —
//! *no front-end query depends on it*: optimization is a backend concern, so
//! opt-level is threaded to [`crate::context::BackendCtx`] (the sandbox's
//! backend-analog; the interpreter stands in for codegen) and enters no
//! cached key. Parse/resolve/mono keys are opt-invariant by construction —
//! the wiring stops above them, which is the whole point. When a real
//! codegen query lands it declares its flavors by folding the relevant
//! dimensions into its key (see `StableHash for Flavors`).
//!
//! Adding a dimension is a field on [`Flavors`] plus a fold in its
//! `StableHash`; that is the "room for others" the design leaves open
//! (opt-level now; target/debug-info later, with the backend).

use crate::keys::{StableCtx, StableHash, StableHasher};

/// The ambient flavor environment of one compile. Extend with further
/// result-affecting config dimensions as they arrive (target, debug-info,
/// …); keep inputs (which filesystem → content) and query subjects
/// (generics → parameters) *out* — plans/flavors.md is explicit that those
/// are not flavors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Flavors {
    pub opt: OptLevel,
}

/// Optimization level — the first genuine flavor. Result-affecting at
/// codegen (a `Release` build lowers differently than `Debug`), and *only*
/// there: it must never reach parse/resolve/typecheck keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum OptLevel {
    #[default]
    Debug = 0,
    Release = 1,
}

impl StableHash for OptLevel {
    fn stable_hash(&self, _ctx: &StableCtx<'_>, out: &mut StableHasher) {
        out.write_u8(*self as u8);
    }
}

/// Folds every flavor dimension in a fixed order — for a future query that
/// declares dependence on the *whole* ambient set. A query depending on a
/// subset folds those dimensions individually instead (e.g. only `opt`);
/// that selectivity is Option C, and it is why parse/resolve/mono, which
/// fold nothing here, stay shared across flavors.
impl StableHash for Flavors {
    fn stable_hash(&self, ctx: &StableCtx<'_>, out: &mut StableHasher) {
        self.opt.stable_hash(ctx, out);
    }
}
