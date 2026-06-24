//! Per-workspace runtime path derivation for aifed-daemon.
//!
//! Derives deterministic, collision-free paths for the daemon's per-workspace
//! runtime artifacts and defines the endpoint file the CLI reads to discover a
//! running daemon. All paths share a `<name>-<hash16>` stem derived from the
//! canonical workspace path, so the daemon and CLI always agree on a location
//! without exchanging it explicitly.

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use thiserror::Error;
use xxhash_rust::xxh64::xxh64;

/// Errors from per-workspace path derivation, endpoint file I/O, or (Unix)
/// permission restriction of the endpoint file.
#[derive(Debug, Error)]
pub enum PathError {
    #[error("Failed to canonicalize workspace path: {0}")]
    CanonicalizeError(std::io::Error),

    #[error("Cannot determine cache directory")]
    NoCacheDir,

    #[error("Cannot determine state directory")]
    NoStateDir,

    #[error("Endpoint file I/O error: {0}")]
    EndpointIo(std::io::Error),

    #[error("Endpoint file is corrupt: {0}")]
    EndpointCorrupt(serde_json::Error),

    #[error("Failed to restrict endpoint file permissions: {0}")]
    PermissionRestrictionFailed(std::io::Error),
}

/// Daemon contact info the CLI reads to reach a running per-workspace daemon.
///
/// Written atomically by the daemon at startup (see [`write_endpoint`]) and read
/// by the CLI to learn the daemon's loopback TCP port and bearer token. Treated
/// as a cache: the source of truth for liveness is a TCP `/health` probe that
/// presents `token` — a present-but-stale file (e.g. after a crash) is simply
/// overwritten once the probe fails.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonEndpoint {
    /// Canonical workspace root this daemon serves.
    pub workspace: String,
    /// OS process id of the daemon (diagnostics / `daemon status`).
    pub pid: u32,
    /// Loopback TCP port the daemon is listening on.
    pub port: u16,
    /// Random bearer token clients must present (`Authorization: Bearer <token>`).
    pub token: String,
}

impl DaemonEndpoint {
    /// The HTTP base URL to reach this daemon (`http://127.0.0.1:{port}`).
    ///
    /// Centralizes the loopback URL so the host/port format lives in one place.
    pub fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }
}

/// Generate the deterministic endpoint file path for a workspace.
///
/// Format: `~/.cache/aifed/<name>-<hash16>.endpoint.json`
///
/// Holds the daemon's port + token so the CLI can discover a running daemon
/// without a hardcoded port (TCP ports are not path-derivable).
pub fn endpoint_path(workspace: &Path) -> Result<PathBuf, PathError> {
    let base = base_name(workspace)?;
    Ok(dirs::cache_dir()
        .ok_or(PathError::NoCacheDir)?
        .join("aifed")
        .join(format!("{base}.endpoint.json")))
}

/// Generate the deterministic PID lock file path for a workspace.
///
/// Format: `~/.cache/aifed/<name>-<hash16>.lock`
///
/// Target of the advisory lock (std `File::try_lock`) held for the daemon's
/// entire lifetime to prevent two daemons for one workspace.
pub fn lock_path(workspace: &Path) -> Result<PathBuf, PathError> {
    let base = base_name(workspace)?;
    Ok(dirs::cache_dir()
        .ok_or(PathError::NoCacheDir)?
        .join("aifed")
        .join(format!("{base}.lock")))
}

/// Generate the deterministic log file path for a workspace.
///
/// Format: `~/.local/state/aifed/logs/<name>-<hash16>.log`
pub fn log_path(workspace: &Path) -> Result<PathBuf, PathError> {
    let base = base_name(workspace)?;
    Ok(dirs::state_dir()
        .ok_or(PathError::NoStateDir)?
        .join("aifed")
        .join("logs")
        .join(format!("{base}.log")))
}

/// Read and deserialize a [`DaemonEndpoint`] file.
///
/// Returns an error if the file is missing or corrupt; callers treat any error
/// as "no daemon running" and (re)spawn.
pub fn read_endpoint(path: &Path) -> Result<DaemonEndpoint, PathError> {
    let bytes = std::fs::read(path).map_err(PathError::EndpointIo)?;
    serde_json::from_slice(&bytes).map_err(PathError::EndpointCorrupt)
}

/// Atomically write a [`DaemonEndpoint`] file with restrictive permissions.
///
/// Writes to a sibling temp file, restricts it to `0600` on Unix, then renames
/// it into place — so readers never observe a partially-written file, and on
/// Unix the token is owner-private from the moment it appears.
///
/// Returns `Err` if the file can't be written or (Unix) its permissions can't
/// be restricted — the endpoint holds a bearer token, so a failure to secure it
/// is treated as fatal rather than publishing a possibly world-readable secret.
/// On non-Unix there is no Unix permission model to apply; `Ok` there means the
/// file was written and its confidentiality is delegated to the private user
/// directory, not that this function hardened anything.
pub fn write_endpoint(path: &Path, endpoint: &DaemonEndpoint) -> Result<(), PathError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(PathError::EndpointIo)?;
    }

    let json = serde_json::to_vec_pretty(endpoint).map_err(PathError::EndpointCorrupt)?;

    let temp_path = path.with_file_name(format!(
        "{}.tmp",
        path.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    ));

    {
        let mut file = std::fs::File::create(&temp_path).map_err(PathError::EndpointIo)?;
        // Restrict the temp file's permissions BEFORE writing the token and
        // renaming — order matters, so the published file is never briefly
        // world-readable between rename and a later chmod.
        restrict_permissions(&file)?;
        file.write_all(&json).map_err(PathError::EndpointIo)?;
        file.sync_all().map_err(PathError::EndpointIo)?;
    }

    std::fs::rename(&temp_path, path).map_err(PathError::EndpointIo)?;
    Ok(())
}

