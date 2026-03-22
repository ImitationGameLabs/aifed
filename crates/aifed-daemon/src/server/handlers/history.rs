//! HTTP handlers for history operations

use crate::history::{HistoryEntry, HistoryError, LineDiff};
use crate::server::state::DaemonState;
use aifed_common::{
    ApiResponse, HistoryEntryDto, HistoryListResponse, LineDiffDto, RecordAccessRequest,
    RecordAccessResponse, RecordEditRequest, UndoRedoResponse,
};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde::Deserialize;
use std::path::PathBuf;

/// Convert internal LineDiff to DTO
fn line_diff_to_dto(diff: &LineDiff) -> LineDiffDto {
    LineDiffDto {
        line_num: diff.line_num,
        old_hash: diff.old_hash.clone(),
        old_content: diff.old_content.clone(),
        new_content: diff.new_content.clone(),
    }
}

/// Convert internal HistoryEntry to DTO
fn history_entry_to_dto(entry: &HistoryEntry) -> HistoryEntryDto {
    HistoryEntryDto {
        id: entry.id,
        timestamp: entry.timestamp,
        summary: entry.summary.clone(),
        diffs: entry.diffs.iter().map(line_diff_to_dto).collect(),
    }
}

/// Query parameters for history list
#[derive(Debug, Deserialize)]
pub struct HistoryQuery {
    #[serde(default)]
    pub count: Option<usize>,
    #[serde(default)]
    pub stat: bool,
}

/// Query parameters for undo/redo
#[derive(Debug, Deserialize)]
pub struct UndoRedoQuery {
    #[serde(default)]
    pub dry_run: bool,
}

/// POST /api/v1/history/access
/// Record a file access (read operation)
pub async fn record_access(
    State(state): State<DaemonState>,
    Json(req): Json<RecordAccessRequest>,
) -> Result<Json<ApiResponse<RecordAccessResponse>>, (StatusCode, Json<ApiResponse<()>>)> {
    let path = PathBuf::from(&req.file);
    tracing::info!("Record access for: {:?}", path);

    // Compute file hash
    let hash = match compute_file_hash(&path) {
        Ok(h) => h,
        Err(e) => {
            tracing::error!("Failed to compute hash: {}", e);
            return Err((StatusCode::BAD_REQUEST, Json(ApiResponse::error("FILE_ERROR", e))));
        }
    };

    // Record the access
    if let Err(e) = state.history_manager.record_access(&path, &hash) {
        tracing::error!("Failed to record access: {}", e);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::error("HISTORY_ERROR", e.to_string())),
        ));
    }

    tracing::info!("Record access succeeded, hash: {}", hash);
    Ok(Json(ApiResponse::success(RecordAccessResponse { hash })))
}

/// POST /api/v1/history/edit
/// Record an edit operation
pub async fn record_edit(
    State(state): State<DaemonState>,
    Json(req): Json<RecordEditRequest>,
) -> Result<Json<ApiResponse<()>>, (StatusCode, Json<ApiResponse<()>>)> {
    let path = PathBuf::from(&req.file);

    // Convert DTO diffs to internal diffs
    let diffs: Vec<LineDiff> = req
        .diffs
        .iter()
        .map(|d| match (&d.old_content, &d.new_content) {
            (None, Some(new)) => LineDiff::for_insertion(d.line_num, new),
            (Some(old), None) => {
                LineDiff::for_deletion(d.line_num, d.old_hash.as_deref().unwrap_or(""), old)
            }
            (Some(_), Some(new)) => LineDiff::for_replacement(
                d.line_num,
                d.old_hash.as_deref().unwrap_or(""),
                d.old_content.as_deref().unwrap_or(""),
                new,
            ),
            (None, None) => LineDiff::for_replacement(d.line_num, "", "", ""),
        })
        .collect();

    match state.history_manager.record_edit(&path, &req.expected_hash, &req.new_hash, diffs) {
        Ok(()) => Ok(Json(ApiResponse::ok())),
        Err(HistoryError::HashMismatch { expected, actual }) => Err((
            StatusCode::CONFLICT,
            Json(ApiResponse::error(
                "HASH_MISMATCH",
                format!(
                    "File modified externally. Expected hash: {}, actual: {}",
                    expected, actual
                ),
            )),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::error("HISTORY_ERROR", e.to_string())),
        )),
    }
}

