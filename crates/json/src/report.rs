//! Report envelope: the top-level contract type carrying findings and diagnostics.

use serde::{Deserialize, Serialize};

use crate::{Diagnostic, Finding};

/// The report envelope: the complete analysis output carrying facts and diagnostics.
///
/// Five fields, exactly in this order (order is part of the byte-stable contract):
/// 1. `schema_version` — the contract version (serialized from SCHEMA_VERSION constant)
/// 2. `tool_version` — the CLI/engine release (injected by the CLI at runtime)
/// 3. `analyzed_at` — RFC 3339 timestamp (injected by the CLI at runtime, excluded from determinism tests)
/// 4. `findings` — the facts discovered during analysis
/// 5. `diagnostics` — records of what analysis couldn't determine
///
/// Both `findings` and `diagnostics` are always present as arrays (never null, never omitted),
/// expressing the contract's invariant shape: an empty-detector run still produces a well-formed report.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Report {
    /// Schema version (e.g. `"0.1.0"`), serialized from `SCHEMA_VERSION`.
    /// Used by consumers to enforce compatibility bounds.
    pub schema_version: String,

    /// Tool version: the CLI/engine release that produced this report.
    /// **Not populated by the `json` crate.** Injected by the CLI at runtime.
    /// Many tool versions may share one schema version.
    pub tool_version: String,

    /// RFC 3339 timestamp when analysis completed.
    /// **Not populated by the `json` crate.** Injected by the CLI at runtime.
    /// String (not a chrono type) to keep the `json` crate free of time dependencies
    /// and to keep the contract format-explicit.
    /// Excluded from determinism tests (analysis output may vary by timestamp).
    pub analyzed_at: String,

    /// The facts discovered during analysis.
    /// Empty array is valid — an empty-detector run still produces a well-formed report.
    pub findings: Vec<Finding>,

    /// Records of what the analysis couldn't determine (errors, warnings).
    /// Empty array is valid. Never null or omitted.
    pub diagnostics: Vec<Diagnostic>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_report_serialization() {
        let report = Report {
            schema_version: "0.1.0".to_string(),
            tool_version: "0.0.0".to_string(),
            analyzed_at: "2026-07-19T00:00:00Z".to_string(),
            findings: vec![],
            diagnostics: vec![],
        };

        let json = serde_json::to_value(&report).unwrap();

        // Verify all five fields are present and in order
        assert!(json.get("schema_version").is_some());
        assert!(json.get("tool_version").is_some());
        assert!(json.get("analyzed_at").is_some());
        assert!(json.get("findings").is_some());
        assert!(json.get("diagnostics").is_some());

        // Verify findings and diagnostics are always-present empty arrays, never null
        assert_eq!(json.get("findings"), Some(&serde_json::Value::Array(vec![])));
        assert_eq!(json.get("diagnostics"), Some(&serde_json::Value::Array(vec![])));
    }

    #[test]
    fn test_report_field_order() {
        let report = Report {
            schema_version: "0.1.0".to_string(),
            tool_version: "0.0.0".to_string(),
            analyzed_at: "2026-07-19T00:00:00Z".to_string(),
            findings: vec![],
            diagnostics: vec![],
        };

        let json_value = serde_json::to_value(&report).unwrap();
        let json_obj = json_value.as_object().unwrap();
        let keys: Vec<&str> = json_obj.keys().map(|s| s.as_str()).collect();

        // Verify exact field order (part of the contract)
        assert_eq!(
            keys,
            vec![
                "schema_version",
                "tool_version",
                "analyzed_at",
                "findings",
                "diagnostics"
            ]
        );
    }
}
