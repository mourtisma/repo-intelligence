//! Git tag collection: the only place in the engine that runs a process.
//!
//! Runs the system `git` once per scan, with a cleared environment and global
//! and system config disabled, so output depends only on the repository
//! (`architecture.md` §5). Requires git 2.32+ for `GIT_CONFIG_GLOBAL`.
//!
//! Failures are diagnostics with fixed messages. Git's own stderr is never
//! copied, because it contains absolute paths and localized text.
//!
//! Git runs against the **canonical** root, with `GIT_CEILING_DIRECTORIES` set
//! to the canonical parent. Git ignores relative or symlinked ceiling entries,
//! so without canonicalization a broken `.git` under a relative root made git
//! walk up and report a parent repository's tags.
//!
//! Each `git` call is bounded by a timeout. A timeout is a diagnostic, so it is
//! the one case where output can depend on machine speed.

use std::ffi::OsStr;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use repo_intelligence_json::Diagnostic;

use crate::context::warning;

/// A git tag as found in the repository. A tag is git state, not a release:
/// deciding which tags are versions is the detector's job.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct GitTag {
    /// Tag name without the `refs/tags/` prefix.
    pub name: String,
    /// True for annotated tag objects, false for lightweight tags.
    pub annotated: bool,
    /// The commit id the tag names, peeled one level. `None` when the tag names
    /// a non-commit object (tree, blob, or a tag of a tag).
    pub commit: Option<String>,
}

impl GitTag {
    pub fn new(name: impl Into<String>, annotated: bool, commit: Option<String>) -> Self {
        Self {
            name: name.into(),
            annotated,
            commit,
        }
    }
}

/// Field order matches `parse_tag_line`. Each field is separated by NUL (`%00`).
const FOR_EACH_REF_FORMAT: &str =
    "--format=%(refname)%00%(objecttype)%00%(*objecttype)%00%(objectname)%00%(*objectname)";

