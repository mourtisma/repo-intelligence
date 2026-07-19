//! Example report validation test.
//! Verifies that a hand-authored example report deserializes into Report
//! and that the emitted JSON Schema correctly constrains the report structure.

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

    // Verify each finding has required structure (data is always an object)
    for finding in &report.findings {
        assert!(
            finding.data.is_object(),
            "Finding.data must be a JSON object, not scalar/array"
        );
    }
}

#[test]
fn schema_artifact_exists_and_is_valid() {
    let schema_json = include_str!("../../../schema/report.schema.json");

    // Schema must parse as valid JSON
    let schema: serde_json::Value =
        serde_json::from_str(schema_json).expect("schema parses as valid JSON");

    // Schema must be an object with required top-level keys
    assert!(schema.is_object(), "JSON Schema artifact must be an object");
    assert!(
        schema.get("$schema").is_some(),
        "Schema must declare $schema version"
    );
    assert!(schema.get("title").is_some(), "Schema must have a title");
    assert!(
        schema.get("properties").is_some(),
        "Schema must define properties"
    );
}

#[test]
fn data_field_constrained_to_object_in_schema() {
    let schema_json = include_str!("../../../schema/report.schema.json");
    let schema: serde_json::Value =
        serde_json::from_str(schema_json).expect("schema parses as JSON");

    // Navigate to Finding.data in the schema
    let finding_data_schema = schema
        .get("definitions")
        .and_then(|d| d.get("Finding"))
        .and_then(|f| f.get("properties"))
        .and_then(|p| p.get("data"))
        .expect("Finding.data schema must exist in definitions");

    // Assert that data is constrained to type: object
    assert_eq!(
        finding_data_schema.get("type"),
        Some(&serde_json::Value::String("object".to_string())),
        "Finding.data must be constrained to type: object (phase-1-implementation-plan §5.3)"
    );
}
