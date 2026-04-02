//! Socket path generation
//!
//! Provides deterministic socket path generation for aifed-daemon.
//! Both the daemon and CLI use this to agree on socket location.

use std::path::{Path, PathBuf};
use thiserror::Error;
use xxhash_rust::xxh64::xxh64;

/// Socket path generation errors
#[derive(Debug, Error)]
pub enum SocketError {
    #[error("Failed to canonicalize workspace path: {0}")]
    CanonicalizeError(std::io::Error),

    #[error("Cannot determine cache directory")]
    NoCacheDir,

    #[error("Cannot determine state directory")]
    NoStateDir,
}

/// Generate a deterministic socket path for a workspace.
///
/// Format: `~/.cache/aifed/<name>-<hash16>.sock`
///
/// - `name`: sanitized workspace directory name (alphanumeric, dash, underscore)
/// - `hash16`: first 16 hex chars of xxh64 hash of canonical path
pub fn socket_path(workspace: &Path) -> Result<PathBuf, SocketError> {
    let base = base_name(workspace)?;
    let socket_name = format!("{}.sock", base);
    Ok(dirs::cache_dir()
        .ok_or(SocketError::NoCacheDir)?
        .join("aifed")
        .join(socket_name))
}

/// Generate a deterministic PID lock file path for a workspace.
///
/// Format: `~/.cache/aifed/<name>-<hash16>.lock`
///
/// Used to prevent multiple daemons from starting for the same workspace.
pub fn lock_path(workspace: &Path) -> Result<PathBuf, SocketError> {
    let base = base_name(workspace)?;
    let lock_name = format!("{}.lock", base);
    Ok(dirs::cache_dir()
        .ok_or(SocketError::NoCacheDir)?
        .join("aifed")
        .join(lock_name))
}

/// Generate a deterministic log file path for a workspace.
///
/// Format: `~/.local/state/aifed/logs/<name>-<hash16>.log`
pub fn log_path(workspace: &Path) -> Result<PathBuf, SocketError> {
    let base = base_name(workspace)?;
    let log_name = format!("{}.log", base);
    Ok(dirs::state_dir()
        .ok_or(SocketError::NoStateDir)?
        .join("aifed")
        .join("logs")
        .join(log_name))
}

/// Generate base name for workspace: `<name>-<hash16>`
fn base_name(workspace: &Path) -> Result<String, SocketError> {
    let canonical = workspace
        .canonicalize()
        .map_err(SocketError::CanonicalizeError)?;

    // Extract and sanitize directory name
    let name: String = canonical
        .file_name()
        .map(|n| n.to_string_lossy())
        .unwrap_or_default()
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
        .take(32)
        .collect();

    // Generate hash of canonical path
    let path_str = canonical.to_string_lossy();
    let hash = xxh64(path_str.as_bytes(), 0);
    let hash_str = format!("{:016x}", hash);

    Ok(format!("{}-{}", name, &hash_str[..16]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_socket_path_format() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path();
        let result = socket_path(path).unwrap();

        // Should be under cache dir
        assert!(result.to_string_lossy().contains("aifed"));

        // Should end with .sock
        assert!(result.extension().map(|e| e == "sock").unwrap_or(false));

        // Should contain a dash separator
        let name = result.file_stem().unwrap().to_string_lossy();
        assert!(name.contains('-'));
    }

    #[test]
    fn test_deterministic_hash() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path();
        let result1 = socket_path(path).unwrap();
        let result2 = socket_path(path).unwrap();
        assert_eq!(result1, result2);
    }

    #[test]
    fn test_lock_path_format() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path();
        let result = lock_path(path).unwrap();

        // Should be under cache dir
        assert!(result.to_string_lossy().contains("aifed"));

        // Should end with .lock
        assert!(result.extension().map(|e| e == "lock").unwrap_or(false));
    }

    #[test]
    fn test_log_path_format() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path();
        let result = log_path(path).unwrap();

        // Should contain logs directory
        assert!(result.to_string_lossy().contains("logs"));

        // Should end with .log
        assert!(result.extension().map(|e| e == "log").unwrap_or(false));
    }

    #[test]
    fn test_paths_share_base_name() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path();

        let socket = socket_path(path).unwrap();
        let lock = lock_path(path).unwrap();
        let log = log_path(path).unwrap();

        // All should have same base name (before extension)
        let socket_stem = socket.file_stem().unwrap().to_string_lossy();
        let lock_stem = lock.file_stem().unwrap().to_string_lossy();
        let log_stem = log.file_stem().unwrap().to_string_lossy();

        assert_eq!(socket_stem, lock_stem);
        assert_eq!(socket_stem, log_stem);
    }
}
