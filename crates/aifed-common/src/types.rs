//! API request and response types

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

// --- Generic API Response ---

/// Generic API response wrapper
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ApiError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self { success: true, data: Some(data), error: None }
    }

    pub fn error(code: &str, message: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(ApiError { code: code.into(), message: message.into() }),
        }
    }
}

impl ApiResponse<()> {
    pub fn ok() -> Self {
        Self { success: true, data: Some(()), error: None }
    }
}

// --- Daemon Status Types ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusResponse {
    pub workspace: String,
    pub uptime_secs: u64,
    pub servers: Vec<ServerStatusDto>,
}

/// Server status DTO (Data Transfer Object)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerStatusDto {
    pub language: String,
    pub workspace: String,
    pub state: ServerStateDto,
}

/// Server state DTO - simplified for API responses
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ServerStateDto {
    Starting {
        #[serde(with = "time::serde::rfc3339")]
        at: OffsetDateTime,
    },
    Running {
        #[serde(with = "time::serde::rfc3339")]
        at: OffsetDateTime,
    },
    Stopped {
        #[serde(with = "time::serde::rfc3339")]
        at: OffsetDateTime,
    },
    Failed {
        #[serde(with = "time::serde::rfc3339")]
        at: OffsetDateTime,
        reason: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    pub language: String,
    pub workspace: String,
    pub state: ServerStateDto,
    #[serde(default)]
    pub progress: Vec<ProgressInfoDto>,
}

/// Progress information from LSP server work done notifications
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressInfoDto {
    pub title: Option<String>,
    pub message: Option<String>,
    pub percentage: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServersResponse {
    pub servers: Vec<ServerInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartServerRequest {
    pub language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StopServerRequest {
    pub language: String,
    #[serde(default)]
    pub force: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerStatusResponse {
    pub language: String,
    pub workspace: String,
    pub state: String,
}

// --- LSP Operation Request Types ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspPositionRequest {
    pub language: String,
    pub file_path: String,
    pub position: Position,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HoverRequest {
    pub language: String,
    pub file_path: String,
    pub position: Position,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenameRequest {
    pub language: String,
    pub file_path: String,
    pub position: Position,
    pub new_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticsRequest {
    pub language: String,
    pub file_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DidOpenRequest {
    pub language: String,
    pub file_path: String,
    pub language_id: String,
    pub version: i32,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DidChangeRequest {
    pub language: String,
    pub file_path: String,
    pub version: i32,
    pub content_changes: Vec<ContentChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DidCloseRequest {
    pub language: String,
    pub file_path: String,
}

// --- Common Types ---

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentChange {
    pub range: Option<Range>,
    pub text: String,
}

// --- LSP Operation Response Types ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationResponse {
    pub file_path: String,
    pub range: Range,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HoverResponse {
    pub contents: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefinitionResponse {
    pub locations: Vec<LocationResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferencesResponse {
    pub locations: Vec<LocationResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionsResponse {
    pub items: Vec<CompletionItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionItem {
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documentation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticsResponse {
    pub diagnostics: Vec<DiagnosticItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticItem {
    pub range: Range,
    pub severity: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenameResponse {
    pub changes: Vec<FileEdit>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEdit {
    pub file_path: String,
    pub edits: Vec<TextEdit>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextEdit {
    pub range: Range,
    pub new_text: String,
}
