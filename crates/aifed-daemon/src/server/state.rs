//! Shared state for the HTTP server

use crate::idle::IdleMonitor;
use crate::lsp::LanguageServerManager;
use std::path::PathBuf;
use std::sync::Arc;

/// Shared state for the daemon HTTP server.
#[derive(Clone)]
pub struct DaemonState {
    /// Workspace path (canonicalized)
    pub workspace: PathBuf,
    /// LSP manager for this workspace
    pub lsp_manager: Arc<LanguageServerManager>,
    /// Idle timeout monitor
    pub idle_monitor: Arc<IdleMonitor>,
}
