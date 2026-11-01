//! Git tag version sources (`git-tags`, version-source). Reads `ctx.git_tags()`
//! and never runs git itself.
//!
//! A tag is git state, not a release. Only version-like tags become findings.
//! Other tags (`nightly`, `v1.2`) are valid git state and produce nothing,
//! not even a diagnostic.

use repo_intelligence_core::{DetectionOutput, Detector, DetectorCategory, RepoContext};
use repo_intelligence_json::{Confidence, Finding, Provenance};
use serde_json::json;

const DETECTOR: &str = "git-tags";

pub(crate) struct GitTags;

impl Detector for GitTags {
    fn id(&self) -> &'static str {
        DETECTOR
    }

    fn category(&self) -> DetectorCategory {
        DetectorCategory::VersionSource
    }

    fn applies(&self, ctx: &RepoContext) -> bool {
        ctx.git_tags().is_some()
    }

    fn detect(&self, ctx: &RepoContext) -> DetectionOutput {
        let findings = ctx
            .git_tags()
            .unwrap_or(&[])
            .iter()
            .filter_map(|tag| {
                let (prefix, version, confidence) = classify(&tag.name)?;
                Some(Finding {
                    category: DetectorCategory::VersionSource,
                    kind: "git-tag".to_string(),
                    path: None,
                    confidence,
                    data: json!({
                        "name": tag.name,
                        "version": version,
                        "prefix": prefix,
                        "annotated": tag.annotated,
                        "commit": tag.commit,
                    }),
                    provenance: Provenance {
                        detector: DETECTOR.to_string(),
                        sources: Vec::new(),
                    },
                })
            })
            .collect();
        DetectionOutput::new(findings, Vec::new())
    }
}

/// Classifies a tag name. Returns the prefix, the version, and the confidence.
///
/// 1. The whole name, with an optional leading `v` removed, is semver: High,
///    no prefix (`v1.2.3`, `2.0.0-rc.1`).
/// 2. Otherwise, the first separator (`-`, `/`, `@`, scanning left to right)
///    whose remainder is semver: Medium, prefix = text before it
///    (`repo-core-v0.1.0`, `core/v1.0.0`).
/// 3. Otherwise: not version-like.
fn classify(name: &str) -> Option<(Option<String>, String, Confidence)> {
    if let Some(version) = semver_after_v(name) {
        return Some((None, version.to_string(), Confidence::High));
    }
    for (i, c) in name.char_indices() {
        if i == 0 || !matches!(c, '-' | '/' | '@') {
            continue;
        }
        if let Some(version) = semver_after_v(&name[i + c.len_utf8()..]) {
            return Some((
                Some(name[..i].to_string()),
                version.to_string(),
                Confidence::Medium,
            ));
        }
    }
    None
}

/// `s` with an optional leading `v` removed, if the result is valid semver.
fn semver_after_v(s: &str) -> Option<&str> {
    let version = s.strip_prefix('v').unwrap_or(s);
    semver::Version::parse(version).ok().map(|_| version)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (prefix, version, confidence) as owned values, for comparison.
    fn kind(name: &str) -> Option<(Option<String>, String, Confidence)> {
        classify(name)
    }

    fn some(
        prefix: Option<&str>,
        version: &str,
        c: Confidence,
    ) -> Option<(Option<String>, String, Confidence)> {
        Some((prefix.map(str::to_string), version.to_string(), c))
    }

    #[test]
    fn classification_table() {
        use Confidence::{High, Medium};
        assert_eq!(kind("v1.2.3"), some(None, "1.2.3", High));
        assert_eq!(kind("1.2.3"), some(None, "1.2.3", High));
        assert_eq!(kind("v2.0.0-rc.1"), some(None, "2.0.0-rc.1", High));
        assert_eq!(kind("1.2.3-1.2.3"), some(None, "1.2.3-1.2.3", High));
        assert_eq!(
            kind("repo-intelligence-core-v0.1.0"),
            some(Some("repo-intelligence-core"), "0.1.0", Medium)
        );
        assert_eq!(kind("core/v1.0.0"), some(Some("core"), "1.0.0", Medium));
        assert_eq!(kind("pkg@1.0.0"), some(Some("pkg"), "1.0.0", Medium));
        assert_eq!(kind("alpha-v2.1.0"), some(Some("alpha"), "2.1.0", Medium));
        assert_eq!(kind("nightly"), None);
        assert_eq!(kind("before-refactor"), None);
        assert_eq!(kind("v1.2"), None);
        assert_eq!(kind("v01.2.3"), None);
        assert_eq!(kind("-1.0.0"), None);
    }
}
