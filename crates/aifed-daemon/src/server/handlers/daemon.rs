//! Daemon status and health handlers

use crate::server::state::DaemonState;
use crate::server::types::*;
use axum::extract::State;
use axum::response::{IntoResponse, Json};
use std::time::Instant;

static START_TIME: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

pub async fn health(State(_state): State<DaemonState>) -> impl IntoResponse {
    Json(ApiResponse::success(HealthResponse { status: "ok".into() }))
}

pub async fn status(State(state): State<DaemonState>) -> impl IntoResponse {
    let start = START_TIME.get_or_init(Instant::now);
    let uptime_secs = start.elapsed().as_secs();
    let servers = state.lsp_manager.list_servers().await;

    Json(ApiResponse::success(StatusResponse {
        workspace: state.workspace.to_string_lossy().to_string(),
        uptime_secs,
        servers,
    }))
}

pub async fn list_servers(State(state): State<DaemonState>) -> impl IntoResponse {
    let manager = &state.lsp_manager;
    let server_statuses = manager.list_servers().await;

    let mut servers = Vec::with_capacity(server_statuses.len());
    for s in server_statuses {
        let progress = manager.get_server_progress(&s.language, &s.workspace).await;
        servers.push(ServerInfo {
            language: s.language,
            workspace: s.workspace.to_string_lossy().to_string(),
            state: s.state,
            progress: progress
                .into_iter()
                .map(|p| ProgressInfo {
                    title: p.title,
                    message: p.message,
                    percentage: p.percentage,
                })
                .collect(),
        });
    }

    Json(ApiResponse::success(ServersResponse { servers }))
}
