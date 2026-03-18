//! Client error types

use serde::{Deserialize, Serialize};

/// Client error type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientError {
    /// Connection failed
    ConnectionFailed { message: String },
    /// Request failed
    RequestFailed { message: String },
    /// API returned an error
    ApiError { code: String, message: String },
    /// Serialization/deserialization error
    SerializationError { message: String },
    /// Daemon not running
    DaemonNotRunning,
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConnectionFailed { message } => write!(f, "Connection failed: {}", message),
            Self::RequestFailed { message } => write!(f, "Request failed: {}", message),
            Self::ApiError { code, message } => write!(f, "API error ({}): {}", code, message),
            Self::SerializationError { message } => write!(f, "Serialization error: {}", message),
            Self::DaemonNotRunning => write!(f, "Daemon is not running"),
        }
    }
}

impl std::error::Error for ClientError {}
