/// Round-trip test: serialize → deserialize → assert_eq on the example report.
/// Verifies that serialization preserves all data and structure fidelity.

#[test]
fn example_report_round_trip() {
    let example_json = include_str!("fixtures/example_report.json");

    // Deserialize from JSON
    let original: repo_intelligence_json::Report =
        serde_json::from_str(example_json).expect("deserialize example");

    // Serialize back to JSON
    let serialized = serde_json::to_string(&original).expect("serialize");

    // Deserialize the re-serialized JSON
    let round_tripped: repo_intelligence_json::Report =
        serde_json::from_str(&serialized).expect("deserialize round-tripped");

    // Assert equality: the deserialized Report must be identical
    assert_eq!(original, round_tripped);
}
