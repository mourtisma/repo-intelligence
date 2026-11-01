//! Determinism harness (Phase 2 exit gate): repeated runs are byte-identical, and
//! the output does not depend on detector registration or emission order.

mod common;

use common::{fixture, gitignore_tree, tree};
use repo_intelligence_core::{
    run, scan, DetectionOutput, Detector, DetectorCategory, DetectorRegistry, RepoContext,
};
use repo_intelligence_json::{
    Confidence, Diagnostic, Finding, Provenance, Report, Severity, SCHEMA_VERSION,
};
use serde_json::json;

fn to_bytes(report: &Report) -> String {
    serde_json::to_string(report).unwrap()
}

/// Test-only detector that emits a fixed list of findings and diagnostics.
#[derive(Clone)]
struct Emitter {
    id: &'static str,
    applies: bool,
    findings: Vec<Finding>,
    diagnostics: Vec<Diagnostic>,
}

impl Detector for Emitter {
    fn id(&self) -> &'static str {
        self.id
    }
    fn category(&self) -> DetectorCategory {
        DetectorCategory::Structure
    }
    fn applies(&self, _ctx: &RepoContext) -> bool {
        self.applies
    }
    fn detect(&self, _ctx: &RepoContext) -> DetectionOutput {
        DetectionOutput {
            findings: self.findings.clone(),
            diagnostics: self.diagnostics.clone(),
        }
    }
}

fn finding(path: Option<&str>, kind: &str, detector: &str, data: serde_json::Value) -> Finding {
    Finding {
        category: DetectorCategory::Structure,
        kind: kind.to_string(),
        path: path.map(str::to_string),
        confidence: Confidence::High,
        data,
        provenance: Provenance {
            detector: detector.to_string(),
            sources: path.map(|p| vec![p.to_string()]).unwrap_or_default(),
        },
    }
}

fn diag(path: Option<&str>, detector: &str, message: &str) -> Diagnostic {
    Diagnostic {
        severity: Severity::Warning,
        message: message.to_string(),
        detector: detector.to_string(),
        path: path.map(str::to_string),
    }
}

#[test]
fn empty_run_is_schema_shaped_and_byte_stable_on_each_fixture() {
    let registry = DetectorRegistry::new();
    let gitignore = gitignore_tree();
    let empty = tree(&[]);
    for root in [
        fixture("nested"),
        gitignore.path().into(),
        empty.path().into(),
    ] {
        let first = run(&scan(&root).unwrap(), &registry);

        assert_eq!(first.schema_version, SCHEMA_VERSION);
        assert_eq!(first.tool_version, "");
        assert_eq!(first.analyzed_at, "");
        assert!(first.findings.is_empty());

        let expected = to_bytes(&first);
        for _ in 0..10 {
            let again = run(&scan(&root).unwrap(), &registry);
            assert_eq!(to_bytes(&again), expected, "{} not stable", root.display());
        }
    }
}

