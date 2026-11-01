//! Analysis engine crate: walks a repository and drives detectors to produce
//! a report, using the JSON contract defined by `repo-intelligence-json`.
//!
//! # Entry points (Option A from `docs/phase-2-implementation-plan.md` §7)
//!
//! ```text
//! let ctx = repo_intelligence_core::scan(root)?;        // one traversal
//! let report = repo_intelligence_core::run(&ctx, &registry); // detectors → Report
//! ```
//!
//! `core` ships with an empty `DetectorRegistry`. Concrete detectors are injected by
//! the caller (the CLI, from Phase 3), so this crate never depends on `detectors`.
//!
//! # Determinism
//!
//! The same tree produces a byte-identical `Report` across runs. The traversal
//! sorts paths byte-wise, the pipeline applies a documented total order to findings
//! and diagnostics (see `pipeline`), and the engine sets no timestamps.
//! Traversal policies (hidden entries, ignore files, symlinks, non-UTF-8 paths) are
//! described in `context`.

mod context;
mod detector;
mod error;
mod pipeline;

pub use context::{scan, RepoContext};
pub use detector::{DetectionOutput, Detector, DetectorCategory, DetectorRegistry};
pub use error::EngineError;
pub use pipeline::run;
