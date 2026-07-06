//! The three-identifier key model from docs/keys-and-invalidation.md.
//!
//! Every query (compiler step instance) has three identifiers with distinct jobs:
//!
//! - **Logical id** — `(query kind, args)`, e.g. "parse file F". Process-local,
//!   may use interned indices freely; it names the query across time and edits.
//!   In this crate the logical id is [`crate::graph::StepId`].
//! - **[`ContentKey`]** — `hash(SCHEMA_VERSION, kind, stable args, dep result
//!   fingerprints)`. The lookup key into the (eventually persistent) cache.
//!   Built exclusively from stable data, never from interner indices.
//! - **[`Fingerprint`]** — `hash(direct output)`. The ingredient of dependents'
//!   content keys; enables early cutoff.
//!
//! Everything that enters a [`ContentKey`] or [`Fingerprint`] goes through the
//! [`StableHash`] trait, which (unlike `std::hash::Hash`) guarantees a
//! deterministic byte encoding and resolves process-local ids ([`Sym`]) to
//! their stable string form via [`StableCtx`] — see
//! docs/deterministic-hashing.md. `std::hash::Hash` remains in use for
//! in-memory maps, where nondeterminism is harmless.

use crate::common::{Interner, Name, Path, Sym, FQ};
use crate::types::{BinOp, Expr, FuncId, MExpr, MonoFuncData, PreExpr, ScopeId, SymbolTable, Ty, Value, VarId, VarInfo};
use xxhash_rust::xxh3::Xxh3;

/// Version of the compiler's data formats and semantics, folded into every
/// [`ContentKey`] by construction (the README item "include schema hash in the
/// file cache"). Bump it whenever a change makes previously cached results
/// unusable — a different AST shape, different phase semantics, a different
/// `StableHash` encoding. Old entries are then unreachable (superseded garbage,
/// not hazards): a cold cache, never a stale one.
/// History: 2 — `StableHasher` swapped `DefaultHasher` → xxh3, keys widened
/// to 128 bits.
pub const SCHEMA_VERSION: u64 = 2;

/// The query kind tag. Folding it into every content key puts each phase in a
/// disjoint keyspace (docs/keys-and-invalidation.md "Per-Phase Keyspaces"): a
/// parse key can never alias a mono key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QueryKind {
    Parse = 1,
    Resolve = 2,
    Mono = 3,
    Exec = 4,
}

/// Context for stable hashing: carries whatever is needed to map process-local
/// ids to their stable forms (today: the interner, to turn a [`Sym`] index back
/// into its string). Mirrors the `CtxDisplay` pattern.
pub struct StableCtx<'a> {
    pub interner: &'a Interner,
}

/// The single place the hash algorithm lives.
///
/// Backed by xxh3 per docs/hashing.md: [`finish128`](StableHasher::finish128)
/// for content keys and content digests (global, ever-accumulating keyspace),
/// [`finish64`](StableHasher::finish64) for result fingerprints (bounded
/// per-slot domain) — one streaming state serves both widths. Unlike the
/// previous `DefaultHasher` backing, xxh3's output is *specified*, stable
/// across toolchains and platforms, which is what lets keys outlive the
/// process (persistent cache) and lets golden-hash tests exist. Nothing
/// outside this struct knows the algorithm.
///
/// The byte *encoding* is ours and stable (fixed-width little-endian
/// integers, length-prefixed strings and sequences, tagged enums — the rules
/// in docs/deterministic-hashing.md), so an algorithm swap never changes a
/// `StableHash` impl; the golden tests below pin encoding and algorithm both.
pub struct StableHasher {
    inner: Xxh3,
}

impl StableHasher {
    fn new() -> StableHasher {
        // Unseeded (`new`, not `with_seed`): the specified, stable variant.
        StableHasher { inner: Xxh3::new() }
    }

    fn finish64(&self) -> u64 {
        self.inner.digest()
    }

    fn finish128(&self) -> u128 {
        self.inner.digest128()
    }

    pub fn write_u8(&mut self, v: u8) {
        self.inner.update(&[v]);
    }

    pub fn write_u32(&mut self, v: u32) {
        self.inner.update(&v.to_le_bytes());
    }

    pub fn write_u64(&mut self, v: u64) {
        self.inner.update(&v.to_le_bytes());
    }

    pub fn write_u128(&mut self, v: u128) {
        self.inner.update(&v.to_le_bytes());
    }

    pub fn write_i32(&mut self, v: i32) {
        self.inner.update(&v.to_le_bytes());
    }

    pub fn write_i64(&mut self, v: i64) {
        self.inner.update(&v.to_le_bytes());
    }

    /// Length as `u64` — the one sanctioned way to hash a `usize`, so 32- and
    /// 64-bit platforms agree.
    pub fn write_len(&mut self, len: usize) {
        self.write_u64(len as u64);
    }

