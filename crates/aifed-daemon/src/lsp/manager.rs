use crate::error::{Error, Result};
use crate::lsp::client::StdioLspClient;
use crate::lsp::{LanguageServerConfig, LspClient};
use aifed_common::ServerState;
use lsp_types::{
    ClientCapabilities, CompletionParams, CompletionResponse, DidChangeTextDocumentParams,
    DidCloseTextDocumentParams, DidOpenTextDocumentParams, DocumentDiagnosticParams,
    DocumentDiagnosticReportResult, GotoDefinitionParams, GotoDefinitionResponse, Hover,
    HoverParams, InitializeParams, Location, ReferenceParams, RenameParams, Url, WorkspaceEdit,
    WorkspaceFolder,
};
use serde::{Serialize, Serializer};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Key for identifying a language server instance
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct ServerKey {
    pub language: String,
    pub workspace: PathBuf,
}

/// Status of a language server
#[derive(Debug, Clone)]
pub struct ServerStatus {
    pub language: String,
    pub workspace: PathBuf,
    pub state: ServerState,
}

impl Serialize for ServerStatus {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("ServerStatus", 3)?;
        s.serialize_field("language", &self.language)?;
        s.serialize_field("workspace", &self.workspace.to_string_lossy())?;
        s.serialize_field("state", &self.state)?;
        s.end()
    }
}

/// Internal server entry
struct ServerEntry {
    client: Option<Box<dyn LspClient>>,
    status: ServerStatus,
}

/// Manager for language server instances
///
/// This manager handles the lifecycle of language servers and provides
/// proxy methods for LSP operations. Callers don't need to handle locking.
pub struct LanguageServerManager {
    servers: RwLock<HashMap<ServerKey, ServerEntry>>,
    configs: RwLock<HashMap<String, Arc<dyn LanguageServerConfig>>>,
}

impl Default for LanguageServerManager {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageServerManager {
    /// Create a new language server manager
    pub fn new() -> Self {
        Self { servers: RwLock::new(HashMap::new()), configs: RwLock::new(HashMap::new()) }
    }

    /// Register a language server configuration
    pub async fn register_config(&self, config: impl LanguageServerConfig + 'static) {
        let mut configs = self.configs.write().await;
        configs.insert(config.language_id().to_string(), Arc::new(config));
    }

    /// Start a language server for a workspace
    pub async fn start(&self, language: &str, workspace_root: PathBuf) -> Result<()> {
        let key = ServerKey { language: language.to_string(), workspace: workspace_root.clone() };

        // Check if already running and insert placeholder atomically to prevent race condition
        {
            let mut servers = self.servers.write().await;
            if let Some(entry) = servers.get(&key)
                && matches!(
                    entry.status.state,
                    ServerState::Running { .. } | ServerState::Starting { .. }
                )
            {
                return Ok(());
            }
            // Insert placeholder immediately while holding the lock
            servers.insert(
                key.clone(),
                ServerEntry {
                    client: None,
                    status: ServerStatus {
                        language: language.to_string(),
                        workspace: workspace_root.clone(),
                        state: ServerState::starting(),
                    },
                },
            );
        }

        // Get config
        let config = {
            let configs = self.configs.read().await;
            configs
                .get(language)
                .cloned()
                .ok_or_else(|| Error::ConfigNotFound { language: language.to_string() })?
        };

        // Spawn the server
        let mut command = config.spawn_command(&workspace_root);
        tracing::debug!(
            "Spawning LSP server: {} ({})",
            config.display_name(),
            language
        );
        let client = match StdioLspClient::spawn(&mut command, &workspace_root).await {
            Ok(c) => c,
            Err(e) => {
                let mut servers = self.servers.write().await;
                if let Some(entry) = servers.get_mut(&key) {
                    entry.status.state = ServerState::failed(e.to_string());
                }
                return Err(e);
            }
        };

        // Initialize the server
        let mut client: Box<dyn LspClient> = Box::new(client);
        let init_params = InitializeParams {
            capabilities: ClientCapabilities::default(),
            workspace_folders: Some(vec![WorkspaceFolder {
                uri: Url::from_directory_path(&workspace_root).map_err(|_| {
                    Error::InvalidRequest { reason: "Invalid workspace path".into() }
                })?,
                name: workspace_root
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "workspace".into()),
            }]),
            initialization_options: config.initialization_options(),
            ..Default::default()
        };

        match client.initialize(init_params).await {
            Ok(_) => {
                if let Err(e) = client.initialized().await {
                    let mut servers = self.servers.write().await;
                    if let Some(entry) = servers.get_mut(&key) {
                        entry.status.state = ServerState::failed(e.to_string());
                    }
                    return Err(Error::LspInitializationFailed { reason: e.to_string() });
                }
            }
            Err(e) => {
                let mut servers = self.servers.write().await;
                if let Some(entry) = servers.get_mut(&key) {
                    entry.status.state = ServerState::failed(e.to_string());
                }
                return Err(Error::LspInitializationFailed { reason: e.to_string() });
            }
        }

        // Update with running client
        {
            let mut servers = self.servers.write().await;
            servers.insert(
                key,
                ServerEntry {
                    client: Some(client),
                    status: ServerStatus {
                        language: language.to_string(),
                        workspace: workspace_root,
                        state: ServerState::running(),
                    },
                },
            );
        }

        Ok(())
    }

