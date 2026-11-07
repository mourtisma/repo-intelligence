//! Tests against real, throwaway git repositories. Each repository is built in a
//! tempdir with fixed identities and dates, so commit ids are the same everywhere
//! and the developer's git configuration is never read.

use std::path::{Path, PathBuf};
use std::process::Command;

use repo_intelligence_core::{scan, GitTag};

const NULL_DEVICE: &str = if cfg!(windows) { "NUL" } else { "/dev/null" };

/// True if a `git` binary can run. Tests return early (with a note) without it,
/// so a machine without git reports skipped work instead of failures.
fn git_available() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

macro_rules! require_git {
    () => {
        if !git_available() {
            eprintln!("skipped: git is not installed");
            return;
        }
    };
}

/// Runs `git` in `dir` with the same isolation as the engine. Returns stdout.
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
        .expect("git is required for these tests");
    assert!(out.status.success(), "git {args:?} failed");
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

/// A repository with one commit, a lightweight tag `v0.1.0`, and an
/// annotated tag `v0.2.0`. Returns the directory and the commit id.
fn repo_with_tags(dir: &Path) -> String {
    git(dir, &["init", "-q"]);
    std::fs::write(dir.join("f.txt"), "x\n").unwrap();
    git(dir, &["add", "f.txt"]);
    git(dir, &["commit", "-q", "-m", "init"]);
    let head = git(dir, &["rev-parse", "HEAD"]);
    git(dir, &["tag", "v0.1.0"]);
    git(dir, &["tag", "-a", "v0.2.0", "-m", "release"]);
    head
}

fn tempdir() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

#[test]
fn lightweight_and_annotated_tags_are_distinguished() {
    require_git!();
    let dir = tempdir();
    let head = repo_with_tags(dir.path());

    let ctx = scan(dir.path()).unwrap();
    assert_eq!(
        ctx.git_tags().unwrap(),
        &[
            GitTag::new("v0.1.0", false, Some(head.clone())),
            GitTag::new("v0.2.0", true, Some(head)),
        ]
    );
    assert!(ctx.diagnostics().is_empty());
}

#[test]
fn tag_naming_a_tree_has_no_commit() {
    require_git!();
    let dir = tempdir();
    repo_with_tags(dir.path());
    let tree = git(dir.path(), &["rev-parse", "HEAD^{tree}"]);
    git(dir.path(), &["tag", "tree-tag", &tree]);

    let ctx = scan(dir.path()).unwrap();
    let tag = ctx
        .git_tags()
        .unwrap()
        .iter()
        .find(|t| t.name == "tree-tag")
        .unwrap();
    assert_eq!(tag, &GitTag::new("tree-tag", false, None));
}

#[test]
fn repository_without_tags_yields_empty_list() {
    require_git!();
    let dir = tempdir();
    git(dir.path(), &["init", "-q"]);
    let ctx = scan(dir.path()).unwrap();
    assert_eq!(ctx.git_tags(), Some(&[][..]));
    assert!(ctx.diagnostics().is_empty());
}

#[test]
fn shallow_clone_warns_that_tags_may_be_incomplete() {
    require_git!();
    let source = tempdir();
    repo_with_tags(source.path());
    let clone_root = tempdir();
    let dest: PathBuf = clone_root.path().join("clone");
    let url = format!("file://{}", source.path().display());
    git(
        clone_root.path(),
        &["clone", "-q", "--depth", "1", &url, dest.to_str().unwrap()],
    );

    let ctx = scan(&dest).unwrap();
    let messages: Vec<&str> = ctx
        .diagnostics()
        .iter()
        .map(|d| d.message.as_str())
        .collect();
    assert_eq!(messages, vec!["shallow clone: tag list may be incomplete"]);
    assert!(ctx.git_tags().is_some());
}

#[test]
fn subdirectory_of_a_repository_has_no_git_data() {
    require_git!();
    let dir = tempdir();
    repo_with_tags(dir.path());
    let sub = dir.path().join("sub");
    std::fs::create_dir(&sub).unwrap();

    let ctx = scan(&sub).unwrap();
    assert_eq!(ctx.git_tags(), None);
    assert!(ctx.diagnostics().is_empty());
}

#[test]
fn same_history_gives_identical_tags_in_different_locations() {
    require_git!();
    let first = tempdir();
    let second = tempdir();
    repo_with_tags(first.path());
    repo_with_tags(second.path());

    let a = scan(first.path()).unwrap();
    let b = scan(second.path()).unwrap();
    assert_eq!(a.git_tags(), b.git_tags());
}

/// A hostile global config must not change the result, or run anything.
/// `HOME` is set for this process only; the engine's git runs with a cleared
/// environment, so it must not read `HOME/.gitconfig` in any case.
#[cfg(unix)]
#[test]
fn hostile_global_config_is_ignored() {
    require_git!();
    let dir = tempdir();
    let head = repo_with_tags(dir.path());
    let baseline = scan(dir.path()).unwrap().git_tags().unwrap().to_vec();

    let home = tempdir();
    let marker = home.path().join("ran");
    std::fs::write(
        home.path().join(".gitconfig"),
        format!(
            "[tag]\n\tsort = -refname\n[core]\n\tfsmonitor = touch {}\n[alias]\n\tfor-each-ref = !touch {} ; false\n",
            marker.display(),
            marker.display()
        ),
    )
    .unwrap();
    let previous = std::env::var_os("HOME");
    std::env::set_var("HOME", home.path());
    let result = scan(dir.path());
    match previous {
        Some(v) => std::env::set_var("HOME", v),
        None => std::env::remove_var("HOME"),
    }

    let tags = result.unwrap();
    assert_eq!(tags.git_tags().unwrap(), baseline.as_slice());
    assert_eq!(baseline[0].commit.as_deref(), Some(head.as_str()));
    assert!(!marker.exists(), "global config executed a command");
}

/// The path to `target` relative to the current directory, built with `..`
/// components so the test does not change the process's working directory.
fn relative_from_cwd(target: &Path) -> PathBuf {
    let cwd = std::env::current_dir().unwrap().canonicalize().unwrap();
    let target = target.canonicalize().unwrap();
    let common = cwd
        .components()
        .zip(target.components())
        .take_while(|(a, b)| a == b)
        .count();
    let mut relative = PathBuf::new();
    for _ in cwd.components().skip(common) {
        relative.push("..");
    }
    for part in target.components().skip(common) {
        relative.push(part);
    }
    relative
}

/// Regression (Phase 3 review F1): a relative root whose `.git` is broken must
/// not make git walk up and report the enclosing repository's tags.
#[test]
fn broken_dot_git_under_relative_root_does_not_leak_parent_tags() {
    require_git!();
    let outer = tempdir();
    repo_with_tags(outer.path());
    let inner = outer.path().join("inner");
    std::fs::create_dir_all(inner.join(".git")).unwrap();
    std::fs::write(inner.join(".git/HEAD"), "garbage\n").unwrap();

    let relative = relative_from_cwd(&inner);
    assert!(relative.is_relative());
    let ctx = scan(&relative).unwrap();
    assert_eq!(ctx.git_tags(), None);
    let messages: Vec<&str> = ctx
        .diagnostics()
        .iter()
        .map(|d| d.message.as_str())
        .collect();
    assert_eq!(
        messages,
        vec!["git metadata unreadable; tags not collected"]
    );
}
