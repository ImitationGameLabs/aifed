//! API request and response types

use crate::lsp::ServerState;
use serde::{Deserialize, Serialize};

// --- Error Codes ---

/// API error codes
#[derive(Debug, Clone, Copy, Serialize)]
pub enum ErrorCode {
    InvalidPath,
    LspStartFailed,
    LspStopFailed,
    LspServerBusy,
    LspError,
}

impl ErrorCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::InvalidPath => "INVALID_PATH",
            Self::LspStartFailed => "LSP_START_FAILED",
            Self::LspStopFailed => "LSP_STOP_FAILED",
            Self::LspServerBusy => "LSP_SERVER_BUSY",
            Self::LspError => "LSP_ERROR",
        }
    }
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// --- Generic API Response ---

/// Generic API response wrapper
#[derive(Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ApiError>,
}

#[derive(Serialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self { success: true, data: Some(data), error: None }
    }

    pub fn error(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(ApiError { code: code.to_string(), message: message.into() }),
        }
    }
}

impl ApiResponse<()> {
    pub fn ok() -> Self {
        Self { success: true, data: Some(()), error: None }
    }
}

// --- Daemon Status Types ---

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
}

#[derive(Serialize)]
pub struct StatusResponse {
    pub workspace: String,
    pub uptime_secs: u64,
    pub bin_path: String,
    pub socket_path: String,
    pub log_path: String,
    pub servers: Vec<crate::lsp::ServerStatus>,
}

/// Progress information from LSP server work done notifications
#[derive(Serialize)]
pub struct ProgressInfo {
    pub title: Option<String>,
    pub message: Option<String>,
    pub percentage: Option<u32>,
}

#[derive(Serialize)]
pub struct ServerInfo {
    pub language: String,
    pub workspace: String,
    pub state: ServerState,
    #[serde(default)]
    pub progress: Vec<ProgressInfo>,
}

#[derive(Serialize)]
pub struct ServersResponse {
    pub servers: Vec<ServerInfo>,
}

#[derive(Deserialize)]
pub struct StartServerRequest {
    pub language: String,
}

#[derive(Deserialize)]
pub struct StopServerRequest {
    pub language: String,
    #[serde(default)]
    pub force: bool,
}

// --- LSP Operation Request Types ---

#[derive(Deserialize)]
pub struct LspPositionRequest {
    pub language: String,
    pub file_path: String,
    pub position: Position,
}

#[derive(Deserialize)]
pub struct HoverRequest {
    pub language: String,
    pub file_path: String,
    pub position: Position,
}

#[derive(Deserialize)]
pub struct RenameRequest {
    pub language: String,
    pub file_path: String,
    pub position: Position,
    pub new_name: String,
}

#[derive(Deserialize)]
pub struct DiagnosticsRequest {
    pub language: String,
    pub file_path: String,
}

#[derive(Deserialize)]
pub struct DidOpenRequest {
    pub language: String,
    pub file_path: String,
    pub language_id: String,
    pub version: i32,
    pub text: String,
}

#[derive(Deserialize)]
pub struct DidChangeRequest {
    pub language: String,
    pub file_path: String,
    pub version: i32,
    pub content_changes: Vec<ContentChange>,
}

#[derive(Deserialize)]
pub struct DidCloseRequest {
    pub language: String,
    pub file_path: String,
}

// --- Common Types ---

#[derive(Deserialize, Serialize)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

#[derive(Deserialize, Serialize)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

#[derive(Deserialize)]
pub struct ContentChange {
    pub range: Option<Range>,
    pub text: String,
}

// --- LSP Operation Response Types ---

#[derive(Serialize)]
pub struct LocationResponse {
    pub file_path: String,
    pub range: Range,
}

#[derive(Serialize)]
pub struct HoverResponse {
    pub contents: Option<String>,
}

#[derive(Serialize)]
pub struct DefinitionResponse {
    pub locations: Vec<LocationResponse>,
}

#[derive(Serialize)]
pub struct ReferencesResponse {
    pub locations: Vec<LocationResponse>,
}

#[derive(Serialize)]
pub struct CompletionsResponse {
    pub items: Vec<CompletionItem>,
}

#[derive(Serialize)]
pub struct CompletionItem {
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documentation: Option<String>,
}

#[derive(Serialize)]
pub struct DiagnosticsResponse {
    pub diagnostics: Vec<DiagnosticItem>,
}

#[derive(Serialize)]
pub struct DiagnosticItem {
    pub range: Range,
    pub severity: String,
    pub message: String,
}

#[derive(Serialize)]
pub struct RenameResponse {
    pub changes: Vec<FileEdit>,
}

#[derive(Serialize)]
pub struct FileEdit {
    pub file_path: String,
    pub edits: Vec<TextEdit>,
}

#[derive(Serialize)]
pub struct TextEdit {
    pub range: Range,
    pub new_text: String,
}
