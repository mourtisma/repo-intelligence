//! Runs the full pipeline with `default_registry()` over the committed Cargo
//! fixtures. Exact bytes are pinned by the golden tests in `cli`.

use std::path::Path;

use repo_intelligence_core::{run, scan};
use repo_intelligence_detectors::default_registry;
use repo_intelligence_json::{Report, Severity};
use serde_json::{json, Value};

fn analyze(name: &str) -> Report {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name);
    run(&scan(&root).unwrap(), &default_registry())
}

/// (kind, path, data) for every finding, in report order.
fn findings(report: &Report) -> Vec<(&str, Option<&str>, &Value)> {
    report
        .findings
        .iter()
        .map(|f| (f.kind.as_str(), f.path.as_deref(), &f.data))
        .collect()
}

/// (severity, path, message) for every diagnostic, in report order.
fn diagnostics(report: &Report) -> Vec<(Severity, Option<&str>, &str)> {
    report
        .diagnostics
        .iter()
        .map(|d| (d.severity, d.path.as_deref(), d.message.as_str()))
        .collect()
}

#[test]
fn single_crate_has_one_package_and_one_version() {
    let report = analyze("rust-single-crate");
    assert_eq!(
        findings(&report),
        vec![
            (
                "cargo-package",
                Some("Cargo.toml"),
                &json!({ "name": "single", "workspace": null })
            ),
            (
                "cargo-toml",
                Some("Cargo.toml"),
                &json!({ "name": "single", "version": "0.1.0", "field": "package.version", "inherited_from": null })
            ),
        ]
    );
    assert!(diagnostics(&report).is_empty());
}

#[test]
fn workspace_members_inherited_and_literal_versions() {
    let report = analyze("rust-workspace");
    assert!(
        diagnostics(&report).is_empty(),
        "{:?}",
        diagnostics(&report)
    );

    let got: Vec<(&str, Option<&str>, Value)> = findings(&report)
        .into_iter()
        .map(|(k, p, d)| (k, p, d.clone()))
        .collect();
    let want: Vec<(&str, Option<&str>, Value)> = vec![
        (
            "cargo-package",
            Some("crates/alpha/Cargo.toml"),
            json!({ "name": "alpha", "workspace": "Cargo.toml" }),
        ),
        (
            "cargo-package",
            Some("crates/beta/Cargo.toml"),
            json!({ "name": "beta", "workspace": "Cargo.toml" }),
        ),
        (
            "cargo-package",
            Some("crates/skip/Cargo.toml"),
            json!({ "name": "skip", "workspace": null }),
        ),
        (
            "cargo-package",
            Some("tools/helper/Cargo.toml"),
            json!({ "name": "helper", "workspace": null }),
        ),
        (
            "cargo-workspace",
            Some("Cargo.toml"),
            json!({
                "members": ["crates/alpha/Cargo.toml", "crates/beta/Cargo.toml"],
                "member_patterns": ["crates/*"],
                "exclude": ["crates/skip"]
            }),
        ),
        (
            "cargo-toml",
            Some("Cargo.toml"),
            json!({ "name": null, "version": "2.1.0", "field": "workspace.package.version", "inherited_from": null }),
        ),
        (
            "cargo-toml",
            Some("crates/alpha/Cargo.toml"),
            json!({ "name": "alpha", "version": "2.1.0", "field": "workspace.package.version", "inherited_from": "Cargo.toml" }),
        ),
        (
            "cargo-toml",
            Some("crates/beta/Cargo.toml"),
            json!({ "name": "beta", "version": "0.3.0", "field": "package.version", "inherited_from": null }),
        ),
        (
            "cargo-toml",
            Some("crates/skip/Cargo.toml"),
            json!({ "name": "skip", "version": "9.9.9", "field": "package.version", "inherited_from": null }),
        ),
        (
            "cargo-toml",
            Some("tools/helper/Cargo.toml"),
            json!({ "name": "helper", "version": "0.0.1", "field": "package.version", "inherited_from": null }),
        ),
    ];
    // Order is the Phase 2 total order: kind is the first key after path, so
    // the expected list is compared as a set of tuples.
    let mut got_sorted = got.clone();
    let mut want_sorted = want.clone();
    got_sorted.sort_by_key(|(k, p, d)| (p.map(str::to_string), k.to_string(), d.to_string()));
    want_sorted.sort_by_key(|(k, p, d)| (p.map(str::to_string), k.to_string(), d.to_string()));
    assert_eq!(got_sorted, want_sorted);
}

#[test]
fn messy_reports_diagnostics_and_never_panics() {
    let report = analyze("messy");
    let got = diagnostics(&report);
    // Printed for review; the golden file pins the exact text.
    for d in &got {
        eprintln!("{d:?}");
    }
    assert!(got.iter().any(|(s, p, m)| *s == Severity::Error
        && *p == Some("crates/broken/Cargo.toml")
        && m.starts_with("invalid TOML at line")));
    assert!(got
        .iter()
        .any(|(_, p, m)| *p == Some("crates/binary/Cargo.toml")
            && *m == "manifest is not valid UTF-8"));
    assert!(got
        .iter()
        .any(|(_, p, m)| *p == Some("crates/badver/Cargo.toml")
            && *m == "version is not valid semver"));
    assert!(got
        .iter()
        .any(|(_, p, m)| *p == Some("crates/orphan/Cargo.toml")
            && *m == "inherited version cannot be resolved"));
    assert!(got
        .iter()
        .any(|(_, p, m)| *p == Some("crates/noname/Cargo.toml") && *m == "package has no name"));
    assert!(got.iter().any(|(_, p, m)| *p == Some("Cargo.toml")
        && *m == "workspace member pattern matches no Cargo.toml: missing/*"));
}
