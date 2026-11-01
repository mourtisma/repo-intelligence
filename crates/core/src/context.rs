//! `RepoContext`: the read-only, in-memory model of a scanned repository, and the
//! single traversal that builds it.
//!
//! Traversal policies (pinned by tests in `tests/scan.rs`):
//! - **Sorted:** file paths are ordered byte-wise on their forward-slash form.
//! - **Hidden entries listed:** dotfiles such as `.github/workflows/*.yml` are
//!   part of the listing. Only directories or files named `.git` are excluded.
//! - **`.gitignore` honored:** `.gitignore` files inside the root (and
//!   `.git/info/exclude`) apply even without a `.git` directory. `.ignore` files,
//!   ignore files in parent directories, and the user's global gitignore are NOT
//!   read, so results do not depend on the machine the scan runs on.
//! - **Symlinks not followed:** a symlink is skipped with a warning diagnostic.
//!   Symlinked directories (e.g. monorepo package links) are therefore not scanned.
//! - **Directories not listed:** only regular files appear in the listing.
//! - **Non-UTF-8 paths skipped:** with a warning diagnostic (path is `None`).
//! - **Unreadable entries:** recorded as warning diagnostics; the scan continues.
//! - **Diagnostics are machine-independent:** messages are fixed phrases and
//!   paths are repo-relative; no absolute paths or OS error text are emitted.
//!
//! `read()` loads whole files with no size limit. A cap is deferred until a
//! detector needs to read potentially large files.

use std::collections::BTreeMap;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::OnceLock;

use std::ffi::OsStr;

use ignore::WalkBuilder;
use repo_intelligence_json::{Diagnostic, Severity};

use crate::error::EngineError;
use crate::git::{self, GitTag};

/// Stable detector id used for diagnostics produced by the traversal itself.
pub(crate) const SCANNER_ID: &str = "scanner";

/// Read-only model of a repository: root, sorted relative file paths, and
/// on-demand file reads. Immutable after construction; detectors receive `&RepoContext`.
#[derive(Debug)]
pub struct RepoContext {
    root: PathBuf,
    /// Keyed by repo-relative, forward-slash path; `BTreeMap` gives the byte-wise order.
    files: BTreeMap<String, OnceLock<Vec<u8>>>,
    diagnostics: Vec<Diagnostic>,
    /// `None` when the root is not a readable git repository.
    git_tags: Option<Vec<GitTag>>,
}

impl RepoContext {
    /// Builds a context from an explicit path list, with no traversal diagnostics.
    /// Paths are sorted and de-duplicated. Intended for tests and synthetic inputs.
    #[doc(hidden)]
    pub fn from_paths<I, S>(root: impl Into<PathBuf>, paths: I) -> RepoContext
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        RepoContext {
            root: root.into(),
            files: paths
                .into_iter()
                .map(|p| (p.into(), OnceLock::new()))
                .collect(),
            diagnostics: Vec::new(),
            git_tags: None,
        }
    }

    /// Returns this context with git tags set. Intended for tests and synthetic inputs.
    #[doc(hidden)]
    pub fn with_git_tags(mut self, tags: Vec<GitTag>) -> Self {
        self.git_tags = Some(tags);
        self
    }

    /// Tags in byte-wise name order, or `None` if the root is not a readable repository.
    pub fn git_tags(&self) -> Option<&[GitTag]> {
        self.git_tags.as_deref()
    }

    /// The repository root path this context was built from.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Repo-relative, forward-slash file paths in byte-wise sorted order.
    pub fn paths(&self) -> impl Iterator<Item = &str> {
        self.files.keys().map(String::as_str)
    }

    /// True if `path` is in the listing. Exact match on the normalized form.
    pub fn contains(&self, path: &str) -> bool {
        self.files.contains_key(path)
    }

    /// Diagnostics produced by the traversal (unreadable, symlinked, non-UTF-8 entries).
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Reads a file listed in this context, by its repo-relative path.
    ///
    /// Successful reads are memoized, so a file is read from disk at most once
    /// in single-threaded use. Concurrent first reads may both hit the disk; the
    /// first result stored wins. Failed reads are not cached. Paths not in the
    /// listing return `NotFound`.
    pub fn read(&self, path: &str) -> io::Result<&[u8]> {
        let cell = self.files.get(path).ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, format!("not in listing: {path}"))
        })?;
        if let Some(bytes) = cell.get() {
            return Ok(bytes);
        }
        let bytes = std::fs::read(self.root.join(path))?;
        Ok(cell.get_or_init(|| bytes))
    }
}

/// Scans `root` once and returns its `RepoContext`.
///
/// Fails only when the root itself is unusable. Every problem below the root
/// becomes a diagnostic on the returned context.
pub fn scan(root: &Path) -> Result<RepoContext, EngineError> {
    scan_with_git(root, OsStr::new("git"))
}

