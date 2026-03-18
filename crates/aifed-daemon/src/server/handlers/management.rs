//! LSP server lifecycle management handlers

use crate::server::state::DaemonState;
use crate::server::types::*;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};

pub async fn start_server(
    State(state): State<DaemonState>,
    Json(req): Json<StartServerRequest>,
) -> impl IntoResponse {
    match state.lsp_manager.start(&req.language, state.workspace.clone()).await {
        Ok(()) => {
            let server_info = ServerStatusResponse {
                language: req.language,
                workspace: state.workspace.to_string_lossy().to_string(),
                state: "running".into(),
            };
            (StatusCode::OK, Json(ApiResponse::success(server_info)))
        }
        Err(e) => {
            let resp: ApiResponse<ServerStatusResponse> =
                ApiResponse::error("LSP_START_FAILED", e.to_string());
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
            let server_info = ServerStatusResponse {
                language: req.language,
                workspace: state.workspace.to_string_lossy().to_string(),
                state: "stopped".into(),
            };
            (StatusCode::OK, Json(ApiResponse::success(server_info)))
        }
        Err(e) => {
            let code = if matches!(e, crate::error::Error::LspShutdownTimeout) {
                "LSP_SERVER_BUSY"
            } else {
                "LSP_STOP_FAILED"
            };
            let resp = ApiResponse::<ServerStatusResponse>::error(code, e.to_string());
            (StatusCode::INTERNAL_SERVER_ERROR, Json(resp))
        }
    }
}
