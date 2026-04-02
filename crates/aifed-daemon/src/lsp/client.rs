use async_trait::async_trait;
use lsp_types::*;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::{Child, Command};
use tokio::sync::{Mutex, oneshot};
use tokio::time::timeout;

use crate::error::{Error, Result};
use crate::lsp::progress::{ProgressInfo, ProgressTracker};
use crate::lsp::protocol::{RequestId, Response, ServerMessage, parse_content_length};

/// Trait for LSP client operations
#[async_trait]
pub trait LspClient: Send + Sync {
    /// Initialize the LSP connection
    async fn initialize(&mut self, params: InitializeParams) -> Result<InitializeResult>;

    /// Send initialized notification
    async fn initialized(&mut self) -> Result<()>;

    /// Send shutdown request
    ///
    /// If `force` is true, kill the process if it doesn't exit gracefully.
    /// If `force` is false, return an error if the process doesn't exit in time.
    async fn shutdown(&mut self, force: bool) -> Result<()>;

    /// Go to definition
    async fn goto_definition(
        &mut self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>>;

    /// Find references
    async fn references(&mut self, params: ReferenceParams) -> Result<Option<Vec<Location>>>;

    /// Get hover information
    async fn hover(&mut self, params: HoverParams) -> Result<Option<Hover>>;

    /// Get completions
    async fn completion(&mut self, params: CompletionParams) -> Result<Option<CompletionResponse>>;

    /// Rename a symbol
    async fn rename(&mut self, params: RenameParams) -> Result<Option<WorkspaceEdit>>;

    /// Get diagnostics for a document (pull diagnostics, LSP 3.16+)
    async fn diagnostic(
        &mut self,
        params: DocumentDiagnosticParams,
    ) -> Result<DocumentDiagnosticReportResult>;

    /// Notify the server of document open
    async fn did_open(&mut self, params: DidOpenTextDocumentParams) -> Result<()>;

    /// Notify the server of document changes
    async fn did_change(&mut self, params: DidChangeTextDocumentParams) -> Result<()>;

    /// Notify the server of document close
    async fn did_close(&mut self, params: DidCloseTextDocumentParams) -> Result<()>;

    /// Get active progress information from the server
    async fn get_progress(&self) -> Vec<ProgressInfo>;
}

/// Shared state between main thread and background task
struct Shared {
    /// Writer for sending messages to server
    writer: Box<dyn tokio::io::AsyncWrite + Unpin + Send>,
    /// Pending response channels, keyed by request ID
    pending_responses: HashMap<
        RequestId,
        oneshot::Sender<std::result::Result<Response<serde_json::Value>, Error>>,
    >,
}

/// Stdio-based LSP client implementation
///
/// This client uses a background task to handle incoming messages from the server.
/// Reader and writer are separated to avoid deadlocks:
/// - Background task owns the reader exclusively
/// - Main thread uses writer through shared Mutex
///
/// **Important**: Call `shutdown()` explicitly for graceful termination.
/// The Drop impl only aborts the background task; the child process relies on
/// `kill_on_drop(true)` for cleanup.
pub struct StdioLspClient {
    request_id: AtomicU32,
    initialized: AtomicBool,
    child: Mutex<Option<Child>>,
    default_timeout: Duration,
    /// Shared state (writer, pending responses)
    shared: Arc<Mutex<Shared>>,
    /// Progress tracker (separate Arc for async access)
    progress: Arc<ProgressTracker>,
    /// Background message loop task handle
    message_loop: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl StdioLspClient {
    /// Create a new LSP client by spawning a language server process
    pub async fn spawn(command: &mut Command, workspace_root: &Path) -> Result<Self> {
        command
            .current_dir(workspace_root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        let mut child = command.spawn().map_err(|e| Error::LspProcessSpawnFailed {
            command: format!("{:?}", command),
            source: e,
        })?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| Error::Transport("Failed to get stdin from child process".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| Error::Transport("Failed to get stdout from child process".into()))?;

        // Create separate reader and writer
        let reader = BufReader::new(stdout);
        let writer: Box<dyn tokio::io::AsyncWrite + Unpin + Send> = Box::new(BufWriter::new(stdin));

        // Create shared state and progress tracker
        let shared = Arc::new(Mutex::new(Shared {
            writer,
            pending_responses: HashMap::new(),
        }));
        let progress = Arc::new(ProgressTracker::new());

        // Clone for background task
        let shared_clone = shared.clone();
        let progress_clone = progress.clone();

        // Spawn background message loop (owns reader exclusively)
        let message_loop = tokio::spawn(async move {
            let mut reader = reader;
            let mut buffer = String::new();

            loop {
                // Read headers
                let mut content_length: Option<usize> = None;

                loop {
                    buffer.clear();
                    let bytes_read = match reader.read_line(&mut buffer).await {
                        Ok(n) => n,
                        Err(_) => break, // Error reading
                    };

                    if bytes_read == 0 {
                        // EOF - process exited, notify all pending requests
                        let mut shared = shared_clone.lock().await;
                        for (_, sender) in shared.pending_responses.drain() {
                            let _ = sender.send(Err(Error::LspProcessExited { code: None }));
                        }
                        return;
                    }

                    let line = buffer.trim();
                    if line.is_empty() {
                        break; // End of headers
                    }

                    if let Some(len) = parse_content_length(line) {
                        content_length = Some(len);
                    }
                }

                let content_length = match content_length {
                    Some(len) => len,
                    None => continue, // No content length, skip
                };

                // Read content
                let mut content = vec![0u8; content_length];
                if reader.read_exact(&mut content).await.is_err() {
                    return; // Error reading content
                }

                let json = match String::from_utf8(content) {
                    Ok(s) => s,
                    Err(_) => continue, // Invalid UTF-8
                };

                match ServerMessage::parse(&json) {
                    Ok(ServerMessage::Response(response)) => {
                        let mut shared = shared_clone.lock().await;
                        if let Some(sender) = shared.pending_responses.remove(&response.id) {
                            let _ = sender.send(Ok(response));
                        }
                    }
                    Ok(ServerMessage::Request(request)) => {
                        if request.method == "window/workDoneProgress/create"
                            && let Some(params) = &request.params
                            && let Ok(create_params) = serde_json::from_value::<
                                WorkDoneProgressCreateParams,
                            >(params.clone())
                        {
                            progress_clone.register_token(create_params.token).await;

                            // Send success response
                            let response_json = serde_json::json!({
                                "jsonrpc": "2.0",
                                "id": request.id,
                                "result": null
                            });
                            let response_str = serde_json::to_string(&response_json).unwrap();
                            let encoded = crate::lsp::protocol::encode_message(&response_str);
                            let mut shared = shared_clone.lock().await;
                            let _ = shared.writer.write_all(&encoded).await;
                            let _ = shared.writer.flush().await;
                        }
                    }
                    Ok(ServerMessage::Notification(notification)) => {
                        if notification.method == "$/progress"
                            && let Some(params) = notification.params
                            && let Ok(progress_params) =
                                serde_json::from_value::<ProgressParams>(params)
                        {
                            progress_clone.handle_progress(progress_params).await;
                        }
                    }
                    Err(_) => {
                        // Malformed message - ignore
                    }
                }
            }
        });

        Ok(Self {
            request_id: AtomicU32::new(1),
            initialized: AtomicBool::new(false),
            child: Mutex::new(Some(child)),
            default_timeout: Duration::from_secs(30),
            shared,
            progress,
            message_loop: Mutex::new(Some(message_loop)),
        })
    }

    /// Get the next request ID
    fn next_id(&self) -> u32 {
        self.request_id.fetch_add(1, Ordering::SeqCst)
    }

    /// Build a JSON-RPC request
    fn build_request<P: Serialize>(id: u32, method: &'static str, params: P) -> Result<String> {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        });
        serde_json::to_string(&request).map_err(Error::JsonSerialize)
    }

    /// Build a JSON-RPC notification
    fn build_notification<P: Serialize>(method: &'static str, params: P) -> Result<String> {
        let notification = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        });
        serde_json::to_string(&notification).map_err(Error::JsonSerialize)
    }