/// `scan`, with the `git` executable given explicitly (tests simulate a missing binary).
pub(crate) fn scan_with_git(root: &Path, git_exe: &OsStr) -> Result<RepoContext, EngineError> {
    let meta = std::fs::metadata(root).map_err(|e| match e.kind() {
        io::ErrorKind::NotFound => EngineError::RootNotFound(root.to_path_buf()),
        _ => EngineError::RootUnreadable(root.to_path_buf(), e),
    })?;
    if !meta.is_dir() {
        return Err(EngineError::RootNotADirectory(root.to_path_buf()));
    }
    if let Err(e) = std::fs::read_dir(root) {
        return Err(EngineError::RootUnreadable(root.to_path_buf(), e));
    }

    let mut paths = Vec::new();
    let mut diagnostics = Vec::new();

    let walker = WalkBuilder::new(root)
        .hidden(false)
        .ignore(false)
        .require_git(false)
        .parents(false)
        .git_global(false)
        .follow_links(false)
        .filter_entry(|entry| entry.file_name() != ".git")
        .sort_by_file_path(|a, b| a.cmp(b))
        .build();

    for entry in walker {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                // Never format `err` itself: it embeds absolute paths and
                // OS-specific text, which would make reports machine-dependent.
                let path = error_path(&err).and_then(|p| normalize_rel(root, p));
                diagnostics.push(warning(path, error_message(&err).to_string()));
                continue;
            }
        };
        // The root itself is yielded first; it is not a file in the listing.
        if entry.depth() == 0 {
            continue;
        }
        let Some(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_symlink() {
            let rel = normalize_rel(root, entry.path());
            diagnostics.push(warning(rel.clone(), "symlink not followed".to_string()));
            continue;
        }
        if !file_type.is_file() {
            continue;
        }
        match normalize_rel(root, entry.path()) {
            Some(rel) => paths.push(rel),
            None => diagnostics.push(warning(
                None,
                format!("non-UTF-8 path skipped: {}", lossy_rel(root, entry.path())),
            )),
        }
    }

    let git = git::collect(root, git_exe);
    diagnostics.extend(git.diagnostics);

    Ok(RepoContext {
        root: root.to_path_buf(),
        files: paths.into_iter().map(|p| (p, OnceLock::new())).collect(),
        diagnostics,
        git_tags: git.tags,
    })
}

/// Converts `path` (under `root`) to a forward-slash, repo-relative string.
/// Returns `None` if any component is not valid UTF-8.
fn normalize_rel(root: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?;
    let mut parts = Vec::new();
    for component in rel.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_str()?),
            _ => return None,
        }
    }
    Some(parts.join("/"))
}

/// Like `normalize_rel`, but replaces invalid UTF-8 instead of failing.
/// Used only inside diagnostic messages.
fn lossy_rel(root: &Path, path: &Path) -> String {
    let rel = path.strip_prefix(root).unwrap_or(path);
    rel.components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// The innermost path an `ignore::Error` refers to, if any.
fn error_path(err: &ignore::Error) -> Option<&Path> {
    match err {
        ignore::Error::WithPath { path, .. } => Some(path),
        ignore::Error::WithDepth { err, .. } | ignore::Error::WithLineNumber { err, .. } => {
            error_path(err)
        }
        ignore::Error::Loop { child, .. } => Some(child),
        ignore::Error::Partial(errs) => errs.first().and_then(error_path),
        _ => None,
    }
}

/// A fixed, platform-independent message for a traversal error.
fn error_message(err: &ignore::Error) -> &'static str {
    match err.io_error().map(io::Error::kind) {
        Some(io::ErrorKind::PermissionDenied) => "unreadable entry: permission denied",
        Some(io::ErrorKind::NotFound) => "unreadable entry: not found",
        Some(_) => "unreadable entry: I/O error",
        None => "invalid ignore file",
    }
}

pub(crate) fn warning(path: Option<String>, message: String) -> Diagnostic {
    Diagnostic {
        severity: Severity::Warning,
        message,
        detector: SCANNER_ID.to_string(),
        path,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_paths_sorts_and_dedups() {
        let ctx = RepoContext::from_paths("/r", ["b.txt", "a/z.txt", "a.txt", "b.txt"]);
        let paths: Vec<&str> = ctx.paths().collect();
        assert_eq!(paths, vec!["a.txt", "a/z.txt", "b.txt"]);
        assert!(ctx.diagnostics().is_empty());
    }

    #[test]
    fn read_returns_contents_and_memoizes() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("f.txt"), b"first").unwrap();
        let ctx = RepoContext::from_paths(dir.path(), ["f.txt"]);

        assert_eq!(ctx.read("f.txt").unwrap(), b"first");
        // Changing the file on disk must not change a memoized read.
        std::fs::write(dir.path().join("f.txt"), b"second").unwrap();
        assert_eq!(ctx.read("f.txt").unwrap(), b"first");
    }

    #[test]
    fn read_rejects_paths_outside_listing() {
        let ctx = RepoContext::from_paths("/r", ["a.txt"]);
        let err = ctx.read("missing.txt").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
    }
}
