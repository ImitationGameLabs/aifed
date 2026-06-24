//! Shared state for the HTTP server

use crate::history::HistoryManager;
use crate::idle::IdleMonitor;
use crate::lsp::LanguageServerManager;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

/// Shared state for the daemon HTTP server.
#[derive(Clone)]
pub struct DaemonState {
    /// Workspace path (canonicalized)
    pub workspace: PathBuf,
    /// LSP manager for this workspace
    pub lsp_manager: Arc<LanguageServerManager>,
    /// Idle timeout monitor
    pub idle_monitor: Arc<IdleMonitor>,
    /// History manager for undo/redo
    pub history_manager: Arc<HistoryManager>,
    /// Clipboard content (in-memory only, single entry)
    pub clipboard: Arc<RwLock<Option<String>>>,
    /// Daemon bind address (e.g. `127.0.0.1:54321`).
    pub address: String,
    /// SHA-256 of the bearer token clients must present. Only the hash is kept
    /// in memory (the plaintext lives in the endpoint file for the CLI); the
    /// middleware compares `SHA-256(received)` against this in constant time.
    pub token_hash: [u8; 32],
    /// Log file path
    pub log_path: PathBuf,
}
