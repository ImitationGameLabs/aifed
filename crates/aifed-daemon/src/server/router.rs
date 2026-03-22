//! HTTP router configuration

use crate::server::handlers;
use crate::server::state::DaemonState;
use axum::Router;
use axum::routing::{get, post};

/// Build the HTTP router for the daemon.
pub fn build_router(state: DaemonState) -> Router {
    Router::new()
        // Daemon management
        .route("/api/v1/health", get(handlers::health))
        .route("/api/v1/heartbeat", post(handlers::heartbeat))
        .route("/api/v1/status", get(handlers::status))
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
        .with_state(state)
}
