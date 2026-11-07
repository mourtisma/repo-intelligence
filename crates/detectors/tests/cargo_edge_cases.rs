//! Cargo workspace semantics, pinned case by case (Phase 3 plan §4.1, §6, §9).
//! Each test builds a small tree in a tempdir and checks the exact findings and
//! diagnostics the Cargo detectors produce.

use repo_intelligence_core::{run, scan};
use repo_intelligence_detectors::default_registry;
use repo_intelligence_json::{Confidence, Report, Severity};
use serde_json::{json, Value};

fn analyze(files: &[(&str, &str)]) -> Report {
    let dir = tempfile::tempdir().unwrap();
    for (path, contents) in files {
        let full = dir.path().join(path);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(full, contents).unwrap();
    }
    run(&scan(dir.path()).unwrap(), &default_registry())
}

/// The `data` of the finding with this kind and path.
fn data<'a>(report: &'a Report, kind: &str, path: &str) -> &'a Value {
    &report
        .findings
        .iter()
        .find(|f| f.kind == kind && f.path.as_deref() == Some(path))
        .unwrap_or_else(|| panic!("no {kind} finding at {path}"))
        .data
}

fn confidence(report: &Report, kind: &str, path: &str) -> Confidence {
    report
        .findings
        .iter()
        .find(|f| f.kind == kind && f.path.as_deref() == Some(path))
        .unwrap()
        .confidence
}

fn diagnostics(report: &Report) -> Vec<(Severity, Option<&str>, &str)> {
    report
        .diagnostics
        .iter()
        .map(|d| (d.severity, d.path.as_deref(), d.message.as_str()))
        .collect()
}

#[test]
fn root_package_is_a_member_of_its_own_workspace() {
    let report = analyze(&[
        (
            "Cargo.toml",
            "[package]\nname='root'\nversion='1.0.0'\n[workspace]\nmembers=['m']\n[workspace.package]\nversion='9.0.0'\n",
        ),
        ("m/Cargo.toml", "[package]\nname='m'\nversion.workspace=true\n"),
    ]);
    assert_eq!(
        data(&report, "cargo-workspace", "Cargo.toml")["members"],
        json!(["Cargo.toml", "m/Cargo.toml"])
    );
    assert_eq!(
        data(&report, "cargo-package", "Cargo.toml"),
        &json!({ "name": "root", "workspace": "Cargo.toml" })
    );
    // The root has two version sources: its own package and the workspace default.
    let root_versions: Vec<&Value> = report
        .findings
        .iter()
        .filter(|f| f.kind == "cargo-toml" && f.path.as_deref() == Some("Cargo.toml"))
        .map(|f| &f.data["field"])
        .collect();
    assert_eq!(
        root_versions,
        vec![
            &json!("package.version"),
            &json!("workspace.package.version")
        ]
    );
    assert_eq!(
        data(&report, "cargo-toml", "m/Cargo.toml"),
        &json!({ "name": "m", "version": "9.0.0", "field": "workspace.package.version", "inherited_from": "Cargo.toml" })
    );
    assert!(diagnostics(&report).is_empty());
}

#[test]
fn nested_workspace_owns_its_own_members() {
    let report = analyze(&[
        (
            "Cargo.toml",
            "[workspace]\nmembers=['crates/*']\n[workspace.package]\nversion='1.0.0'\n",
        ),
        (
            "crates/a/Cargo.toml",
            "[package]\nname='a'\nversion.workspace=true\n",
        ),
        ("crates/inner/Cargo.toml", "[workspace]\nmembers=['x']\n"),
        (
            "crates/inner/x/Cargo.toml",
            "[package]\nname='x'\nversion='2.0.0'\n",
        ),
    ]);
    assert_eq!(
        data(&report, "cargo-workspace", "Cargo.toml")["members"],
        json!(["crates/a/Cargo.toml"])
    );
    assert_eq!(
        data(&report, "cargo-workspace", "crates/inner/Cargo.toml")["members"],
        json!(["crates/inner/x/Cargo.toml"])
    );
    assert_eq!(
        data(&report, "cargo-package", "crates/inner/x/Cargo.toml")["workspace"],
        json!("crates/inner/Cargo.toml")
    );
    assert!(diagnostics(&report).is_empty());
}

