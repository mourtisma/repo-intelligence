//! The run pipeline: gate detectors with `applies()`, run `detect()`, synthesize,
//! order, and assemble a `Report`.
//!
//! # Result ordering (part of the observable contract)
//!
//! `findings` are sorted by, in order:
//! 1. `path` ascending, with repo-global (`None`) findings last;
//! 2. `category`, by declaration order of `DetectorCategory`;
//! 3. `kind`;
//! 4. `provenance.detector`;
//! 5. `confidence`, ascending (`Low` < `Medium` < `High`);
//! 6. `provenance.sources`, lexicographically;
//! 7. `data`, compared by its compact JSON serialization.
//!
//! `diagnostics` are sorted by `path` (`None` last), then `severity`
//! (`Error` before `Warning`), then `detector`, then `message`.
//!
//! The key is total: two entries that compare equal serialize identically, so the
//! output cannot depend on detector registration or emission order.
//!
//! # Detector identity
//!
//! A finding is kept only if its `provenance.detector` equals the emitting
//! detector's `id()` and its `category` equals the detector's `category()`.
//! Otherwise it is dropped and replaced by an `error` diagnostic. A detector's
//! diagnostics are always attributed to its `id()`.
//!
//! # Panics
//!
//! A panicking detector aborts the run. Detectors must report problems as
//! diagnostics; panic isolation is deferred until a real detector needs it.
//!
//! # Engine-set envelope fields
//!
//! `tool_version` and `analyzed_at` are set to empty strings. The CLI injects them
//! at runtime (Phase 3), so the engine never introduces time-like or
//! environment-dependent values.

use std::cmp::Ordering;

use repo_intelligence_json::{
    DetectorCategory, Diagnostic, Finding, Report, Severity, SCHEMA_VERSION,
};

use crate::context::RepoContext;
use crate::detector::DetectorRegistry;

/// Runs every applicable detector in `registry` over `ctx` and assembles a `Report`.
pub fn run(ctx: &RepoContext, registry: &DetectorRegistry) -> Report {
    let mut findings = Vec::new();
    let mut diagnostics: Vec<Diagnostic> = ctx.diagnostics().to_vec();

    for detector in registry.iter() {
        if !detector.applies(ctx) {
            continue;
        }
        let output = detector.detect(ctx);
        let (id, category) = (detector.id(), detector.category());
        for finding in output.findings {
            if finding.provenance.detector == id && finding.category == category {
                findings.push(finding);
            } else {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    message: format!(
                        "finding rejected: kind `{}` does not match detector id/category",
                        finding.kind
                    ),
                    detector: id.to_string(),
                    path: finding.path,
                });
            }
        }
        diagnostics.extend(output.diagnostics.into_iter().map(|mut d| {
            d.detector = id.to_string();
            d
        }));
    }

    let (mut findings, mut diagnostics) = synthesize(findings, diagnostics);
    findings.sort_by(compare_findings);
    diagnostics.sort_by(compare_diagnostics);

    Report {
        schema_version: SCHEMA_VERSION.to_string(),
        tool_version: String::new(),
        analyzed_at: String::new(),
        findings,
        diagnostics,
    }
}

/// Synthesis seam: the only place cross-detector correlation may happen.
/// Pure. Identity in Phase 2, because no detectors exist yet to correlate.
fn synthesize(
    findings: Vec<Finding>,
    diagnostics: Vec<Diagnostic>,
) -> (Vec<Finding>, Vec<Diagnostic>) {
    (findings, diagnostics)
}

fn compare_findings(a: &Finding, b: &Finding) -> Ordering {
    a.path
        .is_none()
        .cmp(&b.path.is_none())
        .then_with(|| a.path.cmp(&b.path))
        .then_with(|| category_rank(a.category).cmp(&category_rank(b.category)))
        .then_with(|| a.kind.cmp(&b.kind))
        .then_with(|| a.provenance.detector.cmp(&b.provenance.detector))
        .then_with(|| a.confidence.cmp(&b.confidence))
        .then_with(|| a.provenance.sources.cmp(&b.provenance.sources))
        .then_with(|| data_key(&a.data).cmp(&data_key(&b.data)))
}

fn compare_diagnostics(a: &Diagnostic, b: &Diagnostic) -> Ordering {
    a.path
        .is_none()
        .cmp(&b.path.is_none())
        .then_with(|| a.path.cmp(&b.path))
        .then_with(|| severity_rank(a.severity).cmp(&severity_rank(b.severity)))
        .then_with(|| a.detector.cmp(&b.detector))
        .then_with(|| a.message.cmp(&b.message))
}

/// Declaration order of `DetectorCategory`. `json` does not derive `Ord`, and the
/// contract crate stays unchanged in Phase 2, so the rank is spelled out here.
fn category_rank(category: DetectorCategory) -> u8 {
    match category {
        DetectorCategory::Structure => 0,
        DetectorCategory::Language => 1,
        DetectorCategory::Ecosystem => 2,
        DetectorCategory::VersionSource => 3,
        DetectorCategory::Ci => 4,
        DetectorCategory::ReleaseTooling => 5,
    }
}

/// Declaration order of `Severity`.
fn severity_rank(severity: Severity) -> u8 {
    match severity {
        Severity::Error => 0,
        Severity::Warning => 1,
    }
}

fn data_key(data: &serde_json::Value) -> String {
    serde_json::to_string(data).unwrap_or_default()
}
