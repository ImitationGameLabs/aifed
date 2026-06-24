//! HTTP router configuration

use crate::server::handlers;
use crate::server::state::DaemonState;
use axum::Router;
use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::{Next, from_fn_with_state};
use axum::response::Response;
use axum::routing::{get, post, put};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

/// Build the HTTP router for the daemon.
///
/// Every route is guarded by [`require_token`], which rejects requests that do
/// not present the daemon's bearer token. This both authorizes callers and lets
/// a CLI reliably distinguish our daemon from an unrelated process that reused
/// the port after a crash (it won't have the token → 401).
pub fn build_router(state: DaemonState) -> Router {
    let token_hash = state.token_hash;
    Router::new()
        // Daemon management
        .route("/api/v1/health", get(handlers::health))
        .route("/api/v1/heartbeat", post(handlers::heartbeat))
        .route("/api/v1/status", get(handlers::status))
        .route("/api/v1/shutdown", post(handlers::shutdown))
        // LSP server management
        .route("/api/v1/lsp/servers", get(handlers::list_servers))
        .route("/api/v1/lsp/servers/start", post(handlers::start_server))
        .route("/api/v1/lsp/servers/stop", post(handlers::stop_server))
        // LSP operations
        .route("/api/v1/lsp/definition", post(handlers::definition))
        .route("/api/v1/lsp/references", post(handlers::references))
        .route("/api/v1/lsp/hover", post(handlers::hover))
        .route("/api/v1/lsp/completions", post(handlers::completions))
        .route("/api/v1/lsp/diagnostics", post(handlers::diagnostics))
        .route("/api/v1/lsp/rename", post(handlers::rename))
        // Text document synchronization
        .route("/api/v1/lsp/didOpen", post(handlers::did_open))
        .route("/api/v1/lsp/didChange", post(handlers::did_change))
        .route("/api/v1/lsp/didClose", post(handlers::did_close))
        // History operations
        .route("/api/v1/history/access", post(handlers::record_access))
        .route("/api/v1/history/edit", post(handlers::record_edit))
        .route("/api/v1/history/{file}", get(handlers::get_history))
        .route("/api/v1/history/{file}/undo", post(handlers::undo))
        .route("/api/v1/history/{file}/redo", post(handlers::redo))
        // Clipboard operations
        .route("/api/v1/clipboard", put(handlers::set_clipboard))
        .route("/api/v1/clipboard", get(handlers::get_clipboard))
        .with_state(state)
        .layer(from_fn_with_state(token_hash, require_token))
}

/// Middleware: require `Authorization: Bearer <token>` whose SHA-256 matches the
/// daemon's stored hash. Comparing hashes (not the plaintext) in constant time
/// means the in-memory state holds no usable credential and the comparison
/// leaks no token material regardless of input length.
async fn require_token(
    State(expected): State<[u8; 32]>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let authorized = request
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .map(|token| {
            let received = Sha256::digest(token.as_bytes());
            bool::from(expected[..].ct_eq(received.as_slice()))
        })
        .unwrap_or(false);
    if authorized { Ok(next.run(request).await) } else { Err(StatusCode::UNAUTHORIZED) }
}