#[test]
fn package_workspace_override_is_followed_or_reported() {
    let report = analyze(&[
        (
            "Cargo.toml",
            "[workspace]\nmembers=['crates/*']\n[workspace.package]\nversion='1.0.0'\n",
        ),
        (
            "crates/a/Cargo.toml",
            "[package]\nname='a'\nworkspace='../..'\nversion.workspace=true\n",
        ),
        (
            "other/Cargo.toml",
            "[package]\nname='o'\nworkspace='../nowhere'\nversion='1.0.0'\n",
        ),
    ]);
    assert_eq!(
        data(&report, "cargo-package", "crates/a/Cargo.toml")["workspace"],
        json!("Cargo.toml")
    );
    assert_eq!(
        data(&report, "cargo-toml", "crates/a/Cargo.toml")["version"],
        json!("1.0.0")
    );
    assert_eq!(
        data(&report, "cargo-package", "other/Cargo.toml")["workspace"],
        Value::Null
    );
    assert_eq!(
        diagnostics(&report),
        vec![(
            Severity::Warning,
            Some("other/Cargo.toml"),
            "package.workspace does not name a workspace root"
        )]
    );
}

#[test]
fn wrong_type_and_invalid_inherited_versions_are_reported_once() {
    let report = analyze(&[
        (
            "Cargo.toml",
            "[workspace]\nmembers=['a','b','c']\n[workspace.package]\nversion='1.x'\n",
        ),
        ("a/Cargo.toml", "[package]\nname='a'\nversion=1\n"),
        (
            "b/Cargo.toml",
            "[package]\nname='b'\nversion.workspace=true\n",
        ),
        (
            "c/Cargo.toml",
            "[package]\nname='c'\nversion.workspace=true\n",
        ),
    ]);
    // `a` has no version finding: an integer version is not a version source.
    assert!(!report
        .findings
        .iter()
        .any(|f| f.kind == "cargo-toml" && f.path.as_deref() == Some("a/Cargo.toml")));
    // Inheriting members get a Low finding, but the warning is only at the root.
    assert_eq!(
        confidence(&report, "cargo-toml", "b/Cargo.toml"),
        Confidence::Low
    );
    assert_eq!(
        confidence(&report, "cargo-toml", "c/Cargo.toml"),
        Confidence::Low
    );
    assert_eq!(
        diagnostics(&report),
        vec![
            (
                Severity::Warning,
                Some("Cargo.toml"),
                "version is not valid semver"
            ),
            (
                Severity::Warning,
                Some("a/Cargo.toml"),
                "version has an unsupported type"
            ),
        ]
    );
}

#[test]
fn workspace_below_repo_root_with_explicit_members_and_exclude() {
    let report = analyze(&[
        (
            "sub/Cargo.toml",
            "[workspace]\nmembers=['crates/a','crates/b']\nexclude=['crates/b']\n",
        ),
        (
            "sub/crates/a/Cargo.toml",
            "[package]\nname='a'\nversion='0.1.0'\n",
        ),
        (
            "sub/crates/b/Cargo.toml",
            "[package]\nname='b'\nversion='0.1.0'\n",
        ),
    ]);
    assert_eq!(
        data(&report, "cargo-workspace", "sub/Cargo.toml")["members"],
        json!(["sub/crates/a/Cargo.toml"])
    );
    assert_eq!(
        data(&report, "cargo-package", "sub/crates/b/Cargo.toml")["workspace"],
        Value::Null
    );
    assert!(diagnostics(&report).is_empty());
}

#[test]
fn member_glob_does_not_cross_directories() {
    let report = analyze(&[
        ("Cargo.toml", "[workspace]\nmembers=['crates/*']\n"),
        (
            "crates/a/b/Cargo.toml",
            "[package]\nname='ab'\nversion='0.1.0'\n",
        ),
    ]);
    assert_eq!(
        data(&report, "cargo-workspace", "Cargo.toml")["members"],
        json!([])
    );
    assert_eq!(
        data(&report, "cargo-package", "crates/a/b/Cargo.toml")["workspace"],
        Value::Null
    );
    assert_eq!(
        diagnostics(&report),
        vec![(
            Severity::Warning,
            Some("Cargo.toml"),
            "workspace member pattern matches no Cargo.toml: crates/*"
        )]
    );
}
