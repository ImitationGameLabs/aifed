//! HTTP handlers for clipboard operations

use crate::server::state::DaemonState;
use aifed_common::{ApiResponse, ClipboardResponse, SetClipboardRequest};
use axum::{Json, extract::State};

/// PUT /api/v1/clipboard
/// Set clipboard content
pub async fn set_clipboard(
    State(state): State<DaemonState>,
    Json(req): Json<SetClipboardRequest>,
) -> Json<ApiResponse<ClipboardResponse>> {
    let mut clipboard = state.clipboard.write().unwrap();
    *clipboard = req.content.clone();
    Json(ApiResponse::success(ClipboardResponse { content: req.content }))
}

/// GET /api/v1/clipboard
/// Get clipboard content
pub async fn get_clipboard(
    State(state): State<DaemonState>,
) -> Json<ApiResponse<ClipboardResponse>> {
    let clipboard = state.clipboard.read().unwrap();
    Json(ApiResponse::success(ClipboardResponse { content: clipboard.clone() }))
}
