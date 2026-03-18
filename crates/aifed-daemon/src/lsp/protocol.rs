#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// Request ID type (can be number or string)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RequestId {
    Number(i64),
    String(String),
}

/// JSON-RPC 2.0 response wrapper
#[derive(Debug, Deserialize)]
pub struct Response<T> {
    pub jsonrpc: String,
    pub id: RequestId,
    #[serde(default)]
    pub result: Option<T>,
    #[serde(default)]
    pub error: Option<RpcError>,
}

/// JSON-RPC error object
#[derive(Debug, Deserialize, Clone)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
    #[serde(default)]
    pub data: Option<serde_json::Value>,
}

impl RpcError {
    /// Parse error
    pub const PARSE_ERROR: i32 = -32700;
    /// Invalid request
    pub const INVALID_REQUEST: i32 = -32600;
    /// Method not found
    pub const METHOD_NOT_FOUND: i32 = -32601;
    /// Invalid params
    pub const INVALID_PARAMS: i32 = -32602;
    /// Internal error
    pub const INTERNAL_ERROR: i32 = -32603;
}

/// JSON-RPC request from server (has id and method)
#[derive(Debug, Deserialize)]
pub struct IncomingRequest {
    pub jsonrpc: String,
    pub id: RequestId,
    pub method: String,
    #[serde(default)]
    pub params: Option<serde_json::Value>,
}

/// JSON-RPC notification from server (no id)
#[derive(Debug, Deserialize)]
pub struct IncomingNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(default)]
    pub params: Option<serde_json::Value>,
}

/// Server message types (parsed from JSON)
#[derive(Debug)]
pub enum ServerMessage {
    /// Response to a client request
    Response(Response<serde_json::Value>),
    /// Request from server (e.g., window/workDoneProgress/create)
    Request(IncomingRequest),
    /// Notification from server (e.g., $/progress)
    Notification(IncomingNotification),
}

impl ServerMessage {
    /// Parse a JSON string into a server message
    pub fn parse(json: &str) -> Result<Self, serde_json::Error> {
        let value: serde_json::Value = serde_json::from_str(json)?;
        if value.get("id").is_some() {
            if value.get("method").is_some() {
                // Has id and method → Request from server
                Ok(ServerMessage::Request(serde_json::from_value(value)?))
            } else {
                // Has id but no method → Response to client request
                Ok(ServerMessage::Response(serde_json::from_value(value)?))
            }
        } else {
            // No id → Notification from server
            Ok(ServerMessage::Notification(serde_json::from_value(value)?))
        }
    }
}

/// Encode a message with Content-Length header for LSP transport
pub fn encode_message(content: &str) -> Vec<u8> {
    format!("Content-Length: {}\r\n\r\n{}", content.len(), content).into_bytes()
}

/// Parse Content-Length header value
pub fn parse_content_length(header: &str) -> Option<usize> {
    header
        .strip_prefix("Content-Length:")
        .or_else(|| header.strip_prefix("Content-Length :"))
        .and_then(|s| s.trim().parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_message() {
        let content = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#;
        let encoded = encode_message(content);
        let expected = format!("Content-Length: {}\r\n\r\n{}", content.len(), content);
        assert_eq!(encoded, expected.as_bytes());
    }

    #[test]
    fn test_parse_content_length() {
        assert_eq!(parse_content_length("Content-Length: 42"), Some(42));
        assert_eq!(parse_content_length("Content-Length:42"), Some(42));
        assert_eq!(parse_content_length("Content-Length : 42"), Some(42));
        assert_eq!(parse_content_length("Invalid"), None);
    }

    #[test]
    fn test_response_deserialization() {
        let json = r#"{"jsonrpc":"2.0","id":1,"result":{"capabilities":{}}}"#;
        let resp: Response<serde_json::Value> = serde_json::from_str(json).unwrap();
        assert_eq!(resp.jsonrpc, "2.0");
        assert!(resp.result.is_some());
        assert!(resp.error.is_none());
    }

    #[test]
    fn test_error_response_deserialization() {
        let json =
            r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32601,"message":"Method not found"}}"#;
        let resp: Response<serde_json::Value> = serde_json::from_str(json).unwrap();
        assert!(resp.result.is_none());
        assert!(resp.error.is_some());
        let err = resp.error.unwrap();
        assert_eq!(err.code, RpcError::METHOD_NOT_FOUND);
        assert_eq!(err.message, "Method not found");
    }
}
