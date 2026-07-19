/// Stability test: serialize the same Report value N times and assert byte-identical output.
/// This proves serialization is deterministic and HashMap-free structures maintain stable ordering.

#[test]
fn report_serialization_is_byte_stable() {
    let example_json = include_str!("fixtures/example_report.json");
    let report: repo_intelligence_json::Report =
        serde_json::from_str(example_json).expect("deserialize example");

    // Serialize N times and collect the bytes
    let serializations: Vec<String> = (0..5)
        .map(|_| serde_json::to_string(&report).expect("serialize"))
        .collect();

    // All serializations must be byte-identical
    for i in 1..serializations.len() {
        assert_eq!(
            serializations[0], serializations[i],
            "serialization #{} differs from #0",
            i
        );
    }
}

#[test]
fn pretty_serialization_is_byte_stable() {
    let example_json = include_str!("fixtures/example_report.json");
    let report: repo_intelligence_json::Report =
        serde_json::from_str(example_json).expect("deserialize example");

    // Serialize pretty-printed N times and collect the bytes
    let serializations: Vec<String> = (0..5)
        .map(|_| serde_json::to_string_pretty(&report).expect("serialize pretty"))
        .collect();

    // All serializations must be byte-identical
    for i in 1..serializations.len() {
        assert_eq!(
            serializations[0], serializations[i],
            "pretty serialization #{} differs from #0",
            i
        );
    }
}
