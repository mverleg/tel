//! Step tracing for debugging the query engine, behind the `step-trace`
//! Cargo feature.
//!
//! When enabled, every step (parse / resolve / mono / exec) appends one JSON
//! line to a trace file — `$TEL_SANDBOX_TRACE_FILE`, or
//! `tel-sandbox-trace.jsonl` in the working directory — recording:
//!
//! - `kind` / `step` — the query kind and the debug repr of the *logical id*
//!   (file path, fully-qualified function name). Only this first-tier key is
//!   rendered in full; content keys and results are too large, so they appear
//!   as hashes only.
//! - `key` — the content key's hash bits, hex. `null` for uncached steps (exec)
//!   and for steps that failed non-terminally before a key was derived.
//! - `cache` — `"hit"` / `"miss"` / `null` (no cache lookup happened).
//! - `age_us` — on a hit: microseconds since this trace first saw the entry
//!   inserted (the tracer stores an insertion instant per key). `null` on a
//!   miss, or when the entry predates the trace (impossible while the content
//!   store is in-memory only).
//! - `fp` — the answer's result-fingerprint hash, hex.
//! - `dur_us` / `t_us` — the step's duration, and its start relative to trace
//!   start, both in microseconds.
//!
//! A `trace_start` line (pid, wall-clock time) is written when the
//! [`crate::Compiler`]'s `Global` is created; the file is opened in append
//! mode, so successive compilers interleave rather than truncate.
//!
//! With the feature *off* (the default), [`StepTrace`] and [`StepSpan`] are
//! zero-sized types whose methods are empty and `#[inline(always)]`, and the
//! closures call sites pass (logical-id formatting, fingerprint lookups) are
//! never invoked — the instrumentation compiles away to nothing.

// The span type is not re-exported: spans are obtained exclusively via
// `StepTrace::span`, so call sites never need to name it.
#[cfg(feature = "step-trace")]
pub use enabled::{ComputeFlag, StepTrace};
#[cfg(not(feature = "step-trace"))]
pub use disabled::{ComputeFlag, StepTrace};

#[cfg(feature = "step-trace")]
mod enabled {
    use crate::keys::{ContentKey, Fingerprint};
    use dashmap::DashMap;
    use serde_json::json;
    use std::fs::{File, OpenOptions};
    use std::io::{BufWriter, Write};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Mutex;
    use std::time::{Instant, SystemTime, UNIX_EPOCH};

    /// The step tracer: owns the trace file and the per-key insertion
    /// instants that make cache-entry ages computable on later hits.
    pub struct StepTrace {
        /// `None` when the trace file could not be opened (warned once on
        /// stderr; the compiler still runs, just untraced).
        out: Option<Mutex<BufWriter<File>>>,
        /// When each content key was first inserted, by raw hash bits — the
        /// stored birth time that `age_us` is measured against.
        inserts: DashMap<u64, Instant>,
        seq: AtomicU64,
        epoch: Instant,
    }

    impl StepTrace {
        pub fn new() -> StepTrace {
            let path = std::env::var("TEL_SANDBOX_TRACE_FILE")
                .unwrap_or_else(|_| "tel-sandbox-trace.jsonl".to_string());
            let out = match OpenOptions::new().create(true).append(true).open(&path) {
                Ok(file) => Some(Mutex::new(BufWriter::new(file))),
                Err(e) => {
                    eprintln!("step-trace: cannot open trace file {}: {} — tracing disabled", path, e);
                    None
                }
            };
            let trace = StepTrace {
                out,
                inserts: DashMap::new(),
                seq: AtomicU64::new(0),
                epoch: Instant::now(),
            };
            let unix_ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            trace.emit(json!({ "event": "trace_start", "pid": std::process::id(), "unix_ms": unix_ms }));
            trace
        }

        /// Open a span for one step. `logical` renders the first-tier key
        /// (filename / fully-qualified name); the record is written when the
        /// span drops, so every exit path of a step — including error
        /// returns — emits exactly one line.
        pub fn span(&self, kind: &'static str, logical: impl FnOnce() -> String) -> StepSpan<'_> {
            StepSpan {
                trace: self,
                kind,
                logical: logical(),
                start: Instant::now(),
                key: None,
                hit: None,
                age_us: None,
                fp: None,
            }
        }

        fn emit(&self, line: serde_json::Value) {
            let Some(out) = &self.out else { return };
            let mut w = out.lock().unwrap();
            // Flushed per line so a crash or panic loses nothing; this is a
            // debug feature, throughput is not the point.
            let _ = writeln!(w, "{}", line);
            let _ = w.flush();
        }
    }

