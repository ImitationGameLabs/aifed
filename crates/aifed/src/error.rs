use std::path::PathBuf;
use thiserror::Error;

use aifed_common::Position;

#[derive(Debug, Error)]
pub enum Error {
    #[error("File not found: {path}")]
    FileNotFound { path: PathBuf },

    #[error(
        "Hash mismatch\n\
         \n\
           File: {path}:{line}\n\
         Expected: {expected}\n\
           Actual: {actual}\n\
         \n\
         Actual content: \"{actual_content}\"\n\
         \n\
         The file may have been modified since you last read it.\n\
         Hint: Run 'aifed read {path}' to get current hashes"
    )]
    HashMismatch {
        path: PathBuf,
        line: usize,
        expected: String,
        actual: String,
        actual_content: String,
    },

    #[error("Invalid locator '{input}': {reason}")]
    InvalidLocator { input: String, reason: String },

    #[error("Invalid operation '{input}'. Expected one of: = (replace), + (insert), - (delete)")]
    InvalidOperation { input: String },

    #[error("IO error for '{path}': {source}")]
    InvalidIo { path: PathBuf, source: std::io::Error },

    #[error("Invalid UTF-8 encoding in '{path}': {source}")]
    InvalidEncoding { path: PathBuf, source: std::string::FromUtf8Error },

    #[error("editing non-text files is not supported: {path}")]
    BinaryFile { path: PathBuf },

    #[error("Batch parse error on line {line_number}: '{line_content}'\n  Reason: {reason}")]
    InvalidBatchOp { line_number: usize, line_content: String, reason: String },

    #[error("stdin not available for reading")]
    StdinNotAvailable,

    #[error("Lightweight mode: workspace not detected (no aifed.toml or .git found)")]
    LightweightMode,

    // Daemon-related errors
    #[error("Daemon is not running for workspace: {workspace}")]
    DaemonNotRunning { workspace: PathBuf },

    #[error("LSP error: {message}")]
    Lsp { message: String },

    #[error("Socket path error: {0}")]
    SocketError(#[from] aifed_common::SocketError),

    #[error("Workspace error: {0}")]
    WorkspaceError(#[from] aifed_common::WorkspaceError),

    #[error("Client error: {0}")]
    ClientError(#[from] aifed_common::ClientError),

    #[error("Invalid range {start:?}-{end:?}: {reason}")]
    InvalidRange { start: Position, end: Position, reason: String },

    #[error("Conflict: line {0} cannot be both deleted and replaced")]
    ConflictDeleteAndReplace(usize),

    #[error("Invalid escape sequence in '{sequence}': {reason}")]
    InvalidEscape { sequence: String, reason: String },

    #[error("Unterminated string literal")]
    UnterminatedString,

    #[error(
        "File hash mismatch during undo/redo\n\
         \n\
           File: {path}\n\
         Expected: {expected}\n\
           Actual: {actual}\n\
         \n\
         The file may have been modified externally since the last aifed operation.\n\
         Hint: Run 'aifed read {path}' to get current state"
    )]
    FileHashMismatch { path: PathBuf, expected: String, actual: String },
}

pub type Result<T> = std::result::Result<T, Error>;
