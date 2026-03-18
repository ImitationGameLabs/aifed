//! LSP operation handlers

use crate::server::converters::*;
use crate::server::state::DaemonState;
use crate::server::types::*;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};
use lsp_types::{
    CompletionParams, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, DocumentDiagnosticParams, GotoDefinitionParams, HoverParams,
    ReferenceContext, ReferenceParams, RenameParams, TextDocumentIdentifier, TextDocumentItem,
    VersionedTextDocumentIdentifier,
};

pub async fn definition(
    State(state): State<DaemonState>,
    Json(req): Json<LspPositionRequest>,
) -> impl IntoResponse {
    state.idle_monitor.record_activity();

    let params = match text_document_position(&req.file_path, &req.position) {
        Ok(p) => GotoDefinitionParams {
            text_document_position_params: p,
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        },
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<DefinitionResponse>::error("INVALID_PATH", e)),
            );
        }
    };

    match state.lsp_manager.goto_definition(&req.language, &state.workspace, params).await {
        Ok(Some(response)) => {
            let locations = match response {
                lsp_types::GotoDefinitionResponse::Scalar(loc) => vec![location_to_response(loc)],
                lsp_types::GotoDefinitionResponse::Array(locs) => {
                    locs.into_iter().map(location_to_response).collect()
                }
                lsp_types::GotoDefinitionResponse::Link(links) => links
                    .into_iter()
                    .map(|link| LocationResponse {
                        file_path: link
                            .target_uri
                            .to_file_path()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string(),
                        range: lsp_range_to_range(link.target_range),
                    })
                    .collect(),
            };
            (StatusCode::OK, Json(ApiResponse::success(DefinitionResponse { locations })))
        }
        Ok(None) => {
            (StatusCode::OK, Json(ApiResponse::success(DefinitionResponse { locations: vec![] })))
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<DefinitionResponse>::error("LSP_ERROR", e.to_string())),
        ),
    }
}

pub async fn references(
    State(state): State<DaemonState>,
    Json(req): Json<LspPositionRequest>,
) -> impl IntoResponse {
    state.idle_monitor.record_activity();

    let text_doc_pos = match text_document_position(&req.file_path, &req.position) {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<ReferencesResponse>::error("INVALID_PATH", e)),
            );
        }
    };

    let params = ReferenceParams {
        text_document_position: text_doc_pos,
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
        context: ReferenceContext { include_declaration: true },
    };

    match state.lsp_manager.references(&req.language, &state.workspace, params).await {
        Ok(Some(locs)) => {
            let locations = locs.into_iter().map(location_to_response).collect();
            (StatusCode::OK, Json(ApiResponse::success(ReferencesResponse { locations })))
        }
        Ok(None) => {
            (StatusCode::OK, Json(ApiResponse::success(ReferencesResponse { locations: vec![] })))
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<ReferencesResponse>::error("LSP_ERROR", e.to_string())),
        ),
    }
}

pub async fn hover(
    State(state): State<DaemonState>,
    Json(req): Json<HoverRequest>,
) -> impl IntoResponse {
    state.idle_monitor.record_activity();

    let params = match text_document_position(&req.file_path, &req.position) {
        Ok(p) => HoverParams {
            text_document_position_params: p,
            work_done_progress_params: Default::default(),
        },
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<HoverResponse>::error("INVALID_PATH", e)),
            );
        }
    };

    match state.lsp_manager.hover(&req.language, &state.workspace, params).await {
        Ok(Some(hover)) => {
            let contents = Some(hover_contents_to_string(hover.contents));
            (StatusCode::OK, Json(ApiResponse::success(HoverResponse { contents })))
        }
        Ok(None) => (StatusCode::OK, Json(ApiResponse::success(HoverResponse { contents: None }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<HoverResponse>::error("LSP_ERROR", e.to_string())),
        ),
    }
}

pub async fn completions(
    State(state): State<DaemonState>,
    Json(req): Json<LspPositionRequest>,
) -> impl IntoResponse {
    state.idle_monitor.record_activity();

    let text_doc_pos = match text_document_position(&req.file_path, &req.position) {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<CompletionsResponse>::error("INVALID_PATH", e)),
            );
        }
    };

    let params = CompletionParams {
        text_document_position: text_doc_pos,
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
        context: None,
    };

    match state.lsp_manager.completion(&req.language, &state.workspace, params).await {
        Ok(Some(response)) => {
            let items = match response {
                lsp_types::CompletionResponse::Array(arr) => arr,
                lsp_types::CompletionResponse::List(list) => list.items,
            };
            let items = items
                .into_iter()
                .map(|item| CompletionItem {
                    label: item.label,
                    detail: item.detail,
                    documentation: item.documentation.map(|doc| match doc {
                        lsp_types::Documentation::String(s) => s,
                        lsp_types::Documentation::MarkupContent(m) => m.value,
                    }),
                })
                .collect();
            (StatusCode::OK, Json(ApiResponse::success(CompletionsResponse { items })))
        }
        Ok(None) => {
            (StatusCode::OK, Json(ApiResponse::success(CompletionsResponse { items: vec![] })))
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<CompletionsResponse>::error("LSP_ERROR", e.to_string())),
        ),
    }
}

