//! Socket path generation

use crate::error::{Error, Result};
use std::path::{Path, PathBuf};
use xxhash_rust::xxh64::xxh64;

/// Generate a deterministic socket path for a workspace.
///
/// Format: `~/.cache/aifed/<name>-<hash16>.sock`
///
/// - `name`: sanitized workspace directory name (alphanumeric, dash, underscore)
/// - `hash16`: first 16 hex chars of xxh64 hash of canonical path
pub fn socket_path(workspace: &Path) -> Result<PathBuf> {
    let canonical = workspace
        .canonicalize()
        .map_err(|e| Error::Internal(format!("Failed to canonicalize workspace path: {}", e)))?;

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

    let socket_name = format!("{}-{}.sock", name, &hash_str[..16]);

    Ok(dirs::cache_dir()
        .ok_or_else(|| Error::Internal("Cannot determine cache directory".into()))?
        .join("aifed")
        .join(socket_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_socket_path_format() {
        // Create a temp directory to get a canonical path
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
}