/// Restrict a file to owner-only on Unix; a genuine no-op elsewhere.
///
/// On Unix, failure is fatal (returned as [`PathError::PermissionRestrictionFailed`])
/// because the endpoint file holds a bearer token. On non-Unix there is no Unix
/// permission model; confidentiality relies on the private user directory.
#[cfg(unix)]
fn restrict_permissions(file: &std::fs::File) -> Result<(), PathError> {
    use std::os::unix::fs::PermissionsExt;
    file.set_permissions(std::fs::Permissions::from_mode(0o600))
        .map_err(PathError::PermissionRestrictionFailed)
}

#[cfg(not(unix))]
fn restrict_permissions(_file: &std::fs::File) -> Result<(), PathError> {
    Ok(())
}

/// Generate base name for workspace: `<name>-<hash16>`
fn base_name(workspace: &Path) -> Result<String, PathError> {
    let canonical = workspace
        .canonicalize()
        .map_err(PathError::CanonicalizeError)?;

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
    fn test_endpoint_path_format() {
        let temp = tempfile::tempdir().unwrap();
        let result = endpoint_path(temp.path()).unwrap();

        // Should be under cache dir
        assert!(result.to_string_lossy().contains("aifed"));

        // Should end with .endpoint.json
        assert!(result.to_string_lossy().ends_with(".endpoint.json"));

        // Should contain a dash separator
        let name = result.file_stem().unwrap().to_string_lossy();
        assert!(name.contains('-'));
    }

    #[test]
    fn test_deterministic_hash() {
        let temp = tempfile::tempdir().unwrap();
        let result1 = endpoint_path(temp.path()).unwrap();
        let result2 = endpoint_path(temp.path()).unwrap();
        assert_eq!(result1, result2);
    }

    #[test]
    fn test_lock_path_format() {
        let temp = tempfile::tempdir().unwrap();
        let result = lock_path(temp.path()).unwrap();

        assert!(result.to_string_lossy().contains("aifed"));
        assert!(result.extension().map(|e| e == "lock").unwrap_or(false));
    }

    #[test]
    fn test_log_path_format() {
        let temp = tempfile::tempdir().unwrap();
        let result = log_path(temp.path()).unwrap();

        assert!(result.to_string_lossy().contains("logs"));
        assert!(result.extension().map(|e| e == "log").unwrap_or(false));
    }

    #[test]
    fn test_paths_share_base_name() {
        let temp = tempfile::tempdir().unwrap();
        let endpoint = endpoint_path(temp.path()).unwrap();
        let lock = lock_path(temp.path()).unwrap();
        let log = log_path(temp.path()).unwrap();

        // Strip extensions to compare stems.
        let endpoint_stem = strip_extensions(&endpoint);
        let lock_stem = lock.file_stem().unwrap().to_string_lossy();
        let log_stem = log.file_stem().unwrap().to_string_lossy();

        assert_eq!(endpoint_stem, lock_stem);
        assert_eq!(endpoint_stem, log_stem);
    }

    /// `<base>.endpoint.json` → `<base>` (drop both trailing extensions).
    fn strip_extensions(path: &Path) -> String {
        let file_name = path.file_name().unwrap().to_string_lossy().into_owned();
        file_name
            .strip_suffix(".endpoint.json")
            .map(|s| s.to_string())
            .unwrap_or(file_name)
    }

    #[test]
    fn test_endpoint_round_trip() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("test.endpoint.json");

        let endpoint = DaemonEndpoint {
            workspace: "/tmp/ws".to_string(),
            pid: 12345,
            port: 54321,
            token: "deadbeef".to_string(),
        };

        write_endpoint(&path, &endpoint).unwrap();
        let read_back = read_endpoint(&path).unwrap();
        assert_eq!(read_back.workspace, endpoint.workspace);
        assert_eq!(read_back.pid, endpoint.pid);
        assert_eq!(read_back.port, endpoint.port);
        assert_eq!(read_back.token, endpoint.token);
    }

    /// On Unix the endpoint file (which holds a bearer token) must be owner-only.
    #[cfg(unix)]
    #[test]
    fn test_endpoint_file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("perms.endpoint.json");
        let endpoint = DaemonEndpoint {
            workspace: "/tmp/ws".to_string(),
            pid: 1,
            port: 1,
            token: "secret".to_string(),
        };

        write_endpoint(&path, &endpoint).unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "endpoint file must be 0600 on unix");
    }

    #[test]
    fn test_read_endpoint_missing_is_error() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("missing.endpoint.json");
        assert!(read_endpoint(&path).is_err());
    }
}
