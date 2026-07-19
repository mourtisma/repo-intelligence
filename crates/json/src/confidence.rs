//! Confidence model: a small bounded enum expressing how confident a detector is
//! in a finding, deterministic and stable across versions.

use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

/// Confidence level of a finding. A small closed enum: exactly three levels.
/// Variant order is meaningful: `Low` < `Medium` < `High` (derives `Ord`).
/// Serialized as lowercase strings to ensure determinism and stability.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    /// Low confidence: the finding may be correct but has significant uncertainty.
    Low,
    /// Medium confidence: the finding is likely correct but uncertain.
    Medium,
    /// High confidence: the finding is almost certainly correct.
    High,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_confidence_equality() {
        assert_eq!(Confidence::High, Confidence::High);
        assert_ne!(Confidence::High, Confidence::Medium);
        assert_ne!(Confidence::Medium, Confidence::Low);
    }

    #[test]
    fn test_confidence_ordering() {
        assert!(Confidence::High > Confidence::Medium);
        assert!(Confidence::Medium > Confidence::Low);
        assert!(Confidence::High > Confidence::Low);
    }
}
