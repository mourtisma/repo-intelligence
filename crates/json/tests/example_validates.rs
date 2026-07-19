/// Example report validation test.
/// Verifies that a hand-authored example report both deserializes into Report
/// and validates against key schema constraints.

#[test]
fn example_report_deserializes() {
    let example_json = include_str!("fixtures/example_report.json");
    let report: repo_intelligence_json::Report =
        serde_json::from_str(example_json).expect("example report deserializes");

    // Verify structure: should have 3 findings and 1 diagnostic
    assert_eq!(report.findings.len(), 3);
    assert_eq!(report.diagnostics.len(), 1);

    // Verify schema version
    assert_eq!(report.schema_version, "0.1.0");
}

#[test]
fn example_report_validates_against_schema() {
    let example_json = include_str!("fixtures/example_report.json");
    let report: repo_intelligence_json::Report =
        serde_json::from_str(example_json).expect("example deserializes");

    // Validate envelope structure (required fields as per schema)
    assert!(!report.schema_version.is_empty());
    assert!(!report.tool_version.is_empty());
    assert!(!report.analyzed_at.is_empty());
    // findings and diagnostics are always Vec (never null)

    // Validate findings structure
    for finding in &report.findings {
        // Each finding must have these required fields
        assert!(!finding.kind.is_empty());
        // path can be None, that's valid
        // confidence, category, data, provenance are all required by type system
    }

    // Validate diagnostics structure
    for diagnostic in &report.diagnostics {
        assert!(!diagnostic.message.is_empty());
        assert!(!diagnostic.detector.is_empty());
        // severity is required by type system
        // path can be None, that's valid
    }

    // Validate specific content from the example
    assert_eq!(report.findings[0].kind, "git-repository");
    assert_eq!(report.findings[1].kind, "cargo-toml");
    assert_eq!(report.findings[2].kind, "rust-detected");
    assert_eq!(report.diagnostics[0].message, "Could not determine CI/CD configuration");
}
