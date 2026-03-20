//! LSP server lifecycle management handlers

use crate::server::state::DaemonState;
use crate::server::types::*;
use aifed_common::{ServerActionResponse, ServerState};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};

pub async fn start_server(
    State(state): State<DaemonState>,
    Json(req): Json<StartServerRequest>,
) -> impl IntoResponse {
    match state.lsp_manager.start(&req.language, state.workspace.clone()).await {
        Ok(()) => {
            // Get the actual server state from the manager
            let servers = state.lsp_manager.list_servers().await;
            let server_state = servers
                .iter()
                .find(|s| s.language == req.language)
                .map(|s| s.state.clone())
                .unwrap_or_else(ServerState::running);

            let response = ServerActionResponse {
                language: req.language,
                workspace: state.workspace.to_string_lossy().to_string(),
                state: server_state,
            };
            (StatusCode::OK, Json(ApiResponse::success(response)))
        }
        Err(e) => {
            let resp: ApiResponse<ServerActionResponse> =
                ApiResponse::error(ErrorCode::LspStartFailed, e.to_string());
            (StatusCode::INTERNAL_SERVER_ERROR, Json(resp))
        }
    }
}

pub async fn stop_server(
    State(state): State<DaemonState>,
    Json(req): Json<StopServerRequest>,
) -> impl IntoResponse {
    match state.lsp_manager.stop(&req.language, &state.workspace, req.force).await {
        Ok(()) => {
            // Get the actual server state from the manager
            let servers = state.lsp_manager.list_servers().await;
            let server_state = servers
                .iter()
                .find(|s| s.language == req.language)
                .map(|s| s.state.clone())
                .unwrap_or_else(ServerState::stopped);

            let response = ServerActionResponse {
                language: req.language,
                workspace: state.workspace.to_string_lossy().to_string(),
                state: server_state,
            };
            (StatusCode::OK, Json(ApiResponse::success(response)))
        }
        Err(e) => {
            let code = if matches!(e, crate::error::Error::LspShutdownTimeout) {
                ErrorCode::LspServerBusy
            } else {
                ErrorCode::LspStopFailed
            };
            let resp = ApiResponse::<ServerActionResponse>::error(code, e.to_string());
            (StatusCode::INTERNAL_SERVER_ERROR, Json(resp))
        }
    }
}
