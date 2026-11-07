//! CLI contract tests: exit codes (`architecture.md` §6) and stdout discipline.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use repo_intelligence_json::Report;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name)
}

fn cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_repo-intelligence"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn valid_fixture_exits_zero_with_only_json_on_stdout() {
    let out = cli(&["analyze", fixture("rust-workspace").to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(0));
    assert!(
        out.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let report: Report = serde_json::from_slice(&out.stdout).expect("stdout is only a report");
    assert!(!report.findings.is_empty());
}

#[test]
fn messy_fixture_exits_zero_and_reports_diagnostics() {
    let out = cli(&["analyze", fixture("messy").to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(0));
    let report: Report = serde_json::from_slice(&out.stdout).unwrap();
    assert!(!report.diagnostics.is_empty());
}

#[test]
fn missing_path_exits_one_with_empty_stdout() {
    let out = cli(&["analyze", "/definitely/not/a/path"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    assert!(!out.stderr.is_empty());
}

#[test]
fn file_path_exits_one() {
    let out = cli(&[
        "analyze",
        fixture("rust-single-crate/Cargo.toml").to_str().unwrap(),
    ]);
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
}

#[test]
fn unknown_flag_exits_one_not_clap_default_two() {
    let out = cli(&["analyze", "--bogus", "."]);
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
}

#[test]
fn unknown_format_exits_one() {
    let out = cli(&["analyze", ".", "--format", "yaml"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
}

#[test]
fn help_and_version_exit_zero_on_stdout() {
    for args in [&["--help"][..], &["--version"][..]] {
        let out = cli(args);
        assert_eq!(out.status.code(), Some(0));
        assert!(!out.stdout.is_empty());
    }
}

#[cfg(unix)]
#[test]
fn unreadable_root_exits_two_with_empty_stdout() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    let locked = dir.path().join("locked");
    std::fs::create_dir(&locked).unwrap();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();

    // Root ignores directory permissions; skip rather than fail.
    if std::fs::read_dir(&locked).is_ok() {
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
        return;
    }

    let out = cli(&["analyze", locked.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());

    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
}
