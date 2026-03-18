use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("File not found: {path}")]
    FileNotFound { path: PathBuf },

    #[error(
        "Hash mismatch\n  File: {path}\n  Line: {line}\n  Expected hash: {expected}\n  Actual hash: {actual}\n  Actual content: {actual_content}\n  Hint: Run 'aifed read {path}' to get current hashes"
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

    #[error("Batch parse error on line {line_number}: '{line_content}'\n  Reason: {reason}")]
    InvalidBatchOp { line_number: usize, line_content: String, reason: String },

    #[error("stdin not available for reading")]
    StdinNotAvailable,
}

pub type Result<T> = std::result::Result<T, Error>;
