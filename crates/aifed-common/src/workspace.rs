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
/// Searches upward from the starting path, checking for both markers at each level.
/// This ensures the closest marker is found, preventing nested git repos from being
/// affected by external aifed.toml files.
///
/// Priority at the same level: `aifed.toml` > `.git`
///
/// Returns `Some(Workspace)` if found, `None` otherwise.
pub fn detect_workspace(from: &Path) -> Option<Workspace> {
    for dir in from.ancestors() {
        // Check aifed.toml first (higher priority at same level)
        if dir.join("aifed.toml").exists() {
            return Some(Workspace { root: dir.to_path_buf() });
        }
        // Then check .git at the same level
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
    fn test_aifed_toml_priority_over_git_at_same_level() {
        let dir = tempdir().unwrap();
        let subdir = dir.path().join("src");
        fs::create_dir_all(&subdir).unwrap();

        // Create both markers at the same level
        fs::File::create(dir.path().join("aifed.toml")).unwrap();
        fs::create_dir(dir.path().join(".git")).unwrap();

        // aifed.toml should win at the same level
        let ws = detect_workspace(&subdir).unwrap();
        assert_eq!(ws.root(), dir.path());
    }

    #[test]
    fn test_closest_marker_wins() {
        // Scenario: external aifed.toml should not affect nested git repo
        // /tmp/xxx/
        //   aifed.toml           <- external config
        //   projects/
        //     my-project/
        //       .git              <- nested git repo
        //       src/
        let dir = tempdir().unwrap();
        let nested_project = dir.path().join("projects").join("my-project");
        let src_dir = nested_project.join("src");
        fs::create_dir_all(&src_dir).unwrap();

        // Create external aifed.toml
        fs::File::create(dir.path().join("aifed.toml")).unwrap();

        // Create nested .git
        fs::create_dir(nested_project.join(".git")).unwrap();

        // Should find the closest marker (nested .git), not external aifed.toml
        let ws = detect_workspace(&src_dir).unwrap();
        assert_eq!(ws.root(), nested_project);
    }

    #[test]
    fn test_no_workspace() {
        let dir = tempdir().unwrap();
        let ws = detect_workspace(dir.path());
        assert!(ws.is_none());
    }
}
