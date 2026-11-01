//! Shared helpers for `core` integration tests.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

/// Path to a committed fixture under `fixtures/`.
pub fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name)
}

/// Builds a tree of `(relative path, contents)` files in a fresh temp directory.
/// Used for cases git cannot commit as fixtures (ignored files, empty dirs).
pub fn tree(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (path, contents) in files {
        let full = dir.path().join(path);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(full, contents).unwrap();
    }
    dir
}

/// The tree used to pin `.gitignore` handling.
pub fn gitignore_tree() -> tempfile::TempDir {
    tree(&[
        (".gitignore", "build/\n*.tmp\n"),
        (".ignore", "keep.txt\n"),
        (".hidden", "hidden\n"),
        (".git/config", "[core]\n"),
        (".git/info/exclude", "secret.txt\n"),
        ("secret.txt", "secret\n"),
        ("keep.txt", "keep\n"),
        ("debug.tmp", "tmp\n"),
        ("build/out.bin", "bin\n"),
        ("src/main.txt", "main\n"),
    ])
}
