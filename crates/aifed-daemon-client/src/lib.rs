//! HTTP client for aifed-daemon
//!
//! This crate provides a client library for communicating with aifed-daemon
//! over Unix socket.

use std::path::{Path, PathBuf};

use aifed_common::*;
use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper::{Method, Request, Uri as HyperUri};
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use hyperlocal::{UnixConnector, Uri};
use serde::de::DeserializeOwned;

/// HTTP client for aifed-daemon
#[derive(Clone)]
pub struct DaemonClient {
    socket_path: PathBuf,
    client: Client<UnixConnector, Full<Bytes>>,
}

impl DaemonClient {
    /// Create a new client connected to the daemon at the given socket path
    pub fn new(socket_path: impl AsRef<Path>) -> Self {
        Self {
            socket_path: socket_path.as_ref().to_path_buf(),
            client: Client::builder(TokioExecutor::new()).build(UnixConnector),
        }
    }

    /// Create a client using the default socket path
    pub fn default_socket() -> Result<Self, ClientError> {
        let socket_path =
            dirs::runtime_dir().unwrap_or_else(std::env::temp_dir).join("aifed-daemon.sock");
        Ok(Self::new(socket_path))
    }

    fn uri(&self, path: &str) -> HyperUri {
        Uri::new(&self.socket_path, path).into()
    }

    /// Check if the daemon is running
    pub async fn is_running(&self) -> bool {
        self.health().await.is_ok()
    }

    /// Get daemon health status
    pub async fn health(&self) -> Result<HealthResponse, ClientError> {
        self.get("/api/v1/health").await
    }

    /// Send heartbeat to keep daemon alive
    pub async fn heartbeat(&self) -> Result<HealthResponse, ClientError> {
        self.post_empty("/api/v1/heartbeat").await
    }

    /// Get daemon status
    pub async fn status(&self) -> Result<StatusResponse, ClientError> {
        self.get("/api/v1/status").await
    }

    /// Shutdown the daemon
    pub async fn shutdown(&self) -> Result<HealthResponse, ClientError> {
        self.post_empty("/api/v1/shutdown").await
    }

    /// List all servers
    pub async fn list_servers(&self) -> Result<ServersResponse, ClientError> {
        self.get("/api/v1/lsp/servers").await
    }

    /// Start a language server
    pub async fn start_server(&self, language: &str) -> Result<ServerActionResponse, ClientError> {
        self.post(
            "/api/v1/lsp/servers/start",
            &StartServerRequest { language: language.to_string() },
        )
        .await
    }

    /// Stop a language server
    pub async fn stop_server(
        &self,
        language: &str,
        force: bool,
    ) -> Result<ServerActionResponse, ClientError> {
        self.post(
            "/api/v1/lsp/servers/stop",
            &StopServerRequest { language: language.to_string(), force },
        )
        .await
    }

    // --- LSP Operations ---

    /// Get hover information
    pub async fn hover(&self, request: HoverRequest) -> Result<HoverResponse, ClientError> {
        self.post("/api/v1/lsp/hover", &request).await
    }

    /// Go to definition
    pub async fn goto_definition(
        &self,
        request: LspPositionRequest,
    ) -> Result<DefinitionResponse, ClientError> {
        self.post("/api/v1/lsp/definition", &request).await
    }

    /// Find references
    pub async fn references(
        &self,
        request: LspPositionRequest,
    ) -> Result<ReferencesResponse, ClientError> {
        self.post("/api/v1/lsp/references", &request).await
    }

    /// Get completions
    pub async fn completions(
        &self,
        request: LspPositionRequest,
    ) -> Result<CompletionsResponse, ClientError> {
        self.post("/api/v1/lsp/completions", &request).await
    }

    /// Rename a symbol
    pub async fn rename(&self, request: RenameRequest) -> Result<RenameResponse, ClientError> {
        self.post("/api/v1/lsp/rename", &request).await
    }

    /// Get diagnostics
    pub async fn diagnostics(
        &self,
        request: DiagnosticsRequest,
    ) -> Result<DiagnosticsResponse, ClientError> {
        self.post("/api/v1/lsp/diagnostics", &request).await
    }

    /// Notify server of document open
    pub async fn did_open(&self, request: DidOpenRequest) -> Result<(), ClientError> {
        self.post("/api/v1/lsp/didOpen", &request).await
    }

    /// Notify server of document changes
    pub async fn did_change(&self, request: DidChangeRequest) -> Result<(), ClientError> {
        self.post("/api/v1/lsp/didChange", &request).await
    }

    /// Notify server of document close
    pub async fn did_close(&self, request: DidCloseRequest) -> Result<(), ClientError> {
        self.post("/api/v1/lsp/didClose", &request).await
    }

    // --- History Operations ---

    /// Record a file access (read operation)
    pub async fn record_access(&self, file: &str) -> Result<RecordAccessResponse, ClientError> {
        self.post("/api/v1/history/access", &RecordAccessRequest { file: file.to_string() }).await
    }