/// Result of collecting git data for one scan.
pub(crate) struct GitScan {
    /// `None` when the root is not a readable git repository.
    pub tags: Option<Vec<GitTag>>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Upper bound for one `git` call.
const GIT_TIMEOUT: Duration = Duration::from_secs(30);

/// Collects tags for the repository at `root`, running `git` as `git_exe`.
/// `git_exe` is a parameter so tests can simulate a missing binary.
pub(crate) fn collect(root: &Path, git_exe: &OsStr) -> GitScan {
    collect_with_timeout(root, git_exe, GIT_TIMEOUT)
}

fn collect_with_timeout(root: &Path, git_exe: &OsStr, timeout: Duration) -> GitScan {
    let mut diagnostics = Vec::new();

    if !root.join(".git").exists() {
        return GitScan {
            tags: None,
            diagnostics,
        };
    }
    let Some(root) = canonical(root) else {
        diagnostics.push(RunProblem::Unreadable.diagnostic());
        return GitScan {
            tags: None,
            diagnostics,
        };
    };
    let root = root.as_path();

    let output = match run(
        command(
            git_exe,
            root,
            &["for-each-ref", FOR_EACH_REF_FORMAT, "refs/tags"],
        ),
        timeout,
    ) {
        Ok(output) => output,
        Err(problem) => {
            diagnostics.push(problem.diagnostic());
            return GitScan {
                tags: None,
                diagnostics,
            };
        }
    };

    let mut tags = Vec::new();
    for line in output.split(|&b| b == b'\n').filter(|l| !l.is_empty()) {
        match std::str::from_utf8(line).ok().and_then(parse_tag_line) {
            Some(tag) => tags.push(tag),
            None => diagnostics.push(warning(
                None,
                "unrecognized git output line skipped".to_string(),
            )),
        }
    }
    tags.sort_by(|a, b| a.name.cmp(&b.name));

    // Advisory only: if this check fails, the tag list above is still valid.
    if let Ok(out) = run(
        command(git_exe, root, &["rev-parse", "--is-shallow-repository"]),
        timeout,
    ) {
        if out.trim_ascii() == b"true" {
            diagnostics.push(warning(
                None,
                "shallow clone: tag list may be incomplete".to_string(),
            ));
        }
    }

    GitScan {
        tags: Some(tags),
        diagnostics,
    }
}

/// The canonical form of `path`, without Windows' `\\?\` verbatim prefix,
/// which git does not accept.
fn canonical(path: &Path) -> Option<PathBuf> {
    let path = std::fs::canonicalize(path).ok()?;
    if cfg!(windows) {
        if let Some(plain) = path.to_str().and_then(|s| s.strip_prefix(r"\\?\")) {
            return Some(PathBuf::from(plain));
        }
    }
    Some(path)
}

/// Parses one `for-each-ref` line. Returns `None` for anything unexpected.
fn parse_tag_line(line: &str) -> Option<GitTag> {
    let fields: Vec<&str> = line.split('\0').collect();
    let [refname, objecttype, peeled_type, objectname, peeled_name] = fields[..] else {
        return None;
    };
    let name = refname
        .strip_prefix("refs/tags/")
        .filter(|n| !n.is_empty())?;

    let annotated = objecttype == "tag";
    let commit = if annotated {
        (peeled_type == "commit" && !peeled_name.is_empty()).then(|| peeled_name.to_string())
    } else if objecttype == "commit" {
        Some(objectname.to_string())
    } else {
        None
    };
    Some(GitTag::new(name, annotated, commit))
}

/// Builds the hardened `git` invocation. The environment is cleared and then
/// filled from an explicit allowlist. `PATH` is the only ambient input.
/// `root` must be canonical, so the ceiling below is absolute.
fn command(git_exe: &OsStr, root: &Path, args: &[&str]) -> Command {
    let null_device = if cfg!(windows) { "NUL" } else { "/dev/null" };
    let mut cmd = Command::new(git_exe);
    cmd.env_clear();
    if let Some(path) = std::env::var_os("PATH") {
        cmd.env("PATH", path);
    }
    if cfg!(windows) {
        // Windows processes need SystemRoot to start correctly.
        if let Some(system_root) = std::env::var_os("SystemRoot") {
            cmd.env("SystemRoot", system_root);
        }
    }
    if let Some(parent) = root.parent() {
        // Stops git from searching above the root for a repository.
        cmd.env("GIT_CEILING_DIRECTORIES", parent);
    }
    cmd.env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", null_device)
        .env("LC_ALL", "C")
        .env("GIT_TERMINAL_PROMPT", "0")
        .arg("-C")
        .arg(root)
        .args(["-c", "core.fsmonitor=false"])
        .args(args)
        .stdin(Stdio::null());
    cmd
}

enum RunProblem {
    NotFound,
    Unreadable,
    TimedOut,
}

impl RunProblem {
    fn diagnostic(&self) -> Diagnostic {
        let message = match self {
            RunProblem::NotFound => "git not found; tags not collected",
            RunProblem::Unreadable => "git metadata unreadable; tags not collected",
            RunProblem::TimedOut => "git timed out; tags not collected",
        };
        warning(None, message.to_string())
    }
}

/// Runs `cmd`, waiting at most `timeout`. stdout is read on a thread so a large
/// tag list cannot fill the pipe and block git. stderr is discarded.
fn run(mut cmd: Command, timeout: Duration) -> Result<Vec<u8>, RunProblem> {
    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => RunProblem::NotFound,
            _ => RunProblem::Unreadable,
        })?;
    let mut stdout = child.stdout.take().expect("stdout is piped");
    let reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        stdout.read_to_end(&mut buf).map(|_| buf)
    });

    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(5)),
            other => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(match other {
                    Ok(None) => RunProblem::TimedOut,
                    _ => RunProblem::Unreadable,
                });
            }
        }
    };
    let output = reader
        .join()
        .ok()
        .and_then(Result::ok)
        .ok_or(RunProblem::Unreadable)?;
    if status.success() {
        Ok(output)
    } else {
        Err(RunProblem::Unreadable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_lightweight_commit_tag() {
        let line = "refs/tags/v0.1.0\0commit\0\0abc123\0";
        assert_eq!(
            parse_tag_line(line),
            Some(GitTag::new("v0.1.0", false, Some("abc123".into())))
        );
    }

    #[test]
    fn parses_annotated_tag_peeled_to_commit() {
        let line = "refs/tags/v0.2.0\0tag\0commit\0tagobj\0abc123";
        assert_eq!(
            parse_tag_line(line),
            Some(GitTag::new("v0.2.0", true, Some("abc123".into())))
        );
    }

    #[test]
    fn tag_naming_a_tree_has_no_commit() {
        let line = "refs/tags/t\0tree\0\0treeid\0";
        assert_eq!(parse_tag_line(line), Some(GitTag::new("t", false, None)));
    }

    #[test]
    fn tag_of_tag_has_no_commit() {
        let line = "refs/tags/tt\0tag\0tag\0outer\0inner";
        assert_eq!(parse_tag_line(line), Some(GitTag::new("tt", true, None)));
    }

    #[test]
    fn rejects_malformed_lines() {
        assert_eq!(parse_tag_line("refs/tags/x\0commit"), None);
        assert_eq!(parse_tag_line("refs/heads/x\0commit\0\0id\0"), None);
        assert_eq!(parse_tag_line("refs/tags/\0commit\0\0id\0"), None);
    }

    #[test]
    fn command_environment_is_an_allowlist() {
        let cmd = command(OsStr::new("git"), Path::new("/r/repo"), &["status"]);
        let allowed = [
            "PATH",
            "SystemRoot",
            "GIT_CONFIG_NOSYSTEM",
            "GIT_CONFIG_GLOBAL",
            "GIT_CEILING_DIRECTORIES",
            "LC_ALL",
            "GIT_TERMINAL_PROMPT",
        ];
        for (key, _) in cmd.get_envs() {
            let key = key.to_string_lossy();
            assert!(allowed.contains(&key.as_ref()), "unexpected env var {key}");
        }
    }

    #[test]
    fn missing_git_binary_is_a_warning_and_no_tags() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join(".git")).unwrap();
        let scan = collect(dir.path(), OsStr::new("/nonexistent/definitely-not-git"));
        assert_eq!(scan.tags, None);
        assert_eq!(scan.diagnostics.len(), 1);
        assert_eq!(
            scan.diagnostics[0].message,
            "git not found; tags not collected"
        );
        assert_eq!(scan.diagnostics[0].path, None);
    }

    #[test]
    fn no_dot_git_means_not_a_repository_and_no_diagnostic() {
        let dir = tempfile::tempdir().unwrap();
        let scan = collect(dir.path(), OsStr::new("git"));
        assert_eq!(scan.tags, None);
        assert!(scan.diagnostics.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn slow_git_times_out_with_a_warning() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join(".git")).unwrap();
        let fake = dir.path().join("slow-git");
        std::fs::write(&fake, "#!/bin/sh\nexec sleep 5\n").unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();

        let started = Instant::now();
        let scan = collect_with_timeout(dir.path(), fake.as_os_str(), Duration::from_millis(200));
        assert!(
            started.elapsed() < Duration::from_secs(4),
            "did not time out"
        );
        assert_eq!(scan.tags, None);
        assert_eq!(
            scan.diagnostics[0].message,
            "git timed out; tags not collected"
        );
    }

    #[test]
    fn corrupt_dot_git_is_unreadable_warning() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join(".git")).unwrap();
        std::fs::write(dir.path().join(".git/HEAD"), "garbage\n").unwrap();
        let scan = collect(dir.path(), OsStr::new("git"));
        assert_eq!(scan.tags, None);
        assert_eq!(
            scan.diagnostics[0].message,
            "git metadata unreadable; tags not collected"
        );
    }
}