/// GET /api/v1/history/:file
/// List history for a file
pub async fn get_history(
    State(state): State<DaemonState>,
    Path(file): Path<String>,
    Query(query): Query<HistoryQuery>,
) -> Json<ApiResponse<HistoryListResponse>> {
    let path = PathBuf::from(&file);
    let entries = state.history_manager.get_history(&path, query.count);

    let dtos: Vec<HistoryEntryDto> = if query.stat {
        // Stat mode: return summaries only, no diffs
        entries
            .iter()
            .map(|e| HistoryEntryDto {
                id: e.id,
                timestamp: e.timestamp,
                summary: e.summary.clone(),
                diffs: Vec::new(), // Empty diffs in stat mode
            })
            .collect()
    } else {
        // Verbose mode: return full details
        entries.iter().map(history_entry_to_dto).collect()
    };

    Json(ApiResponse::success(HistoryListResponse { entries: dtos }))
}

/// POST /api/v1/history/:file/undo
/// Undo the last edit for a file
pub async fn undo(
    State(state): State<DaemonState>,
    Path(file): Path<String>,
    Query(query): Query<UndoRedoQuery>,
) -> Result<Json<ApiResponse<UndoRedoResponse>>, (StatusCode, Json<ApiResponse<()>>)> {
    let path = PathBuf::from(&file);

    let result = match state.history_manager.undo(&path, query.dry_run) {
        Ok(r) => r,
        Err(HistoryError::NoHistory) => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ApiResponse::error("NO_HISTORY", "No history available for this file")),
            ));
        }
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error("HISTORY_ERROR", e.to_string())),
            ));
        }
    };

    let dto_diffs: Vec<LineDiffDto> = result.diffs.iter().map(line_diff_to_dto).collect();

    Ok(Json(ApiResponse::success(UndoRedoResponse {
        diffs: dto_diffs,
        current_hash: result.current_hash,
    })))
}

/// POST /api/v1/history/:file/redo
/// Redo the last undone edit for a file
pub async fn redo(
    State(state): State<DaemonState>,
    Path(file): Path<String>,
    Query(query): Query<UndoRedoQuery>,
) -> Result<Json<ApiResponse<UndoRedoResponse>>, (StatusCode, Json<ApiResponse<()>>)> {
    let path = PathBuf::from(&file);

    let result = match state.history_manager.redo(&path, query.dry_run) {
        Ok(r) => r,
        Err(HistoryError::NoRedo) => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ApiResponse::error("NO_REDO", "No redo available for this file")),
            ));
        }
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error("HISTORY_ERROR", e.to_string())),
            ));
        }
    };

    let dto_diffs: Vec<LineDiffDto> = result.diffs.iter().map(line_diff_to_dto).collect();

    Ok(Json(ApiResponse::success(UndoRedoResponse {
        diffs: dto_diffs,
        current_hash: result.current_hash,
    })))
}

/// Compute hash for a file (using xxhash)
fn compute_file_hash(path: &PathBuf) -> Result<String, String> {
    use std::io::Read;

    let mut file = std::fs::File::open(path).map_err(|e| format!("Failed to open file: {}", e))?;
    let mut content = Vec::new();
    file.read_to_end(&mut content).map_err(|e| format!("Failed to read file: {}", e))?;

    // Use xxhash from the crate's dependencies
    let hash = xxhash_rust::xxh3::xxh3_64(&content);
    Ok(format!("{:016X}", hash))
}