    /// Send raw message through writer
    async fn send_raw(&self, message: &str) -> Result<()> {
        let encoded = crate::lsp::protocol::encode_message(message);
        let mut shared = self.shared.lock().await;
        shared
            .writer
            .write_all(&encoded)
            .await
            .map_err(|e| Error::Transport(format!("Failed to write: {}", e)))?;
        shared
            .writer
            .flush()
            .await
            .map_err(|e| Error::Transport(format!("Failed to flush: {}", e)))?;
        Ok(())
    }

    /// Send a request and wait for response
    async fn send_request<R: DeserializeOwned>(
        &self,
        method: &'static str,
        params: impl Serialize,
    ) -> Result<R> {
        let id = self.next_id();
        let request_json = Self::build_request(id, method, params)?;
        let request_id = RequestId::Number(id as i64);

        // Create response channel and register pending request
        let (tx, rx) = oneshot::channel();
        {
            let mut shared = self.shared.lock().await;
            shared.pending_responses.insert(request_id.clone(), tx);
        }

        // Send request
        self.send_raw(&request_json).await?;

        // Wait for response with timeout
        let response = timeout(self.default_timeout, async {
            rx.await
                .map_err(|_| Error::Transport("Response channel closed".into()))?
        })
        .await
        .map_err(|_| Error::LspTimeout { timeout_ms: self.default_timeout.as_millis() as u64 })??;

        if let Some(error) = response.error {
            return Err(Error::JsonRpc { code: error.code, message: error.message });
        }

        let result = response.result;

        // Handle void responses (result is null or missing)
        if result.is_none() || result.as_ref().is_some_and(|v| v.is_null()) {
            return serde_json::from_value(serde_json::Value::Null).map_err(Error::JsonDeserialize);
        }

        let result = result.unwrap();
        serde_json::from_value(result).map_err(Error::JsonDeserialize)
    }

