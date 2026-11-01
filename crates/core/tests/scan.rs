//! Fixture-backed tests for the single traversal and its pinned policies.

mod common;

use common::{fixture, gitignore_tree};
use repo_intelligence_core::{scan, EngineError};
use repo_intelligence_json::Severity;

#[test]
fn nested_tree_is_listed_in_byte_wise_order() {
    let ctx = scan(&fixture("nested")).unwrap();
    let paths: Vec<&str> = ctx.paths().collect();
    // Byte-wise: "." < "A" < "a"; "a/..." before "top.md". Dot-directories are listed.
    assert_eq!(
        paths,
        vec![
            ".github/workflows/ci.yml",
            "A.txt",
            "a/b/c.txt",
            "a/z.txt",
            "top.md"
        ]
    );
    assert!(ctx.diagnostics().is_empty());
}

#[test]
fn gitignore_is_honored_and_only_dot_git_is_excluded() {
    let dir = gitignore_tree();
    let ctx = scan(dir.path()).unwrap();
    let paths: Vec<&str> = ctx.paths().collect();
    // `.gitignore` excludes `build/` and `*.tmp` without a real git repo;
    // `.git/info/exclude` excludes `secret.txt`; `.ignore` is NOT honored (so
    // `keep.txt` stays); dotfiles are listed; `.git/` itself is excluded.
    assert_eq!(
        paths,
        vec![
            ".gitignore",
            ".hidden",
            ".ignore",
            "keep.txt",
            "src/main.txt"
        ]
    );
    assert!(ctx.diagnostics().is_empty());
}

#[test]
fn empty_directory_yields_empty_listing() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("sub")).unwrap();
    let ctx = scan(dir.path()).unwrap();
    assert_eq!(ctx.paths().count(), 0);
    assert!(ctx.diagnostics().is_empty());
}

#[test]
fn scan_is_repeatable() {
    let first = scan(&fixture("nested")).unwrap();
    for _ in 0..5 {
        let again = scan(&fixture("nested")).unwrap();
        assert_eq!(
            again.paths().collect::<Vec<_>>(),
            first.paths().collect::<Vec<_>>()
        );
    }
}

#[test]
fn scanned_files_are_readable() {
    let ctx = scan(&fixture("nested")).unwrap();
    assert_eq!(ctx.read("a/z.txt").unwrap(), b"z\n");
}

#[test]
fn missing_root_is_engine_error() {
    let err = scan(&fixture("does-not-exist")).unwrap_err();
    assert!(matches!(err, EngineError::RootNotFound(_)));
}

#[test]
fn file_root_is_engine_error() {
    let err = scan(&fixture("nested/top.md")).unwrap_err();
    assert!(matches!(err, EngineError::RootNotADirectory(_)));
}

#[cfg(unix)]
#[test]
fn symlink_is_skipped_with_warning() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("real.txt"), b"x").unwrap();
    std::os::unix::fs::symlink(dir.path().join("real.txt"), dir.path().join("link.txt")).unwrap();

    let ctx = scan(dir.path()).unwrap();
    let paths: Vec<&str> = ctx.paths().collect();
    assert_eq!(paths, vec!["real.txt"]);

    let diags = ctx.diagnostics();
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].severity, Severity::Warning);
    assert_eq!(diags[0].path.as_deref(), Some("link.txt"));
}

#[cfg(unix)]
#[test]
fn non_utf8_path_is_skipped_with_warning() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("ok.txt"), b"x").unwrap();
    let bad = dir.path().join(OsStr::from_bytes(b"bad\xff.txt"));
    std::fs::write(bad, b"x").unwrap();

    let ctx = scan(dir.path()).unwrap();
    let paths: Vec<&str> = ctx.paths().collect();
    assert_eq!(paths, vec!["ok.txt"]);
    assert_eq!(ctx.diagnostics().len(), 1);
    assert_eq!(ctx.diagnostics()[0].severity, Severity::Warning);
    assert_eq!(ctx.diagnostics()[0].path, None);
    assert_eq!(
        ctx.diagnostics()[0].message,
        "non-UTF-8 path skipped: bad\u{FFFD}.txt"
    );
}

#[cfg(unix)]
#[test]
fn unreadable_subdirectory_is_diagnosed_not_fatal() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("ok.txt"), b"x").unwrap();
    let locked = dir.path().join("locked");
    std::fs::create_dir(&locked).unwrap();
    std::fs::write(locked.join("secret.txt"), b"x").unwrap();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();

    // Running as root ignores directory permissions; skip rather than fail.
    if std::fs::read_dir(&locked).is_ok() {
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
        return;
    }

    let ctx = scan(dir.path()).unwrap();
    let paths: Vec<&str> = ctx.paths().collect();
    assert_eq!(paths, vec!["ok.txt"]);
    let diags = ctx.diagnostics();
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].severity, Severity::Warning);
    assert_eq!(diags[0].path.as_deref(), Some("locked"));
    // Fixed phrase: no absolute path, no OS error text.
    assert_eq!(diags[0].message, "unreadable entry: permission denied");

    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
}
