//! Diagnostic model: records where the analysis couldn't determine a fact,
//! for transparency and debugging.

use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

/// Severity level of a diagnostic message.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Severity {
    /// An error that prevented analysis.
    Error,
    /// A warning that analysis continued despite an issue.
    Warning,
}

/// A diagnostic: a record of something the analysis couldn't determine.
/// Diagnostics capture the "why couldn't you find that?" context.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Diagnostic {
    /// Severity of the diagnostic.
    pub severity: Severity,
    /// Human-readable message explaining what couldn't be determined.
    pub message: String,
    /// Which detector produced this diagnostic (stable id).
    pub detector: String,
    /// Optional path this diagnostic is anchored to (repo-relative, forward-slash normalized).
    /// Always emitted (as `null` if absent) to maintain invariant report shape.
    pub path: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_severity_equality() {
        assert_eq!(Severity::Error, Severity::Error);
        assert_ne!(Severity::Error, Severity::Warning);
    }

    #[test]
    fn test_diagnostic_serialization_shape() {
        let diag = Diagnostic {
            severity: Severity::Error,
            message: "file not found".to_string(),
            detector: "cargo-detector".to_string(),
            path: Some("Cargo.toml".to_string()),
        };
        let json = serde_json::to_value(&diag).unwrap();
        assert!(json.get("severity").is_some());
        assert!(json.get("message").is_some());
        assert!(json.get("detector").is_some());
        assert!(json.get("path").is_some());
    }

    #[test]
    fn test_diagnostic_with_null_path() {
        let diag = Diagnostic {
            severity: Severity::Warning,
            message: "analysis incomplete".to_string(),
            detector: "scanner".to_string(),
            path: None,
        };
        let json = serde_json::to_value(&diag).unwrap();
        assert_eq!(json.get("path"), Some(&serde_json::Value::Null));
    }
}
