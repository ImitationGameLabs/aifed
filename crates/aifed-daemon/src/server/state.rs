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
    /// Socket path
    pub socket_path: PathBuf,
    /// Log file path
    pub log_path: PathBuf,
}