pub async fn diagnostics(
    State(state): State<DaemonState>,
    Json(req): Json<DiagnosticsRequest>,
) -> impl IntoResponse {
    state.idle_monitor.record_activity();

    let uri = match file_path_to_uri(&req.file_path) {
        Ok(u) => u,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<DiagnosticsResponse>::error("INVALID_PATH", e)),
            );
        }
    };

    let params = DocumentDiagnosticParams {
        text_document: TextDocumentIdentifier { uri },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
        identifier: None,
        previous_result_id: None,
    };

    match state.lsp_manager.diagnostic(&req.language, &state.workspace, params).await {
        Ok(report) => {
            let diagnostics = extract_diagnostics(report);
            (StatusCode::OK, Json(ApiResponse::success(DiagnosticsResponse { diagnostics })))
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<DiagnosticsResponse>::error("LSP_ERROR", e.to_string())),
        ),
    }
}

pub async fn rename(
    State(state): State<DaemonState>,
    Json(req): Json<RenameRequest>,
) -> impl IntoResponse {
    state.idle_monitor.record_activity();

    let text_doc_pos = match text_document_position(&req.file_path, &req.position) {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<RenameResponse>::error("INVALID_PATH", e)),
            );
        }
    };

    let params = RenameParams {
        text_document_position: text_doc_pos,
        work_done_progress_params: Default::default(),
        new_name: req.new_name,
    };

    match state.lsp_manager.rename(&req.language, &state.workspace, params).await {
        Ok(Some(edit)) => {
            let changes = workspace_edit_to_file_edits(edit);
            (StatusCode::OK, Json(ApiResponse::success(RenameResponse { changes })))
        }
        Ok(None) => {
            (StatusCode::OK, Json(ApiResponse::success(RenameResponse { changes: vec![] })))
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<RenameResponse>::error("LSP_ERROR", e.to_string())),
        ),
    }
}

pub async fn did_open(
    State(state): State<DaemonState>,
    Json(req): Json<DidOpenRequest>,
) -> impl IntoResponse {
    state.idle_monitor.record_activity();

    let uri = match file_path_to_uri(&req.file_path) {
        Ok(u) => u,
        Err(e) => {
            return (StatusCode::BAD_REQUEST, Json(ApiResponse::<()>::error("INVALID_PATH", e)));
        }
    };

    let params = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri,
            language_id: req.language_id,
            version: req.version,
            text: req.text,
        },
    };

    match state.lsp_manager.did_open(&req.language, &state.workspace, params).await {
        Ok(()) => (StatusCode::OK, Json(ApiResponse::ok())),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<()>::error("LSP_ERROR", e.to_string())),
        ),
    }
}

pub async fn did_change(
    State(state): State<DaemonState>,
    Json(req): Json<DidChangeRequest>,
) -> impl IntoResponse {
    state.idle_monitor.record_activity();

    let uri = match file_path_to_uri(&req.file_path) {
        Ok(u) => u,
        Err(e) => {
            return (StatusCode::BAD_REQUEST, Json(ApiResponse::<()>::error("INVALID_PATH", e)));
        }
    };

    let params = DidChangeTextDocumentParams {
        text_document: VersionedTextDocumentIdentifier { uri, version: req.version },
        content_changes: content_changes_to_lsp(req.content_changes),
    };

    match state.lsp_manager.did_change(&req.language, &state.workspace, params).await {
        Ok(()) => (StatusCode::OK, Json(ApiResponse::ok())),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<()>::error("LSP_ERROR", e.to_string())),
        ),
    }
}

pub async fn did_close(
    State(state): State<DaemonState>,
    Json(req): Json<DidCloseRequest>,
) -> impl IntoResponse {
    state.idle_monitor.record_activity();

    let uri = match file_path_to_uri(&req.file_path) {
        Ok(u) => u,
        Err(e) => {
            return (StatusCode::BAD_REQUEST, Json(ApiResponse::<()>::error("INVALID_PATH", e)));
        }
    };

    let params = DidCloseTextDocumentParams { text_document: TextDocumentIdentifier { uri } };

    match state.lsp_manager.did_close(&req.language, &state.workspace, params).await {
        Ok(()) => (StatusCode::OK, Json(ApiResponse::ok())),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<()>::error("LSP_ERROR", e.to_string())),
        ),
    }
}