    /// Stop a language server
    ///
    /// If `force` is true, kill the process if it doesn't exit gracefully.
    /// If `force` is false, return an error if the process doesn't exit in time.
    /// After stopping, the server entry is kept with `Stopped` state for querying.
    pub async fn stop(&self, language: &str, workspace: &Path, force: bool) -> Result<()> {
        let key = ServerKey { language: language.to_string(), workspace: workspace.to_path_buf() };

        let mut servers = self.servers.write().await;
        if let Some(entry) = servers.get_mut(&key) {
            // Only shutdown if client exists (i.e., was fully initialized)
            if let Some(client) = &mut entry.client {
                match client.shutdown(force).await {
                    Ok(()) => {
                        entry.client = None;
                        entry.status.state = ServerState::stopped();
                    }
                    Err(e) => {
                        entry.status.state = ServerState::failed(e.to_string());
                        return Err(e);
                    }
                }
            } else {
                // No client, just update state
                entry.status.state = ServerState::stopped();
            }
        }

        Ok(())
    }

    /// List all servers with their status
    pub async fn list_servers(&self) -> Vec<ServerStatus> {
        let servers = self.servers.read().await;
        servers.values().map(|e| e.status.clone()).collect()
    }

    /// Get progress information for a specific server
    pub async fn get_server_progress(
        &self,
        language: &str,
        workspace: &Path,
    ) -> Vec<crate::lsp::progress::ProgressInfo> {
        let key = ServerKey { language: language.to_string(), workspace: workspace.to_path_buf() };
        let servers = self.servers.read().await;
        if let Some(entry) = servers.get(&key)
            && let Some(client) = &entry.client
        {
            return client.get_progress().await;
        }
        Vec::new()
    }

    // --- LSP proxy methods ---

    /// Go to definition
    pub async fn goto_definition(
        &self,
        language: &str,
        workspace: &Path,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let mut servers = self.servers.write().await;
        let entry = self.get_entry_mut(&mut servers, language, workspace)?;
        entry.client.as_mut().unwrap().goto_definition(params).await
    }

    /// Find references
    pub async fn references(
        &self,
        language: &str,
        workspace: &Path,
        params: ReferenceParams,
    ) -> Result<Option<Vec<Location>>> {
        let mut servers = self.servers.write().await;
        let entry = self.get_entry_mut(&mut servers, language, workspace)?;
        entry.client.as_mut().unwrap().references(params).await
    }

    /// Get hover information
    pub async fn hover(
        &self,
        language: &str,
        workspace: &Path,
        params: HoverParams,
    ) -> Result<Option<Hover>> {
        let mut servers = self.servers.write().await;
        let entry = self.get_entry_mut(&mut servers, language, workspace)?;
        entry.client.as_mut().unwrap().hover(params).await
    }

    /// Get completions
    pub async fn completion(
        &self,
        language: &str,
        workspace: &Path,
        params: CompletionParams,
    ) -> Result<Option<CompletionResponse>> {
        let mut servers = self.servers.write().await;
        let entry = self.get_entry_mut(&mut servers, language, workspace)?;
        entry.client.as_mut().unwrap().completion(params).await
    }

    /// Rename a symbol
    pub async fn rename(
        &self,
        language: &str,
        workspace: &Path,
        params: RenameParams,
    ) -> Result<Option<WorkspaceEdit>> {
        let mut servers = self.servers.write().await;
        let entry = self.get_entry_mut(&mut servers, language, workspace)?;
        entry.client.as_mut().unwrap().rename(params).await
    }

    /// Get diagnostics for a document
    pub async fn diagnostic(
        &self,
        language: &str,
        workspace: &Path,
        params: DocumentDiagnosticParams,
    ) -> Result<DocumentDiagnosticReportResult> {
        let mut servers = self.servers.write().await;
        let entry = self.get_entry_mut(&mut servers, language, workspace)?;
        entry.client.as_mut().unwrap().diagnostic(params).await
    }

    /// Notify the server of document open
    pub async fn did_open(
        &self,
        language: &str,
        workspace: &Path,
        params: DidOpenTextDocumentParams,
    ) -> Result<()> {
        let mut servers = self.servers.write().await;
        let entry = self.get_entry_mut(&mut servers, language, workspace)?;
        entry.client.as_mut().unwrap().did_open(params).await
    }

    /// Notify the server of document changes
    pub async fn did_change(
        &self,
        language: &str,
        workspace: &Path,
        params: DidChangeTextDocumentParams,
    ) -> Result<()> {
        let mut servers = self.servers.write().await;
        let entry = self.get_entry_mut(&mut servers, language, workspace)?;
        entry.client.as_mut().unwrap().did_change(params).await
    }

    /// Notify the server of document close
    pub async fn did_close(
        &self,
        language: &str,
        workspace: &Path,
        params: DidCloseTextDocumentParams,
    ) -> Result<()> {
        let mut servers = self.servers.write().await;
        let entry = self.get_entry_mut(&mut servers, language, workspace)?;
        entry.client.as_mut().unwrap().did_close(params).await
    }

    /// Helper to get a mutable entry for a running server
    fn get_entry_mut<'a>(
        &self,
        servers: &'a mut HashMap<ServerKey, ServerEntry>,
        language: &str,
        workspace: &Path,
    ) -> Result<&'a mut ServerEntry> {
        let key = ServerKey { language: language.to_string(), workspace: workspace.to_path_buf() };
        servers
            .get_mut(&key)
            .filter(|e| matches!(e.status.state, ServerState::Running { .. }) && e.client.is_some())
            .ok_or_else(|| Error::LspServerNotRunning { language: language.to_string() })
    }
}
