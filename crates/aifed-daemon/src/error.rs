use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("LSP server not running for language: {language}")]
    LspServerNotRunning { language: String },

    #[error("LSP request timed out after {timeout_ms}ms")]
    LspTimeout { timeout_ms: u64 },

    #[error("LSP server is busy, shutdown timed out (use force=true to kill)")]
    LspShutdownTimeout,

    #[error("LSP initialization failed: {reason}")]
    LspInitializationFailed { reason: String },

    #[error("LSP process spawn failed for '{command}': {source}")]
    LspProcessSpawnFailed {
        command: String,
        #[source]
        source: std::io::Error,
    },

    #[error("LSP process exited unexpectedly with code: {code:?}")]
    LspProcessExited { code: Option<i32> },

    #[error("JSON-RPC error: code={code}, message={message}")]
    JsonRpc { code: i32, message: String },

    #[error("Transport error: {0}")]
    Transport(String),

    #[error("JSON serialization error: {0}")]
    JsonSerialize(#[source] serde_json::Error),

    #[error("JSON deserialization error: {0}")]
    JsonDeserialize(#[source] serde_json::Error),

    #[error("Invalid request: {reason}")]
    InvalidRequest { reason: String },

    #[error("Language server config not found: {language}")]
    ConfigNotFound { language: String },
}

pub type Result<T> = std::result::Result<T, Error>;
