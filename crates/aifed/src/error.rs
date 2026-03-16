use std::path::PathBuf;

#[derive(Debug)]
pub enum Error {
    FileNotFound {
        path: PathBuf,
    },
    HashMismatch {
        path: PathBuf,
        line: usize,
        expected: String,
        actual: String,
        actual_content: String,
    },
    InvalidLocator {
        input: String,
        reason: String,
    },
    InvalidOperation {
        input: String,
    },
    IoError {
        path: PathBuf,
        source: std::io::Error,
    },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::FileNotFound { path } => {
                write!(f, "File not found: {}", path.display())
            }
            Error::HashMismatch { path, line, expected, actual, actual_content } => {
                write!(
                    f,
                    "Hash mismatch\n  File: {}\n  Line: {}\n  Expected hash: {}\n  Actual hash: {}\n  Actual content: {}\n  Hint: Run 'aifed read {}' to get current hashes",
                    path.display(),
                    line,
                    expected,
                    actual,
                    actual_content,
                    path.display()
                )
            }
            Error::InvalidLocator { input, reason } => {
                write!(f, "Invalid locator '{}': {}", input, reason)
            }
            Error::InvalidOperation { input } => {
                write!(
                    f,
                    "Invalid operation '{}'. Expected one of: = (replace), + (insert), - (delete)",
                    input
                )
            }
            Error::IoError { path, source } => {
                write!(f, "IO error for '{}': {}", path.display(), source)
            }
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;
