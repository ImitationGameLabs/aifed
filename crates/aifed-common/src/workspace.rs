//! Workspace detection for aifed
//!
//! Provides intelligent workspace detection by searching for `aifed.toml` or `.git`
//! markers starting from the current directory and walking up the tree.

use std::path::{Path, PathBuf};
use thiserror::Error;

/// Workspace detection errors
#[derive(Debug, Error)]
pub enum WorkspaceError {
    #[error("No cache directory")]
    NoCacheDir,

    #[error("Invalid path: {0}")]
    InvalidPath(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Workspace information
#[derive(Debug, Clone)]
pub struct Workspace {
    root: PathBuf,
}

impl Workspace {
    /// Get the workspace root directory
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Get the daemon socket path for this workspace
    pub fn socket_path(&self) -> Result<PathBuf, WorkspaceError> {
        crate::socket_path(&self.root).map_err(|e| match e {
            crate::SocketError::NoCacheDir => WorkspaceError::NoCacheDir,
            crate::SocketError::CanonicalizeError(io) => WorkspaceError::Io(io),
            crate::SocketError::NoStateDir => WorkspaceError::NoCacheDir,
        })
    }
}

/// Detect workspace from a starting path.
///
/// Detection priority:
/// 1. Search upward for `aifed.toml`
/// 2. Search upward for `.git`
///
/// Returns `Some(Workspace)` if found, `None` otherwise.
pub fn detect_workspace(from: &Path) -> Option<Workspace> {
    // 1. Search upward for aifed.toml
    for dir in from.ancestors() {
        if dir.join("aifed.toml").exists() {
            return Some(Workspace { root: dir.to_path_buf() });
        }
    }

    // 2. Search upward for .git
    for dir in from.ancestors() {
        if dir.join(".git").exists() {
            return Some(Workspace { root: dir.to_path_buf() });
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_detect_aifed_toml() {
        let dir = tempdir().unwrap();
        let subdir = dir.path().join("src").join("components");
        fs::create_dir_all(&subdir).unwrap();

        // Create aifed.toml in root
        fs::File::create(dir.path().join("aifed.toml")).unwrap();

        // Detect from subdirectory
        let ws = detect_workspace(&subdir);
        assert!(ws.is_some());
        assert_eq!(ws.unwrap().root(), dir.path());
    }

    #[test]
    fn test_detect_git() {
        let dir = tempdir().unwrap();
        let subdir = dir.path().join("crates").join("my-crate");
        fs::create_dir_all(&subdir).unwrap();

        // Create .git in root
        fs::create_dir(dir.path().join(".git")).unwrap();

        // Detect from subdirectory
        let ws = detect_workspace(&subdir);
        assert!(ws.is_some());
        assert_eq!(ws.unwrap().root(), dir.path());
    }

    #[test]
    fn test_aifed_toml_priority_over_git() {
        let dir = tempdir().unwrap();
        let subdir = dir.path().join("src");
        fs::create_dir_all(&subdir).unwrap();

        // Create both markers
        fs::File::create(dir.path().join("aifed.toml")).unwrap();
        fs::create_dir(dir.path().join(".git")).unwrap();

        // aifed.toml should win
        let ws = detect_workspace(&subdir).unwrap();
        assert_eq!(ws.root(), dir.path());
    }

    #[test]
    fn test_no_workspace() {
        let dir = tempdir().unwrap();
        let ws = detect_workspace(dir.path());
        assert!(ws.is_none());
    }
}
