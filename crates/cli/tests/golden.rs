//! Golden tests: the real binary on committed fixtures, compared byte-for-byte
//! with `fixtures/<name>/expected.json` (plain) and `expected.git.json` (with a
//! deterministic git history overlaid). Only `tool_version` and `analyzed_at` are
//! normalized, because they are the only runtime-injected values.
//!
//! Regenerate with `cargo test -p repo-intelligence-cli regenerate_golden -- --ignored`,
//! then review every changed line against the detector docs. Golden diffs get
//! the same scrutiny as code.
//!
//! `messy/expected.json` contains a TOML parse position (`line 1, column 9`)
//! taken from the `toml` crate's error span. A `toml` upgrade may move it. If
//! that is the only change in a golden diff, it is expected; review it as such.
//!
//! Git-overlay cases need a `git` binary; without one they are skipped with a note.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::Value;

const NULL_DEVICE: &str = if cfg!(windows) { "NUL" } else { "/dev/null" };

fn git_available() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

const PLAIN: &[&str] = &["rust-single-crate", "rust-workspace", "messy"];
const WITH_GIT: &[&str] = &["rust-single-crate", "rust-workspace"];

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name)
}

/// Runs the binary and returns the report with runtime values replaced.
fn normalized_report(dir: &Path) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_repo-intelligence"))
        .arg("analyze")
        .arg(dir)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(out.status.success(), "exit {:?}", out.status.code());
    let mut report: Value = serde_json::from_slice(&out.stdout).expect("stdout is JSON");
    report["tool_version"] = Value::from("<tool_version>");
    report["analyzed_at"] = Value::from("<analyzed_at>");
    let mut text = serde_json::to_string_pretty(&report).unwrap();
    text.push('\n');
    text
}

fn expected(name: &str, file: &str) -> String {
    std::fs::read_to_string(fixture(name).join(file))
        .unwrap_or_else(|_| panic!("missing {name}/{file}; run regenerate_golden"))
}

/// Copies a fixture's inputs. Golden files are skipped: they are outputs, and
/// including them would change the git tree (and so every commit id) on each
/// regeneration.
fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        if entry.file_name().to_string_lossy().starts_with("expected") {
            continue;
        }
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// Runs `git` in `dir` with fixed identity and dates, and no ambient configuration.
fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .current_dir(dir)
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap())
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", NULL_DEVICE)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("LC_ALL", "C")
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@example.invalid")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@example.invalid")
        .env("GIT_AUTHOR_DATE", "2020-01-01T00:00:00Z")
        .env("GIT_COMMITTER_DATE", "2020-01-01T00:00:00Z")
        .args(["-c", "init.defaultBranch=main", "-c", "tag.gpgSign=false"])
        .args(args)
        .output()
        .expect("git is required for the golden git tests");
    assert!(out.status.success(), "git {args:?} failed");
    String::from_utf8(out.stdout).unwrap()
}

/// Copies a fixture into `dir` and gives it a fixed history with four tags:
/// `v0.1.0` (lightweight), `v0.2.0` (annotated), `alpha-v2.1.0` (annotated),
/// and `nightly` (lightweight, not a version).
fn with_git_history(name: &str, dir: &Path) {
    copy_dir(&fixture(name), dir);
    git(dir, &["init", "-q"]);
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", "fixture"]);
    git(dir, &["tag", "v0.1.0"]);
    git(dir, &["commit", "-q", "--allow-empty", "-m", "second"]);
    git(dir, &["tag", "-a", "v0.2.0", "-m", "release 0.2.0"]);
    git(dir, &["tag", "-a", "alpha-v2.1.0", "-m", "alpha 2.1.0"]);
    git(dir, &["tag", "nightly"]);
}

#[test]
fn plain_reports_match_goldens() {
    for name in PLAIN {
        assert_eq!(
            normalized_report(&fixture(name)),
            expected(name, "expected.json"),
            "golden mismatch for {name}"
        );
    }
}

#[test]
fn git_overlay_reports_match_goldens() {
    if !git_available() {
        eprintln!("skipped: git is not installed");
        return;
    }
    for name in WITH_GIT {
        let dir = tempfile::tempdir().unwrap();
        with_git_history(name, dir.path());
        assert_eq!(
            normalized_report(dir.path()),
            expected(name, "expected.git.json"),
            "git golden mismatch for {name}"
        );
    }
}

#[test]
fn reports_are_identical_across_runs_and_locations() {
    if !git_available() {
        eprintln!("skipped: git is not installed");
        return;
    }
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    with_git_history("rust-workspace", first.path());
    with_git_history("rust-workspace", second.path());

    let a = normalized_report(first.path());
    assert_eq!(a, normalized_report(first.path()));
    assert_eq!(a, normalized_report(second.path()));
}

#[test]
fn runtime_values_are_injected() {
    let out = Command::new(env!("CARGO_BIN_EXE_repo-intelligence"))
        .args(["analyze"])
        .arg(fixture("rust-single-crate"))
        .output()
        .unwrap();
    let report: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(report["tool_version"], env!("CARGO_PKG_VERSION"));
    let at = report["analyzed_at"].as_str().unwrap();
    assert_eq!(at.len(), 20, "{at}");
    assert!(at.ends_with('Z') && at.as_bytes()[10] == b'T', "{at}");
}

#[test]
#[ignore]
fn regenerate_golden() {
    assert!(git_available(), "regenerating goldens requires git");
    for name in PLAIN {
        std::fs::write(
            fixture(name).join("expected.json"),
            normalized_report(&fixture(name)),
        )
        .unwrap();
    }
    for name in WITH_GIT {
        let dir = tempfile::tempdir().unwrap();
        with_git_history(name, dir.path());
        std::fs::write(
            fixture(name).join("expected.git.json"),
            normalized_report(dir.path()),
        )
        .unwrap();
    }
}