    /// Record an edit operation
    pub async fn record_edit(
        &self,
        file: &str,
        expected_hash: &str,
        new_hash: &str,
        diffs: Vec<LineDiffDto>,
    ) -> Result<(), ClientError> {
        self.post(
            "/api/v1/history/edit",
            &RecordEditRequest {
                file: file.to_string(),
                expected_hash: expected_hash.to_string(),
                new_hash: new_hash.to_string(),
                diffs,
            },
        )
        .await
    }

    /// Get history for a file
    pub async fn get_history(
        &self,
        file: &str,
        count: Option<usize>,
    ) -> Result<HistoryListResponse, ClientError> {
        let path = match count {
            Some(n) => format!("/api/v1/history/{}?count={}", Self::urlencoding_encode(file), n),
            None => format!("/api/v1/history/{}", Self::urlencoding_encode(file)),
        };
        self.get(&path).await
    }

    /// Undo the last edit for a file
    pub async fn undo(&self, file: &str, dry_run: bool) -> Result<UndoRedoResponse, ClientError> {
        let path = if dry_run {
            format!("/api/v1/history/{}/undo?dry_run=true", Self::urlencoding_encode(file))
        } else {
            format!("/api/v1/history/{}/undo", Self::urlencoding_encode(file))
        };
        self.post_empty(&path).await
    }

    /// Redo the last undone edit for a file
    pub async fn redo(&self, file: &str, dry_run: bool) -> Result<UndoRedoResponse, ClientError> {
        let path = if dry_run {
            format!("/api/v1/history/{}/redo?dry_run=true", Self::urlencoding_encode(file))
        } else {
            format!("/api/v1/history/{}/redo", Self::urlencoding_encode(file))
        };
        self.post_empty(&path).await
    }

    // --- HTTP Helpers ---

    /// URL-encode a path segment
    fn urlencoding_encode(s: &str) -> String {
        // Simple URL encoding for file paths
        s.replace('%', "%25").replace('/', "%2F").replace(' ', "%20").replace('+', "%2B")
    }

    /// Make a GET request
    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, ClientError> {
        let uri = self.uri(path);
        let req = Request::builder()
            .method(Method::GET)
            .uri(uri)
            .body(Full::new(Bytes::new()))
            .map_err(|e| ClientError::RequestFailed { message: e.to_string() })?;

        let resp = self
            .client
            .request(req)
            .await
            .map_err(|e| ClientError::ConnectionFailed { message: e.to_string() })?;

        self.parse_response(resp).await
    }

    /// Make a POST request
    async fn post<T: DeserializeOwned, B: serde::Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, ClientError> {
        let uri = self.uri(path);
        let json = serde_json::to_string(body)
            .map_err(|e| ClientError::SerializationError { message: e.to_string() })?;

        let req = Request::builder()
            .method(Method::POST)
            .uri(uri)
            .header("Content-Type", "application/json")
            .body(Full::new(Bytes::from(json)))
            .map_err(|e| ClientError::RequestFailed { message: e.to_string() })?;

        let resp = self
            .client
            .request(req)
            .await
            .map_err(|e| ClientError::ConnectionFailed { message: e.to_string() })?;

        self.parse_response(resp).await
    }

    /// Make a POST request without a body
    async fn post_empty<T: DeserializeOwned>(&self, path: &str) -> Result<T, ClientError> {
        let uri = self.uri(path);
        let req = Request::builder()
            .method(Method::POST)
            .uri(uri)
            .header("Content-Type", "application/json")
            .body(Full::new(Bytes::new()))
            .map_err(|e| ClientError::RequestFailed { message: e.to_string() })?;

        let resp = self
            .client
            .request(req)
            .await
            .map_err(|e| ClientError::ConnectionFailed { message: e.to_string() })?;

        self.parse_response(resp).await
    }

    /// Parse HTTP response
    async fn parse_response<T: DeserializeOwned>(
        &self,
        resp: hyper::Response<hyper::body::Incoming>,
    ) -> Result<T, ClientError> {
        let status = resp.status();

        let body_bytes = resp
            .collect()
            .await
            .map_err(|e| ClientError::RequestFailed { message: e.to_string() })?
            .to_bytes();

        // Try to parse as ApiResponse first (works for both success and error responses)
        let api_response: Result<ApiResponse<T>, _> = serde_json::from_slice(&body_bytes);

        if let Ok(api_response) = api_response {
            if api_response.success {
                return api_response.data.ok_or_else(|| ClientError::SerializationError {
                    message: "No data in response".to_string(),
                });
            } else {
                let error = api_response.error.unwrap_or_else(|| aifed_common::ApiError {
                    code: "UNKNOWN".to_string(),
                    message: "Unknown error".to_string(),
                });
                return Err(ClientError::ApiError { code: error.code, message: error.message });
            }
        }

        // If JSON parsing failed, return a generic HTTP error
        let error_text = String::from_utf8_lossy(&body_bytes);
        Err(ClientError::RequestFailed { message: format!("HTTP {}: {}", status, error_text) })
    }
}
