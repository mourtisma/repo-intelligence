//! Fatal engine error: the only condition that aborts a scan. Entry-level
//! problems are diagnostics, never variants here (`architecture.md` §4.3).

use std::fmt;
use std::path::PathBuf;

/// The root path given to `scan` cannot be used as a repository.
#[derive(Debug)]
pub enum EngineError {
    /// The root path does not exist.
    RootNotFound(PathBuf),
    /// The root path exists but is not a directory.
    RootNotADirectory(PathBuf),
    /// The root path is a directory but cannot be read.
    RootUnreadable(PathBuf, std::io::Error),
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EngineError::RootNotFound(p) => write!(f, "root does not exist: {}", p.display()),
            EngineError::RootNotADirectory(p) => {
                write!(f, "root is not a directory: {}", p.display())
            }
            EngineError::RootUnreadable(p, e) => {
                write!(f, "root is unreadable: {} ({e})", p.display())
            }
        }
    }
}

impl std::error::Error for EngineError {}