    /// One step in flight. Mark the cache outcome and fingerprint as they
    /// become known; the JSONL record is emitted on drop.
    pub struct StepSpan<'a> {
        trace: &'a StepTrace,
        kind: &'static str,
        logical: String,
        start: Instant,
        key: Option<u64>,
        hit: Option<bool>,
        age_us: Option<u64>,
        fp: Option<u64>,
    }

    impl StepSpan<'_> {
        /// The step was served from the content store under `key`; the age is
        /// measured against the key's stored insertion instant.
        pub fn cache_hit(&mut self, key: ContentKey) {
            self.key = Some(key.raw());
            self.hit = Some(true);
            self.age_us = self
                .trace
                .inserts
                .get(&key.raw())
                .map(|at| at.elapsed().as_micros() as u64);
        }

        /// The step computed its answer and stored it under `key`; the
        /// insertion instant is recorded now (first write wins, matching the
        /// content store's append-only semantics).
        pub fn cache_miss(&mut self, key: ContentKey) {
            self.key = Some(key.raw());
            self.hit = Some(false);
            self.trace.inserts.entry(key.raw()).or_insert_with(Instant::now);
        }

        /// Hit/miss decided by a closure — for sites (the async parse cache)
        /// where the outcome is only observable via a side channel. The
        /// closure runs only when tracing is compiled in.
        pub fn record_cache(&mut self, key: ContentKey, hit: impl FnOnce() -> bool) {
            if hit() {
                self.cache_hit(key);
            } else {
                self.cache_miss(key);
            }
        }

        /// Record the answer's result fingerprint. Takes a closure so call
        /// sites can do the lookup lazily — it never runs in production.
        pub fn set_fingerprint(&mut self, fp: impl FnOnce() -> Option<Fingerprint>) {
            self.fp = fp().map(Fingerprint::raw);
        }
    }

    /// A hit/miss probe for caches that only reveal a miss by running the
    /// init closure (the single-flight async parse cache): the closure calls
    /// [`ComputeFlag::set`], and [`StepSpan::record_cache`] reads it back.
    /// Atomic because the closure crosses an await in a `Send` future.
    pub struct ComputeFlag(std::sync::atomic::AtomicBool);

    impl ComputeFlag {
        pub fn new() -> ComputeFlag {
            ComputeFlag(std::sync::atomic::AtomicBool::new(false))
        }

        pub fn set(&self) {
            self.0.store(true, Ordering::Relaxed);
        }

        pub fn is_set(&self) -> bool {
            self.0.load(Ordering::Relaxed)
        }
    }

    impl Drop for StepSpan<'_> {
        fn drop(&mut self) {
            let record = json!({
                "event": "step",
                "seq": self.trace.seq.fetch_add(1, Ordering::Relaxed),
                "kind": self.kind,
                "step": self.logical,
                "key": self.key.map(|k| format!("{:016x}", k)),
                "cache": self.hit.map(|h| if h { "hit" } else { "miss" }),
                "age_us": self.age_us,
                "fp": self.fp.map(|f| format!("{:016x}", f)),
                "t_us": self.start.duration_since(self.trace.epoch).as_micros() as u64,
                "dur_us": self.start.elapsed().as_micros() as u64,
            });
            self.trace.emit(record);
        }
    }
}

#[cfg(not(feature = "step-trace"))]
mod disabled {
    use crate::keys::{ContentKey, Fingerprint};

    /// No-op tracer: zero-sized, every method empty and `#[inline(always)]`,
    /// so the instrumentation in `context.rs` costs nothing in production.
    /// The closures passed for logical-id formatting and fingerprint lookups
    /// are never called.
    pub struct StepTrace;

    impl StepTrace {
        #[inline(always)]
        pub fn new() -> StepTrace {
            StepTrace
        }

        #[inline(always)]
        pub fn span(&self, _kind: &'static str, _logical: impl FnOnce() -> String) -> StepSpan {
            StepSpan
        }
    }

    /// No-op span, see [`StepTrace`].
    pub struct StepSpan;

    impl StepSpan {
        #[inline(always)]
        pub fn cache_hit(&mut self, _key: ContentKey) {}

        #[inline(always)]
        pub fn cache_miss(&mut self, _key: ContentKey) {}

        #[inline(always)]
        pub fn record_cache(&mut self, _key: ContentKey, _hit: impl FnOnce() -> bool) {}

        #[inline(always)]
        pub fn set_fingerprint(&mut self, _fp: impl FnOnce() -> Option<Fingerprint>) {}
    }

    /// No-op probe, see [`StepTrace`].
    pub struct ComputeFlag;

    impl ComputeFlag {
        #[inline(always)]
        pub fn new() -> ComputeFlag {
            ComputeFlag
        }

        #[inline(always)]
        pub fn set(&self) {}

        #[inline(always)]
        pub fn is_set(&self) -> bool {
            false
        }
    }
}
