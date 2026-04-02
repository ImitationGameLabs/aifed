//! Type conversion utilities between LSP types and API types

use super::types::{
    ContentChange, DiagnosticItem, FileEdit, LocationResponse, Position, Range, TextEdit,
};
use lsp_types::{HoverContents, MarkedString};

/// Convert file path to LSP URI
pub fn file_path_to_uri(path: &str) -> Result<lsp_types::Url, String> {
    lsp_types::Url::from_file_path(path).map_err(|_| format!("Invalid file path: {}", path))
}

/// Create TextDocumentIdentifier from file path
pub fn text_document_id(path: &str) -> Result<lsp_types::TextDocumentIdentifier, String> {
    Ok(lsp_types::TextDocumentIdentifier { uri: file_path_to_uri(path)? })
}

/// Create TextDocumentPositionParams from request
pub fn text_document_position(
    path: &str,
    position: &Position,
) -> Result<lsp_types::TextDocumentPositionParams, String> {
    Ok(lsp_types::TextDocumentPositionParams {
        text_document: text_document_id(path)?,
        position: lsp_types::Position { line: position.line, character: position.character },
    })
}

/// Convert LSP Location to API LocationResponse
pub fn location_to_response(loc: lsp_types::Location) -> LocationResponse {
    LocationResponse {
        file_path: loc
            .uri
            .to_file_path()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string(),
        range: Range {
            start: Position { line: loc.range.start.line, character: loc.range.start.character },
            end: Position { line: loc.range.end.line, character: loc.range.end.character },
        },
    }
}

/// Convert LSP Range to API Range
pub fn lsp_range_to_range(range: lsp_types::Range) -> Range {
    Range {
        start: Position { line: range.start.line, character: range.start.character },
        end: Position { line: range.end.line, character: range.end.character },
    }
}

/// Convert API Range to LSP Range
pub fn range_to_lsp_range(range: Range) -> lsp_types::Range {
    lsp_types::Range {
        start: lsp_types::Position { line: range.start.line, character: range.start.character },
        end: lsp_types::Position { line: range.end.line, character: range.end.character },
    }
}

/// Extract string content from HoverContents
pub fn hover_contents_to_string(contents: HoverContents) -> String {
    match contents {
        HoverContents::Scalar(s) => marked_string_to_string(s),
        HoverContents::Array(arr) => arr
            .into_iter()
            .map(marked_string_to_string)
            .collect::<Vec<_>>()
            .join("\n"),
        HoverContents::Markup(m) => m.value,
    }
}

/// Convert MarkedString to String
fn marked_string_to_string(ms: MarkedString) -> String {
    match ms {
        MarkedString::String(s) => s,
        MarkedString::LanguageString(ls) => {
            format!("```{}\n{}\n```", ls.language, ls.value)
        }
    }
}

/// Convert LSP DiagnosticSeverity to string
pub fn severity_to_string(severity: Option<lsp_types::DiagnosticSeverity>) -> String {
    match severity {
        Some(lsp_types::DiagnosticSeverity::ERROR) => "error",
        Some(lsp_types::DiagnosticSeverity::WARNING) => "warning",
        Some(lsp_types::DiagnosticSeverity::INFORMATION) => "information",
        Some(lsp_types::DiagnosticSeverity::HINT) => "hint",
        _ => "unknown",
    }
    .to_string()
}

/// Extract diagnostics from DocumentDiagnosticReportResult
pub fn extract_diagnostics(
    report: lsp_types::DocumentDiagnosticReportResult,
) -> Vec<DiagnosticItem> {
    match report {
        lsp_types::DocumentDiagnosticReportResult::Report(report) => match report {
            lsp_types::DocumentDiagnosticReport::Full(related) => related
                .full_document_diagnostic_report
                .items
                .into_iter()
                .map(|d| DiagnosticItem {
                    range: lsp_range_to_range(d.range),
                    severity: severity_to_string(d.severity),
                    message: d.message,
                })
                .collect(),
            lsp_types::DocumentDiagnosticReport::Unchanged(_) => vec![],
        },
        lsp_types::DocumentDiagnosticReportResult::Partial(partial) => {
            let mut items = Vec::new();
            if let Some(related) = partial.related_documents {
                for (_, doc) in related {
                    match doc {
                        lsp_types::DocumentDiagnosticReportKind::Full(full) => {
                            items.extend(full.items.into_iter().map(|d| DiagnosticItem {
                                range: lsp_range_to_range(d.range),
                                severity: severity_to_string(d.severity),
                                message: d.message,
                            }));
                        }
                        lsp_types::DocumentDiagnosticReportKind::Unchanged(_) => {}
                    }
                }
            }
            items
        }
    }
}

/// Convert WorkspaceEdit to Vec<FileEdit>
pub fn workspace_edit_to_file_edits(edit: lsp_types::WorkspaceEdit) -> Vec<FileEdit> {
    let mut result = Vec::new();

    if let Some(changes) = edit.changes {
        for (uri, edits) in changes {
            let file_path = uri
                .to_file_path()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let text_edits: Vec<TextEdit> = edits
                .into_iter()
                .map(|e| TextEdit { range: lsp_range_to_range(e.range), new_text: e.new_text })
                .collect();
            result.push(FileEdit { file_path, edits: text_edits });
        }
    }

    if let Some(document_changes) = edit.document_changes {
        match document_changes {
            lsp_types::DocumentChanges::Edits(edits) => {
                for edit in edits {
                    let file_path = edit
                        .text_document
                        .uri
                        .to_file_path()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    let text_edits: Vec<TextEdit> = edit
                        .edits
                        .into_iter()
                        .map(|e| match e {
                            lsp_types::OneOf::Left(e) => TextEdit {
                                range: lsp_range_to_range(e.range),
                                new_text: e.new_text,
                            },
                            lsp_types::OneOf::Right(annotated) => TextEdit {
                                range: lsp_range_to_range(annotated.text_edit.range),
                                new_text: annotated.text_edit.new_text,
                            },
                        })
                        .collect();
                    result.push(FileEdit { file_path, edits: text_edits });
                }
            }
            lsp_types::DocumentChanges::Operations(_) => {
                // File operations (create/delete/rename) not supported yet
            }
        }
    }

    result
}

/// Convert API ContentChange to LSP TextDocumentContentChangeEvent
pub fn content_changes_to_lsp(
    changes: Vec<ContentChange>,
) -> Vec<lsp_types::TextDocumentContentChangeEvent> {
    changes
        .into_iter()
        .map(|c| lsp_types::TextDocumentContentChangeEvent {
            range: c.range.map(range_to_lsp_range),
            range_length: None,
            text: c.text,
        })
        .collect()
}