    /// Length-prefixed bytes. The prefix prevents concatenation ambiguity:
    /// `("ab", "c")` must not hash like `("a", "bc")`.
    pub fn write_bytes(&mut self, bytes: &[u8]) {
        self.write_len(bytes.len());
        self.inner.update(bytes);
    }

    pub fn write_str(&mut self, s: &str) {
        self.write_bytes(s.as_bytes());
    }
}

/// Deterministic hashing for fingerprint/content-key inputs.
///
/// Deliberately *not* implemented for nondeterministic types: no raw pointers,
/// no `HashMap`/`HashSet` (iteration order), no bare `usize`, and [`Sym`]
/// hashes its resolved *string*, never its process-local index. A type without
/// an impl failing to compile is the guardrail working as intended.
pub trait StableHash {
    fn stable_hash(&self, ctx: &StableCtx<'_>, out: &mut StableHasher);
}

/// Content key: the cache lookup identity of one query instance
/// (docs/keys-and-invalidation.md). `hash(SCHEMA_VERSION, kind, stable args,
/// direct deps' result fingerprints)` — transitive by recursion through the dep
/// fingerprints, forming a Merkle DAG over *answers*.
///
/// 128 bits (xxh3-128): content keys live in one unbounded, ever-accumulating
/// keyspace — every distinct computation ever seen — which is birthday
/// territory for 64 bits (docs/hashing.md). A distinct newtype from
/// [`Fingerprint`] so the two can never be mixed up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ContentKey(u128);

impl ContentKey {
    /// Build a content key. `SCHEMA_VERSION` and the kind tag are folded in
    /// here, by construction, so no call site can forget them; `fill` writes
    /// the stable args and dep fingerprints.
    pub fn build(kind: QueryKind, fill: impl FnOnce(&mut StableHasher)) -> ContentKey {
        ContentKey::build_with_schema(SCHEMA_VERSION, kind, fill)
    }

    /// Same, with an explicit schema version — exists so tests can prove that a
    /// version bump changes every key. Production code uses [`ContentKey::build`].
    fn build_with_schema(schema: u64, kind: QueryKind, fill: impl FnOnce(&mut StableHasher)) -> ContentKey {
        let mut h = StableHasher::new();
        h.write_u64(schema);
        h.write_u32(kind as u32);
        fill(&mut h);
        ContentKey(h.finish128())
    }

    /// Raw hash bits, for the step tracer's log lines only — never a stable
    /// serialization or a lookup key outside this process.
    #[cfg(feature = "step-trace")]
    pub(crate) fn raw(self) -> u128 {
        self.0
    }

}

/// Result fingerprint: `hash(direct output)` — and only the direct output;
/// given deterministic steps, anything transitive is already reachable through
/// the dep fingerprints inside the content key.
///
/// 64-bit by design (its collision domain is the historical outputs of one
/// logical query — docs/hashing.md). Invariant: never a storage or lookup key;
/// results are stored only *under* content keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Fingerprint(u64);

impl Fingerprint {
    pub fn of<T: StableHash + ?Sized>(value: &T, ctx: &StableCtx<'_>) -> Fingerprint {
        let mut h = StableHasher::new();
        value.stable_hash(ctx, &mut h);
        Fingerprint(h.finish64())
    }

    /// Fingerprint of a *successful* answer. Deterministic errors are answers
    /// too (invariant 6), so success and failure share one fingerprint space —
    /// a `Result` variant tag keeps an `Ok` from ever aliasing an `Err` in the
    /// same dependency slot. All step-output fingerprints go through
    /// [`Fingerprint::of_ok`] / [`Fingerprint::of_err`]; plain
    /// [`Fingerprint::of`] is for hashing *ingredients*, not step answers.
    pub fn of_ok<T: StableHash + ?Sized>(value: &T, ctx: &StableCtx<'_>) -> Fingerprint {
        let mut h = StableHasher::new();
        h.write_u32(0); // Result variant tag: Ok
        value.stable_hash(ctx, &mut h);
        Fingerprint(h.finish64())
    }

    /// Fingerprint of a deterministic *error* answer — see
    /// [`Fingerprint::of_ok`]. Only terminal errors may be fingerprinted;
    /// transient failures (IO, joins) and cycle errors are not answers and
    /// never get one.
    pub fn of_err<E: StableHash + ?Sized>(error: &E, ctx: &StableCtx<'_>) -> Fingerprint {
        let mut h = StableHasher::new();
        h.write_u32(1); // Result variant tag: Err
        error.stable_hash(ctx, &mut h);
        Fingerprint(h.finish64())
    }

    /// Raw hash bits, for the step tracer's log lines only — see
    /// [`ContentKey::raw`].
    #[cfg(feature = "step-trace")]
    pub(crate) fn raw(self) -> u64 {
        self.0
    }

