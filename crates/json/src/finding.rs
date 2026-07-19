//! Finding model: a structured fact discovered during analysis, with provenance and confidence.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use schemars::JsonSchema;

use crate::Confidence;

/// The six stable detector categories from the architecture.
/// Serialized as kebab-case strings for human-legible, language-neutral JSON.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum DetectorCategory {
    /// Repository structure and metadata.
    Structure,
    /// Programming languages present.
    Language,
    /// Package managers, dependencies, ecosystems.
    Ecosystem,
    /// Version sources: tags, branches, manifest versions.
    #[serde(rename = "version-source")]
    VersionSource,
    /// CI/CD systems and configurations.
    Ci,
    /// Release tooling and automation.
    #[serde(rename = "release-tooling")]
    ReleaseTooling,
}

/// Provenance: which detector produced a finding and from which sources.
/// Makes facts traceable to bytes on disk.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Provenance {
    /// The detector's stable id.
    pub detector: String,
    /// Repo-relative, slash-normalized paths the fact was derived from.
    /// May be empty for synthesized facts (Phase 2+).
    pub sources: Vec<String>,
}

/// A Finding: one structured fact with provenance and confidence.
/// The `data` field is an open JSON object for per-kind payloads,
/// allowing detectors to grow schema additively without editing the contract.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Finding {
    /// Which category of question this finding answers.
    pub category: DetectorCategory,
    /// The specific fact type within the category (e.g. "cargo-toml").
    /// A string, not an enum — the load-bearing extensibility choice.
    pub kind: String,
    /// Repo-relative path this fact is anchored to (forward-slash normalized).
    /// `None` for genuinely repo-global facts.
    pub path: Option<String>,
    /// Confidence in this finding.
    pub confidence: Confidence,
    /// The typed-per-kind payload as an open JSON object.
    /// v0 enforces it is always a JSON object (never a bare scalar or array).
    pub data: Value,
    /// Which detector produced this, and from which files.
    pub provenance: Provenance,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detector_category_equality() {
        assert_eq!(DetectorCategory::Structure, DetectorCategory::Structure);
        assert_ne!(DetectorCategory::Structure, DetectorCategory::Ecosystem);
    }

    #[test]
    fn test_finding_round_trip_with_data_order() {
        let finding = Finding {
            category: DetectorCategory::Ecosystem,
            kind: "cargo-toml".to_string(),
            path: Some("Cargo.toml".to_string()),
            confidence: Confidence::High,
            data: serde_json::json!({
                "version": "1.4.0",
                "name": "example"
            }),
            provenance: Provenance {
                detector: "cargo-detector".to_string(),
                sources: vec!["Cargo.toml".to_string()],
            },
        };

        let json_str = serde_json::to_string(&finding).unwrap();
        let deserialized: Finding = serde_json::from_str(&json_str).unwrap();
        assert_eq!(finding, deserialized);

        // Verify data key order is preserved (serde_json with preserve_order)
        let json_obj = serde_json::to_value(&finding).unwrap();
        let keys: Vec<&str> = json_obj
            .as_object()
            .unwrap()
            .keys()
            .map(|s| s.as_str())
            .collect();
        // Keys should be in declaration order: category, kind, path, confidence, data, provenance
        assert_eq!(keys[0], "category");
        assert_eq!(keys[1], "kind");
        assert_eq!(keys[2], "path");
        assert_eq!(keys[3], "confidence");
        assert_eq!(keys[4], "data");
        assert_eq!(keys[5], "provenance");
    }
}
