//! JSON contract crate: defines the stable, versioned report schema shared
//! between the analysis engine and its consumers.
//!
//! # Versioning
//!
//! This crate uses semantic versioning (major.minor.patch). The versioning rule:
//! - **Additive changes bump minor:** new optional fields, new `DetectorCategory`
//!   variants, new finding kinds, new diagnostic severities. Consumers reading
//!   schema version v0.N must tolerate future v0.M where M >= N.
//! - **Breaking changes bump major:** renaming/removing fields, changing field
//!   meaning or type, making optional fields required, removing enum variants.
//! - **Tool version is separate:** `tool_version` in reports is the CLI/engine
//!   release, injected at runtime. Many tool versions may share one schema version.
//!
//! During `0.x` (pre-1.0), we adopt the discipline above rather than relying on
//! semver's technical permission for breaking changes in minors. This ensures
//! external consumers (Skills, third parties) can safely pin to schema v0.x.
//!
//! # The Report envelope
//!
//! The top-level `Report` type carries analysis findings and diagnostics.
//! **Important:** the `json` crate defines the shape but does NOT populate:
//! - `tool_version`: injected by the CLI at runtime (the engine/CLI release version)
//! - `analyzed_at`: injected by the CLI at runtime (RFC 3339 timestamp of analysis)
//!
//! The `json` crate only serializes these fields as strings; runtime injection
//! is the CLI's responsibility to ensure reproducibility and accuracy.

pub const SCHEMA_VERSION: &str = "0.1.0";

mod confidence;
pub use confidence::Confidence;

mod diagnostic;
pub use diagnostic::{Diagnostic, Severity};

mod finding;
pub use finding::{DetectorCategory, Finding, Provenance};

mod report;
pub use report::Report;

#[cfg(test)]
mod schema_generation {
    use schemars::schema_for;

    const SCHEMA_PATH: &str = "../../schema/report.schema.json";

    fn generated_schema_json() -> String {
        let schema = schema_for!(super::Report);
        serde_json::to_string_pretty(&schema).expect("schema serialization")
    }

    /// Asserts the committed schema matches what the types currently generate.
    /// Does not write anything — a stale or missing file is a test failure.
    #[test]
    fn test_schema_generation_and_stability() {
        let schema_json = generated_schema_json();

        let committed = std::fs::read_to_string(SCHEMA_PATH).unwrap_or_else(|_| {
            panic!(
                "Schema file did not exist at {}. Regenerate with: cargo test -p repo-intelligence-json regenerate_schema -- --ignored",
                SCHEMA_PATH
            )
        });

        assert_eq!(
            schema_json, committed,
            "Schema drifted from types. Regenerate with: cargo test -p repo-intelligence-json regenerate_schema -- --ignored"
        );
    }

    /// Not run by default (`cargo test`). Explicitly writes the regenerated
    /// schema to disk for review before committing:
    /// `cargo test -p repo-intelligence-json regenerate_schema -- --ignored`
    #[test]
    #[ignore]
    fn regenerate_schema() {
        std::fs::write(SCHEMA_PATH, generated_schema_json()).expect("write schema file");
    }
}