    /// Raw bits for the persistence boundary only: a stored answer carries
    /// its fingerprint so dependents can key on a disk hit without
    /// re-hashing. Never a storage or lookup key — the invariant above.
    pub(crate) fn raw_bits(self) -> u64 {
        self.0
    }

    /// Inverse of [`raw_bits`](Fingerprint::raw_bits), for decoding stored
    /// answers.
    pub(crate) fn from_bits(bits: u64) -> Fingerprint {
        Fingerprint(bits)
    }
}

/// Fingerprints enter dependents' content-key preimages by value — that is
/// their entire job ("hash(kind, stable args, dep result fingerprints)").
impl StableHash for Fingerprint {
    fn stable_hash(&self, _ctx: &StableCtx<'_>, out: &mut StableHasher) {
        out.write_u64(self.0);
    }
}

/// A content-addressed digest of a source file's bytes — the `input_state` of
/// the model: the external input a leaf query's content key hashes. Not itself
/// a [`ContentKey`] (no kind, no schema); it is an *ingredient* of the parse
/// key, and later stages chain to the parse stage through it.
///
/// Keying on content instead of path is what makes the cache safe across
/// source changes (docs/cache-invalidation-problem.md): changed bytes make a
/// different digest (never a stale hit), reverted bytes hit the old entry
/// (Scenario A).
/// 128-bit like [`ContentKey`], not 64-bit like [`Fingerprint`]: the digest
/// is the *entire* variable part of a parse key's preimage (parse keys are
/// path-free), so it is compared in the same global keyspace of all distinct
/// file contents ever seen — a 64-bit digest would silently cap the 128-bit
/// parse key at 64 bits of entropy. It lives only in memory
/// (`BindingRecord.input_digest` is never persisted), so the width is free.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ContentDigest(u128);

impl ContentDigest {
    pub fn of(source: &str) -> ContentDigest {
        let mut h = StableHasher::new();
        h.write_str(source);
        ContentDigest(h.finish128())
    }
}

impl StableHash for ContentDigest {
    fn stable_hash(&self, _ctx: &StableCtx<'_>, out: &mut StableHasher) {
        out.write_u128(self.0);
    }
}

// ---- StableHash impls: primitives and containers ----------------------------
//
// Encoding rules (docs/deterministic-hashing.md): fixed-width little-endian
// integers; length-prefixed strings, bytes, and sequences; enums as a u32
// variant tag then the payload; structs/tuples field by field in declaration
// order. There is intentionally no impl for usize, raw pointers, floats, or
// HashMap/HashSet.

impl StableHash for u8 {
    fn stable_hash(&self, _ctx: &StableCtx<'_>, out: &mut StableHasher) {
        out.write_u8(*self);
    }
}

impl StableHash for u32 {
    fn stable_hash(&self, _ctx: &StableCtx<'_>, out: &mut StableHasher) {
        out.write_u32(*self);
    }
}

impl StableHash for u64 {
    fn stable_hash(&self, _ctx: &StableCtx<'_>, out: &mut StableHasher) {
        out.write_u64(*self);
    }
}

impl StableHash for i32 {
    fn stable_hash(&self, _ctx: &StableCtx<'_>, out: &mut StableHasher) {
        out.write_i32(*self);
    }
}

impl StableHash for i64 {
    fn stable_hash(&self, _ctx: &StableCtx<'_>, out: &mut StableHasher) {
        out.write_i64(*self);
    }
}

impl StableHash for bool {
    fn stable_hash(&self, _ctx: &StableCtx<'_>, out: &mut StableHasher) {
        out.write_u8(*self as u8);
    }
}

impl StableHash for str {
    fn stable_hash(&self, _ctx: &StableCtx<'_>, out: &mut StableHasher) {
        out.write_str(self);
    }
}

impl StableHash for String {
    fn stable_hash(&self, ctx: &StableCtx<'_>, out: &mut StableHasher) {
        self.as_str().stable_hash(ctx, out);
    }
}

impl<T: StableHash + ?Sized> StableHash for &T {
    fn stable_hash(&self, ctx: &StableCtx<'_>, out: &mut StableHasher) {
        (*self).stable_hash(ctx, out);
    }
}

impl<T: StableHash + ?Sized> StableHash for Box<T> {
    fn stable_hash(&self, ctx: &StableCtx<'_>, out: &mut StableHasher) {
        self.as_ref().stable_hash(ctx, out);
    }
}

impl<T: StableHash> StableHash for [T] {
    fn stable_hash(&self, ctx: &StableCtx<'_>, out: &mut StableHasher) {
        out.write_len(self.len());
        for item in self {
            item.stable_hash(ctx, out);
        }
    }
}

impl<T: StableHash> StableHash for Vec<T> {
    fn stable_hash(&self, ctx: &StableCtx<'_>, out: &mut StableHasher) {
        self.as_slice().stable_hash(ctx, out);
    }
}

/// Tuples hash field by field, like structs; fixed arity needs no length
/// prefix.
impl<A: StableHash, B: StableHash> StableHash for (A, B) {
    fn stable_hash(&self, ctx: &StableCtx<'_>, out: &mut StableHasher) {
        self.0.stable_hash(ctx, out);
        self.1.stable_hash(ctx, out);
    }
}

impl<T: StableHash> StableHash for Option<T> {
    fn stable_hash(&self, ctx: &StableCtx<'_>, out: &mut StableHasher) {
        match self {
            None => out.write_u32(0),
            Some(v) => {
                out.write_u32(1);
                v.stable_hash(ctx, out);
            }
        }
    }
}

// ---- StableHash impls: interned ids (the repo-specific trap) ----------------

/// The reason [`StableHash`] takes a context at all: a `Sym` is a `u32` index
/// assigned in interning order, which differs across runs and schedules. Its
/// derived `Hash` (on the index) stays correct for in-memory maps; for
/// fingerprints we resolve and hash the *string*.
impl StableHash for Sym {
    fn stable_hash(&self, ctx: &StableCtx<'_>, out: &mut StableHasher) {
        out.write_str(ctx.interner.resolve(*self));
    }
}

impl StableHash for Name {
    fn stable_hash(&self, ctx: &StableCtx<'_>, out: &mut StableHasher) {
        self.sym().stable_hash(ctx, out);
    }
}

impl StableHash for Path {
    fn stable_hash(&self, ctx: &StableCtx<'_>, out: &mut StableHasher) {
        self.sym().stable_hash(ctx, out);
    }
}

impl StableHash for FQ {
    fn stable_hash(&self, ctx: &StableCtx<'_>, out: &mut StableHasher) {
        self.path().stable_hash(ctx, out);
        self.name().stable_hash(ctx, out);
    }
}

// ---- StableHash impls: step outputs ------------------------------------------
//
// These make the outputs of today's phases fingerprintable, so later steps can
// key dependents on *output* digests (early cutoff) instead of source bytes.

impl StableHash for Ty {
    fn stable_hash(&self, _ctx: &StableCtx<'_>, out: &mut StableHasher) {
        out.write_u32(match self {
            Ty::I32 => 0,
            Ty::I64 => 1,
        });
    }
}

impl StableHash for BinOp {
    fn stable_hash(&self, _ctx: &StableCtx<'_>, out: &mut StableHasher) {
        out.write_u32(match self {
            BinOp::Add => 0,
            BinOp::Sub => 1,
            BinOp::Mul => 2,
            BinOp::Div => 3,
            BinOp::Greater => 4,
            BinOp::Less => 5,
            BinOp::Equal => 6,
            BinOp::And => 7,
            BinOp::Or => 8,
        });
    }
}

impl StableHash for Value {
    fn stable_hash(&self, _ctx: &StableCtx<'_>, out: &mut StableHasher) {
        match self {
            Value::I32(v) => {
                out.write_u32(0);
                out.write_i32(*v);
            }
            Value::I64(v) => {
                out.write_u32(1);
                out.write_i64(*v);
            }
        }
    }
}

impl StableHash for VarId {
    fn stable_hash(&self, _ctx: &StableCtx<'_>, out: &mut StableHasher) {
        // VarIds are densely allocated in resolution order of one function,
        // which is deterministic given the same input — unlike Sym, the index
        // itself is the stable meaning.
        out.write_u64(self.0 as u64);
    }
}

impl StableHash for ScopeId {
    fn stable_hash(&self, _ctx: &StableCtx<'_>, out: &mut StableHasher) {
        out.write_u64(self.0 as u64);
    }
}

impl StableHash for FuncId {
    fn stable_hash(&self, ctx: &StableCtx<'_>, out: &mut StableHasher) {
        self.0.stable_hash(ctx, out);
    }
}

impl StableHash for crate::graph::MonoId {
    fn stable_hash(&self, ctx: &StableCtx<'_>, out: &mut StableHasher) {
        self.func_loc.stable_hash(ctx, out);
        self.ty.stable_hash(ctx, out);
    }
}

impl StableHash for PreExpr {
    fn stable_hash(&self, ctx: &StableCtx<'_>, out: &mut StableHasher) {
        match self {
            PreExpr::Number { value, ty } => {
                out.write_u32(0);
                value.stable_hash(ctx, out);
                ty.stable_hash(ctx, out);
            }
            PreExpr::Ident(name) => {
                out.write_u32(1);
                name.stable_hash(ctx, out);
            }
            PreExpr::BinaryOp { op, left, right } => {
                out.write_u32(2);
                op.stable_hash(ctx, out);
                left.stable_hash(ctx, out);
                right.stable_hash(ctx, out);
            }
            PreExpr::Let { name, value } => {
                out.write_u32(3);
                name.stable_hash(ctx, out);
                value.stable_hash(ctx, out);
            }
            PreExpr::Set { name, value } => {
                out.write_u32(4);
                name.stable_hash(ctx, out);
                value.stable_hash(ctx, out);
            }
            PreExpr::If { cond, then_branch, else_branch } => {
                out.write_u32(5);
                cond.stable_hash(ctx, out);
                then_branch.stable_hash(ctx, out);
                else_branch.stable_hash(ctx, out);
            }
            PreExpr::Print(inner) => {
                out.write_u32(6);
                inner.stable_hash(ctx, out);
            }
            PreExpr::Return(inner) => {
                out.write_u32(7);
                inner.stable_hash(ctx, out);
            }
            // Fieldless by design (nothing path-derived in the shared parse
            // answer); the tags alone distinguish them.
            PreExpr::Panic => {
                out.write_u32(8);
            }
            PreExpr::Unreachable => {
                out.write_u32(9);
            }
            PreExpr::Import(name) => {
                out.write_u32(10);
                name.stable_hash(ctx, out);
            }
            PreExpr::FunctionDef { name, body } => {
                out.write_u32(11);
                name.stable_hash(ctx, out);
                body.stable_hash(ctx, out);
            }
            PreExpr::Call { func, args } => {
                out.write_u32(12);
                func.stable_hash(ctx, out);
                args.stable_hash(ctx, out);
            }
            PreExpr::Arg(n) => {
                out.write_u32(13);
                n.stable_hash(ctx, out);
            }
            PreExpr::Sequence(items) => {
                out.write_u32(14);
                items.stable_hash(ctx, out);
            }
        }
    }
}

impl StableHash for Expr {
    fn stable_hash(&self, ctx: &StableCtx<'_>, out: &mut StableHasher) {
        match self {
            Expr::Number { value, ty } => {
                out.write_u32(0);
                value.stable_hash(ctx, out);
                ty.stable_hash(ctx, out);
            }
            Expr::VarRef(var) => {
                out.write_u32(1);
                var.stable_hash(ctx, out);
            }
            Expr::BinaryOp { op, left, right } => {
                out.write_u32(2);
                op.stable_hash(ctx, out);
                left.stable_hash(ctx, out);
                right.stable_hash(ctx, out);
            }
            Expr::Let { var, value } => {
                out.write_u32(3);
                var.stable_hash(ctx, out);
                value.stable_hash(ctx, out);
            }
            Expr::Set { var, value } => {
                out.write_u32(4);
                var.stable_hash(ctx, out);
                value.stable_hash(ctx, out);
            }
            Expr::If { cond, then_branch, else_branch } => {
                out.write_u32(5);
                cond.stable_hash(ctx, out);
                then_branch.stable_hash(ctx, out);
                else_branch.stable_hash(ctx, out);
            }
            Expr::Print(inner) => {
                out.write_u32(6);
                inner.stable_hash(ctx, out);
            }
            Expr::Return(inner) => {
                out.write_u32(7);
                inner.stable_hash(ctx, out);
            }
            Expr::Panic { source_location } => {
                out.write_u32(8);
                source_location.stable_hash(ctx, out);
            }
            Expr::Call { func, args } => {
                out.write_u32(9);
                func.stable_hash(ctx, out);
                args.stable_hash(ctx, out);
            }
            Expr::Arg(n) => {
                out.write_u32(10);
                n.stable_hash(ctx, out);
            }
            Expr::Sequence(items) => {
                out.write_u32(11);
                items.stable_hash(ctx, out);
            }
        }
    }
}

impl StableHash for MExpr {
    fn stable_hash(&self, ctx: &StableCtx<'_>, out: &mut StableHasher) {
        match self {
            MExpr::Number(value) => {
                out.write_u32(0);
                value.stable_hash(ctx, out);
            }
            MExpr::VarRef(var) => {
                out.write_u32(1);
                var.stable_hash(ctx, out);
            }
            MExpr::BinaryOp { op, left, right } => {
                out.write_u32(2);
                op.stable_hash(ctx, out);
                left.stable_hash(ctx, out);
                right.stable_hash(ctx, out);
            }
            MExpr::Let { var, value } => {
                out.write_u32(3);
                var.stable_hash(ctx, out);
                value.stable_hash(ctx, out);
            }
            MExpr::Set { var, value } => {
                out.write_u32(4);
                var.stable_hash(ctx, out);
                value.stable_hash(ctx, out);
            }
            MExpr::If { cond, then_branch, else_branch } => {
                out.write_u32(5);
                cond.stable_hash(ctx, out);
                then_branch.stable_hash(ctx, out);
                else_branch.stable_hash(ctx, out);
            }
            MExpr::Print(inner) => {
                out.write_u32(6);
                inner.stable_hash(ctx, out);
            }
            MExpr::Return(inner) => {
                out.write_u32(7);
                inner.stable_hash(ctx, out);
            }
            MExpr::Panic { source_location } => {
                out.write_u32(8);
                source_location.stable_hash(ctx, out);
            }
            MExpr::Call { func, args } => {
                out.write_u32(9);
                func.stable_hash(ctx, out);
                args.stable_hash(ctx, out);
            }
            MExpr::Arg(n) => {
                out.write_u32(10);
                n.stable_hash(ctx, out);
            }
            MExpr::Sequence(items) => {
                out.write_u32(11);
                items.stable_hash(ctx, out);
            }
        }
    }
}

/// The fingerprint of a resolved function: what the mono phase actually
/// consumes from resolution — its content key chains to this, giving
/// *function-level* cutoff (editing one function in a file leaves the file's
/// other functions' mono keys unchanged).
impl StableHash for crate::types::FuncData {
    fn stable_hash(&self, ctx: &StableCtx<'_>, out: &mut StableHasher) {
        self.loc.stable_hash(ctx, out);
        out.write_len(self.arity);
        self.ast.stable_hash(ctx, out);
    }
}

/// The direct output of one resolve step (docs/keys-and-invalidation.md: a
/// result fingerprint hashes the direct output, and *only* that). `funcs` is
/// in registration order, which is deterministic given the input.
impl StableHash for crate::store::ResolveAnswer {
    fn stable_hash(&self, ctx: &StableCtx<'_>, out: &mut StableHasher) {
        self.ast.stable_hash(ctx, out);
        self.table.stable_hash(ctx, out);
        self.funcs.stable_hash(ctx, out);
    }
}

// ---- StableHash impls: deterministic errors ---------------------------------
//
// A deterministic error is a terminal answer (invariant 6 of
// docs/keys-and-invalidation.md) and therefore fingerprintable, so a
// dependent whose dep *stably errors* can still derive its content key. The
// payloads are already-rendered `String`s, which are stable by construction.
// (Impls cover all variants for totality, but non-terminal errors — cycles,
// joins, IO — are never fingerprinted: they never become store entries.)

impl StableHash for crate::types::ParseError {
    fn stable_hash(&self, ctx: &StableCtx<'_>, out: &mut StableHasher) {
        use crate::types::ParseError::*;
        match self {
            UnexpectedEof => out.write_u32(0),
            UnexpectedToken(tok) => {
                out.write_u32(1);
                tok.stable_hash(ctx, out);
            }
            InvalidNumber(s) => {
                out.write_u32(2);
                s.stable_hash(ctx, out);
            }
            EmptyExpression => out.write_u32(3),
            IoError(e) => {
                out.write_u32(4);
                e.stable_hash(ctx, out);
            }
        }
    }
}

impl StableHash for crate::types::ResolveError {
    fn stable_hash(&self, ctx: &StableCtx<'_>, out: &mut StableHasher) {
        use crate::types::ResolveError::*;
        match self {
            UndefinedVariable(c, name) => {
                out.write_u32(0);
                c.stable_hash(ctx, out);
                name.stable_hash(ctx, out);
            }
            UndefinedFunction(c, name) => {
                out.write_u32(1);
                c.stable_hash(ctx, out);
                name.stable_hash(ctx, out);
            }
            InvalidImportPath(c, name) => {
                out.write_u32(2);
                c.stable_hash(ctx, out);
                name.stable_hash(ctx, out);
            }
            VariableAlreadyDefined(c, name) => {
                out.write_u32(3);
                c.stable_hash(ctx, out);
                name.stable_hash(ctx, out);
            }
            ArgOutsideFunction(c) => {
                out.write_u32(4);
                c.stable_hash(ctx, out);
            }
            InvalidArgNumber(c, n) => {
                out.write_u32(5);
                c.stable_hash(ctx, out);
                n.stable_hash(ctx, out);
            }
            ImportNotAtTop(c) => {
                out.write_u32(6);
                c.stable_hash(ctx, out);
            }
            FunctionDefNotAfterImports(c) => {
                out.write_u32(7);
                c.stable_hash(ctx, out);
            }
            FunctionAlreadyDefined(c, name) => {
                out.write_u32(8);
                c.stable_hash(ctx, out);
                name.stable_hash(ctx, out);
            }
            FunctionOverload { loc, existing_arity, new_arity } => {
                out.write_u32(9);
                loc.stable_hash(ctx, out);
                out.write_len(*existing_arity);
                out.write_len(*new_arity);
            }
            ArityMismatch { context, func_name, expected, got } => {
                out.write_u32(10);
                context.stable_hash(ctx, out);
                func_name.stable_hash(ctx, out);
                out.write_len(*expected);
                out.write_len(*got);
            }
            ArityGap { context, func_name, max_arg } => {
                out.write_u32(11);
                context.stable_hash(ctx, out);
                func_name.stable_hash(ctx, out);
                out.write_len(*max_arg);
            }
            UnreachableCode { context, source_location } => {
                out.write_u32(12);
                context.stable_hash(ctx, out);
                source_location.stable_hash(ctx, out);
            }
            CyclicDependency { cycle } => {
                out.write_u32(13);
                cycle.stable_hash(ctx, out);
            }
            IoError(path, e) => {
                out.write_u32(14);
                path.stable_hash(ctx, out);
                e.stable_hash(ctx, out);
            }
            ParseError(path, e) => {
                out.write_u32(15);
                path.stable_hash(ctx, out);
                e.stable_hash(ctx, out);
            }
            JoinError(msg) => {
                out.write_u32(16);
                msg.stable_hash(ctx, out);
            }
            // A panic is not an answer (invariant 6): control flow returns it
            // as a non-terminal failure before any fingerprint or cache write
            // can happen, so hashing one is a bug by construction.
            Panicked(_) => unreachable!("panics are non-terminal and never fingerprinted"),
        }
    }
}

impl StableHash for crate::types::TypeError {
    fn stable_hash(&self, ctx: &StableCtx<'_>, out: &mut StableHasher) {
        use crate::types::TypeError::*;
        match self {
            Mismatch { context, expected, found } => {
                out.write_u32(0);
                context.stable_hash(ctx, out);
                expected.stable_hash(ctx, out);
                found.stable_hash(ctx, out);
            }
            TraitNotSatisfied { context, ty, tr } => {
                out.write_u32(1);
                context.stable_hash(ctx, out);
                ty.stable_hash(ctx, out);
                out.write_u32(match tr {
                    crate::types::Trait::Number => 0,
                });
            }
            LiteralOutOfRange { context, value, ty } => {
                out.write_u32(2);
                context.stable_hash(ctx, out);
                value.stable_hash(ctx, out);
                ty.stable_hash(ctx, out);
            }
            FunctionNotResolved { context } => {
                out.write_u32(3);
                context.stable_hash(ctx, out);
            }
            // See ResolveError::Panicked above: non-terminal, never hashed.
            Panicked(_) => unreachable!("panics are non-terminal and never fingerprinted"),
        }
    }
}

impl StableHash for VarInfo {
    fn stable_hash(&self, ctx: &StableCtx<'_>, out: &mut StableHasher) {
        self.name.stable_hash(ctx, out);
        self.scope_id.stable_hash(ctx, out);
    }
}

impl StableHash for SymbolTable {
    fn stable_hash(&self, ctx: &StableCtx<'_>, out: &mut StableHasher) {
        self.vars.stable_hash(ctx, out);
    }
}

impl StableHash for MonoFuncData {
    fn stable_hash(&self, ctx: &StableCtx<'_>, out: &mut StableHasher) {
        self.key.stable_hash(ctx, out);
        out.write_len(self.arity);
        self.ast.stable_hash(ctx, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::MonoId;
    use crate::types::ResolveError;

    fn ctx(interner: &Interner) -> StableCtx<'_> {
        StableCtx { interner }
    }

    /// Parse is the leaf query (read is fused with parse), and per
    /// docs/keys-and-invalidation.md a leaf's content key hashes the external
    /// input itself — the path is the *logical* id, not a key ingredient. So
    /// identical bytes at two paths share one parse key (sound: `PreExpr`
    /// contains no path-derived data), while any byte change makes a new key.
    #[test]
    fn parse_key_depends_on_content_not_path() {
        let key_for = |source: &str| {
            ContentKey::build(QueryKind::Parse, |h| ContentDigest::of(source).stable_hash(&ctx(&Interner::new()), h))
        };
        assert_eq!(key_for("(print 42)"), key_for("(print 42)"));
        // Whitespace is a byte change: leaf keys must differ (early cutoff
        // happens above the leaf, via unchanged *output* fingerprints).
        assert_ne!(key_for("(print 42)"), key_for("(print 42) "));
        assert_ne!(key_for("(print 42)"), key_for("(print 43)"));
    }

    #[test]
    fn schema_version_bump_changes_every_key() {
        let with_schema = |schema: u64| {
            ContentKey::build_with_schema(schema, QueryKind::Parse, |h| h.write_u64(123))
        };
        assert_eq!(with_schema(1), with_schema(1));
        assert_ne!(with_schema(1), with_schema(2), "a schema bump must invalidate all old keys");
    }

    #[test]
    fn query_kinds_are_disjoint_keyspaces() {
        let with_kind = |kind: QueryKind| ContentKey::build(kind, |h| h.write_u64(123));
        assert_ne!(with_kind(QueryKind::Parse), with_kind(QueryKind::Mono));
        assert_ne!(with_kind(QueryKind::Resolve), with_kind(QueryKind::Exec));
    }

    /// The order-shuffle test from docs/deterministic-hashing.md: the same
    /// semantic value must fingerprint identically even when the interner
    /// assigned different indices (here: forced by interning dummies first).
    /// This is the test a raw-`Sym`-index leak would fail.
    #[test]
    fn fingerprints_are_independent_of_interning_order() {
        let build = |interner: &Interner| {
            let fq = FQ::intern(interner, "some/file.telsb", "double");
            let expr = MExpr::Call {
                func: MonoId { func_loc: fq, ty: Ty::I64 },
                args: vec![Box::new(MExpr::Number(Value::I64(21)))],
            };
            Fingerprint::of(&expr, &ctx(interner))
        };

        let plain = Interner::new();
        let shuffled = Interner::new();
        for dummy in ["zebra", "yak", "xenon", "double"] {
            shuffled.intern(dummy);
        }
        assert_eq!(
            build(&plain),
            build(&shuffled),
            "fingerprints must not depend on process-local interner indices"
        );
    }

    /// Length prefixes prevent concatenation ambiguity between adjacent
    /// variable-length fields.
    #[test]
    fn adjacent_strings_do_not_alias() {
        let interner = Interner::new();
        let fp = |a: &str, b: &str| {
            let mut h = StableHasher::new();
            a.stable_hash(&ctx(&interner), &mut h);
            b.stable_hash(&ctx(&interner), &mut h);
            h.finish64()
        };
        assert_ne!(fp("ab", "c"), fp("a", "bc"));
    }

    /// Success and error answers share one fingerprint space (a dependency
    /// slot can hold either), so the `Result` variant tag must keep an `Ok`
    /// from ever aliasing an `Err` over the same payload bytes.
    #[test]
    fn ok_and_err_step_answers_cannot_alias() {
        let interner = Interner::new();
        let c = ctx(&interner);
        assert_ne!(Fingerprint::of_ok(&42u64, &c), Fingerprint::of_err(&42u64, &c));
        // Both are tagged, so neither collides with the untagged ingredient
        // hash of the same value either.
        assert_ne!(Fingerprint::of_ok(&42u64, &c), Fingerprint::of(&42u64, &c));
        assert_ne!(Fingerprint::of_err(&42u64, &c), Fingerprint::of(&42u64, &c));
    }

    /// Different variants with equal payload bytes must not collide (the u32
    /// variant tag is doing its job).
    #[test]
    fn enum_variants_are_distinguished() {
        let interner = Interner::new();
        let c = ctx(&interner);
        let print = MExpr::Print(Box::new(MExpr::Number(Value::I64(1))));
        let ret = MExpr::Return(Box::new(MExpr::Number(Value::I64(1))));
        assert_ne!(Fingerprint::of(&print, &c), Fingerprint::of(&ret, &c));
    }

    /// Golden hashes (docs/deterministic-hashing.md "Testing Determinism"):
    /// checked-in constants pin the exact output of the stable encoding +
    /// xxh3, so an accidental encoding change surfaces as a red test instead
    /// of a silently cold — or, once persistent, mysteriously slow — cache.
    /// The goldens are also the cross-process guarantee: they were computed
    /// in a different process (at implementation time) than any run that
    /// checks them, so a per-process seed could never pass.
    ///
    /// If this fails after an *intentional* encoding/algorithm change: bump
    /// `SCHEMA_VERSION`, then replace the constants with the values from the
    /// assertion output. Do not "fix" it any other way.
    #[test]
    fn golden_hashes_pin_encoding_and_algorithm() {
        let interner = Interner::new();
        let c = ctx(&interner);

        // Leaf input digest (write_str: length prefix + bytes; xxh3-128).
        let digest = ContentDigest::of("(print 42)\n");

        // Full parse content key (schema + kind tag + digest; xxh3-128).
        let key = ContentKey::build(QueryKind::Parse, |h| digest.stable_hash(&c, h));

        // Ok-answer fingerprint over a value with an interned FQ (exercises
        // Sym→string resolution, enum tags, and length prefixes; xxh3-64).
        let fq = FQ::intern(&interner, "examples/fib/main.telsb", "fib");
        let expr = MExpr::Call {
            func: MonoId { func_loc: fq, ty: Ty::I64 },
            args: vec![Box::new(MExpr::Number(Value::I64(21)))],
        };
        let ok_fp = Fingerprint::of_ok(&expr, &c);

        // Err-answer fingerprint (exercises the Err tag and write_len).
        let err = ResolveError::ArityMismatch {
            context: "main".to_string(),
            func_name: "fib".to_string(),
            expected: 1,
            got: 2,
        };
        let err_fp = Fingerprint::of_err(&err, &c);

        assert_eq!(format!("{:032x}", digest.0), "4af01ba2c8396b3e6b01ec9d7a01ef6a");
        assert_eq!(format!("{:032x}", key.0), "80a61e3e09ba3c98a4fa357671cc45ca");
        assert_eq!(format!("{:016x}", ok_fp.0), "ff75d229ef0e880c");
        assert_eq!(format!("{:016x}", err_fp.0), "b2a5105200850dff");
    }
}