    /// Send a notification (no response expected)
    async fn send_notification<P: Serialize>(&self, method: &'static str, params: P) -> Result<()> {
        let notification_json = Self::build_notification(method, params)?;
        self.send_raw(&notification_json).await
    }
}

impl Drop for StdioLspClient {
    fn drop(&mut self) {
        // Abort the background message loop.
        // The child process has kill_on_drop(true) for automatic cleanup.
        // For graceful shutdown, the caller should explicitly call shutdown().
        if let Some(handle) = self.message_loop.get_mut().take() {
            handle.abort();
        }
    }
}

#[async_trait]
impl LspClient for StdioLspClient {
    async fn initialize(&mut self, params: InitializeParams) -> Result<InitializeResult> {
        let result = self
            .send_request::<InitializeResult>("initialize", params)
            .await?;
        self.initialized.store(true, Ordering::SeqCst);
        Ok(result)
    }

    async fn initialized(&mut self) -> Result<()> {
        self.send_notification("initialized", serde_json::json!({}))
            .await
    }

    async fn shutdown(&mut self, force: bool) -> Result<()> {
        self.send_request::<serde_json::Value>("shutdown", serde_json::json!(null))
            .await?;
        self.send_notification("exit", serde_json::json!({}))
            .await?;
        self.initialized.store(false, Ordering::SeqCst);

        // Stop the background message loop
        let mut message_loop = self.message_loop.lock().await;
        if let Some(handle) = message_loop.take() {
            handle.abort();
        }

        // Wait for child process to exit with timeout
        let mut child_guard = self.child.lock().await;
        if let Some(child) = child_guard.as_mut() {
            match timeout(self.default_timeout, child.wait()).await {
                Ok(_) => {} // Process exited gracefully
                Err(_) => {
                    // Timeout
                    if force {
                        // Force kill the process
                        child.kill().await.map_err(|e| {
                            Error::Transport(format!("Failed to kill process: {}", e))
                        })?;
                    } else {
                        // Return error - server is busy
                        return Err(Error::LspShutdownTimeout);
                    }
                }
            }
        }

        Ok(())
    }

    async fn goto_definition(
        &mut self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        self.send_request("textDocument/definition", params).await
    }

    async fn references(&mut self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        self.send_request("textDocument/references", params).await
    }

    async fn hover(&mut self, params: HoverParams) -> Result<Option<Hover>> {
        self.send_request("textDocument/hover", params).await
    }

    async fn completion(&mut self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        self.send_request("textDocument/completion", params).await
    }

    async fn rename(&mut self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        self.send_request("textDocument/rename", params).await
    }

    async fn diagnostic(
        &mut self,
        params: DocumentDiagnosticParams,
    ) -> Result<DocumentDiagnosticReportResult> {
        self.send_request("textDocument/diagnostic", params).await
    }

    async fn did_open(&mut self, params: DidOpenTextDocumentParams) -> Result<()> {
        self.send_notification("textDocument/didOpen", params).await
    }

    async fn did_change(&mut self, params: DidChangeTextDocumentParams) -> Result<()> {
        self.send_notification("textDocument/didChange", params)
            .await
    }

    async fn did_close(&mut self, params: DidCloseTextDocumentParams) -> Result<()> {
        self.send_notification("textDocument/didClose", params)
            .await
    }

    async fn get_progress(&self) -> Vec<ProgressInfo> {
        self.progress.get_active_progress().await
    }
}

/// Configuration trait for language-specific LSP servers
pub trait LanguageServerConfig: Send + Sync {
    /// Unique language identifier (e.g., "rust", "typescript")
    fn language_id(&self) -> &str;

    /// Command to spawn the language server
    fn spawn_command(&self, workspace_root: &Path) -> Command;

    /// Optional initialization options
    fn initialization_options(&self) -> Option<serde_json::Value> {
        None
    }

    /// Server name for display purposes (e.g., "rust-analyzer").
    ///
    /// Provides a more specific identifier than `language_id` for logging,
    /// error messages, and API responses.
    ///
    /// Default implementation returns `language_id()`.
    fn display_name(&self) -> &str {
        self.language_id()
    }
}