#[test]
fn serialized_report_has_contract_field_order() {
    let report = run(&scan(&fixture("nested")).unwrap(), &DetectorRegistry::new());
    let value = serde_json::to_value(&report).unwrap();
    let keys: Vec<&str> = value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
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

#[test]
fn report_fields_are_required_in_committed_schema() {
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../../../schema/report.schema.json")).unwrap();
    let required: Vec<&str> = schema["required"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    for key in [
        "schema_version",
        "tool_version",
        "analyzed_at",
        "findings",
        "diagnostics",
    ] {
        assert!(required.contains(&key), "schema must require {key}");
    }
}

#[test]
fn output_is_independent_of_registration_and_emission_order() {
    let ctx = scan(&fixture("nested")).unwrap();

    let a = Emitter {
        id: "a",
        applies: true,
        findings: vec![
            finding(Some("z.txt"), "k", "a", json!({"n": 2})),
            finding(None, "global", "a", json!({})),
            finding(Some("a.txt"), "k", "a", json!({"n": 1})),
        ],
        diagnostics: vec![diag(Some("z.txt"), "a", "late"), diag(None, "a", "global")],
    };
    let b = Emitter {
        id: "b",
        applies: true,
        findings: vec![
            finding(Some("a.txt"), "k", "b", json!({"n": 1})),
            finding(Some("z.txt"), "k", "b", json!({"n": 2})),
        ],
        diagnostics: vec![diag(Some("a.txt"), "b", "early")],
    };

    let mut forward = DetectorRegistry::new();
    forward.register(a.clone());
    forward.register(b.clone());

    let mut reversed = DetectorRegistry::new();
    reversed.register(b.clone());
    reversed.register(a.clone());

    let report_forward = run(&ctx, &forward);
    let report_reversed = run(&ctx, &reversed);
    assert_eq!(to_bytes(&report_forward), to_bytes(&report_reversed));

    // Path-bearing findings come first, sorted by path; repo-global last.
    let paths: Vec<Option<&str>> = report_forward
        .findings
        .iter()
        .map(|f| f.path.as_deref())
        .collect();
    assert_eq!(
        paths,
        vec![
            Some("a.txt"),
            Some("a.txt"),
            Some("z.txt"),
            Some("z.txt"),
            None
        ]
    );
    assert_eq!(report_forward.diagnostics[0].path.as_deref(), Some("a.txt"));
}

#[test]
fn detector_that_does_not_apply_contributes_nothing() {
    let ctx = scan(&fixture("nested")).unwrap();
    let mut registry = DetectorRegistry::new();
    registry.register(Emitter {
        id: "off",
        applies: false,
        findings: vec![finding(Some("a.txt"), "k", "off", json!({}))],
        diagnostics: vec![diag(None, "off", "should not appear")],
    });
    let report = run(&ctx, &registry);
    assert!(report.findings.is_empty());
    assert!(report.diagnostics.is_empty());
}

#[test]
fn findings_tied_up_to_data_are_ordered_by_data() {
    let ctx = scan(&fixture("nested")).unwrap();
    let one = finding(Some("a.txt"), "k", "a", json!({"n": 1}));
    let two = finding(Some("a.txt"), "k", "a", json!({"n": 2}));
    let emitter = |findings| Emitter {
        id: "a",
        applies: true,
        findings,
        diagnostics: vec![],
    };

    let mut first = DetectorRegistry::new();
    first.register(emitter(vec![two.clone(), one.clone()]));
    let mut second = DetectorRegistry::new();
    second.register(emitter(vec![one.clone(), two.clone()]));

    let report = run(&ctx, &first);
    assert_eq!(report.findings, vec![one, two]);
    assert_eq!(to_bytes(&report), to_bytes(&run(&ctx, &second)));
}

#[test]
fn findings_with_foreign_identity_are_rejected() {
    let ctx = scan(&fixture("nested")).unwrap();
    let mut wrong_category = finding(Some("b.txt"), "k", "a", json!({}));
    wrong_category.category = DetectorCategory::Ci;
    let mut registry = DetectorRegistry::new();
    registry.register(Emitter {
        id: "a",
        applies: true,
        findings: vec![
            finding(Some("a.txt"), "k", "spoofed", json!({})),
            wrong_category,
            finding(Some("c.txt"), "k", "a", json!({})),
        ],
        diagnostics: vec![diag(None, "spoofed", "attributed to a")],
    });

    let report = run(&ctx, &registry);
    let kept: Vec<Option<&str>> = report.findings.iter().map(|f| f.path.as_deref()).collect();
    assert_eq!(kept, vec![Some("c.txt")]);

    let diags: Vec<(Option<&str>, Severity, &str)> = report
        .diagnostics
        .iter()
        .map(|d| (d.path.as_deref(), d.severity, d.detector.as_str()))
        .collect();
    assert_eq!(
        diags,
        vec![
            (Some("a.txt"), Severity::Error, "a"),
            (Some("b.txt"), Severity::Error, "a"),
            (None, Severity::Warning, "a"),
        ]
    );
}

/// Proxy for cross-machine identity: the same tree at two different absolute
/// locations must produce byte-identical reports, diagnostics included.
#[cfg(unix)]
#[test]
fn same_tree_in_different_directories_is_byte_identical() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::PermissionsExt;

    let build = || {
        let dir = gitignore_tree();
        std::os::unix::fs::symlink("keep.txt", dir.path().join("link.txt")).unwrap();
        std::fs::write(dir.path().join(OsStr::from_bytes(b"bad\xff.txt")), "x").unwrap();
        let locked = dir.path().join("locked");
        std::fs::create_dir(&locked).unwrap();
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
        dir
    };
    let (first, second) = (build(), build());

    let report_first = run(&scan(first.path()).unwrap(), &DetectorRegistry::new());
    let report_second = run(&scan(second.path()).unwrap(), &DetectorRegistry::new());
    assert!(report_first.diagnostics.len() >= 2);
    assert_eq!(to_bytes(&report_first), to_bytes(&report_second));

    for dir in [&first, &second] {
        let locked = dir.path().join("locked");
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
}
