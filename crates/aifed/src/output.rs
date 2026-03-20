use serde::Serialize;

// Re-export LSP response types for formatting
pub use aifed_common::{
    CompletionsResponse, DefinitionResponse, DiagnosticsResponse, HoverResponse,
    ReferencesResponse, RenameResponse,
};

/// Output format selector
#[derive(Debug, Clone, Copy, Default)]
pub enum OutputFormat {
    #[default]
    Text,
    Json,
}

/// Line with hash for output
#[derive(Debug, Clone, Serialize)]
pub struct HashedLine {
    pub line: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    pub content: String,
}

/// File info for output
#[derive(Debug, Serialize)]
pub struct FileInfo {
    pub path: String,
    pub lines: usize,
    pub size: u64,
}

/// Edit result for output
#[derive(Debug, Serialize)]
pub struct EditResult {
    pub success: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub changes: Option<Vec<EditChange>>,
}

#[derive(Debug, Serialize)]
pub struct EditChange {
    pub operation: String,
    pub line: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_content: Option<String>,
}

/// Batch operation error
#[derive(Debug, Serialize)]
pub struct BatchOpError {
    pub line: usize,
    pub operation: String,
    pub error: String,
}

/// Batch edit result for output
#[derive(Debug, Serialize)]
pub struct BatchResult {
    pub success: bool,
    pub total: usize,
    pub successful: usize,
    pub failed: usize,
    pub message: String,
    /// New file content after applying changes
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub new_lines: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub changes: Vec<EditChange>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<BatchOpError>,
}

/// Format hashed lines for output
pub fn format_lines(lines: &[HashedLine], format: OutputFormat, no_hashes: bool) -> String {
    match format {
        OutputFormat::Text => {
            if no_hashes {
                lines.iter().map(|l| l.content.clone()).collect::<Vec<_>>().join("\n")
            } else {
                lines
                    .iter()
                    .map(|l| format!("{}:{}|{}", l.line, l.hash.as_ref().unwrap(), l.content))
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        }
        OutputFormat::Json => {
            let output_lines: Vec<HashedLine> = lines
                .iter()
                .map(|l| HashedLine {
                    line: l.line,
                    hash: if no_hashes { None } else { l.hash.clone() },
                    content: l.content.clone(),
                })
                .collect();

            #[derive(Serialize)]
            struct Output {
                lines: Vec<HashedLine>,
            }

            serde_json::to_string_pretty(&Output { lines: output_lines }).unwrap_or_default()
        }
    }
}

/// Format file info for output
pub fn format_file_info(info: &FileInfo, format: OutputFormat) -> String {
    match format {
        OutputFormat::Text => {
            let size_str = format_size(info.size);
            format!("Path: {}\nLines: {}\nSize: {}", info.path, info.lines, size_str)
        }
        OutputFormat::Json => serde_json::to_string_pretty(&info).unwrap_or_default(),
    }
}

/// Format edit result for output
pub fn format_edit_result(result: &EditResult, format: OutputFormat) -> String {
    match format {
        OutputFormat::Text => {
            if result.success {
                result.message.clone()
            } else {
                format!("Error: {}", result.message)
            }
        }
        OutputFormat::Json => serde_json::to_string_pretty(&result).unwrap_or_default(),
    }
}

/// Format batch result for output
pub fn format_batch_result(result: &BatchResult, format: OutputFormat) -> String {
    match format {
        OutputFormat::Text => {
            let mut output = result.message.clone();
            if !result.errors.is_empty() {
                output.push_str("\n\nErrors:");
                for err in &result.errors {
                    output.push_str(&format!(
                        "\n  Line {}: {} - {}",
                        err.line, err.operation, err.error
                    ));
                }
            }
            output
        }
        OutputFormat::Json => serde_json::to_string_pretty(&result).unwrap_or_default(),
    }
}

/// Format an error for output
pub fn format_error(error: &crate::error::Error, format: OutputFormat) -> String {
    match format {
        OutputFormat::Text => error.to_string(),
        OutputFormat::Json => {
            #[derive(Serialize)]
            struct ErrorOutput {
                error: String,
            }
            let output = ErrorOutput { error: error.to_string() };
            serde_json::to_string_pretty(&output).unwrap_or_default()
        }
    }
}

/// Format size in human-readable form
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;

    if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

// --- LSP Response Formatters ---

/// Format hover response
pub fn format_hover_response(resp: &HoverResponse, format: OutputFormat) -> String {
    match format {
        OutputFormat::Text => resp.contents.clone().unwrap_or_else(|| "No hover info".to_string()),
        OutputFormat::Json => serde_json::to_string_pretty(&resp).unwrap_or_default(),
    }
}

/// Format definition response
pub fn format_definition_response(resp: &DefinitionResponse, format: OutputFormat) -> String {
    match format {
        OutputFormat::Text => {
            if resp.locations.is_empty() {
                "No definition found".to_string()
            } else {
                resp.locations
                    .iter()
                    .map(|l| {
                        format!(
                            "{}:{}:{}",
                            l.file_path,
                            l.range.start.line + 1,
                            l.range.start.character + 1
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        }
        OutputFormat::Json => serde_json::to_string_pretty(&resp).unwrap_or_default(),
    }
}

/// Format references response
pub fn format_references_response(resp: &ReferencesResponse, format: OutputFormat) -> String {
    match format {
        OutputFormat::Text => {
            if resp.locations.is_empty() {
                "No references found".to_string()
            } else {
                resp.locations
                    .iter()
                    .map(|l| {
                        format!(
                            "{}:{}:{}",
                            l.file_path,
                            l.range.start.line + 1,
                            l.range.start.character + 1
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        }
        OutputFormat::Json => serde_json::to_string_pretty(&resp).unwrap_or_default(),
    }
}

/// Format completions response
pub fn format_completions_response(resp: &CompletionsResponse, format: OutputFormat) -> String {
    match format {
        OutputFormat::Text => {
            if resp.items.is_empty() {
                "No completions available".to_string()
            } else {
                resp.items
                    .iter()
                    .map(|item| {
                        let mut s = item.label.clone();
                        if let Some(detail) = &item.detail {
                            s.push_str(&format!(" - {}", detail));
                        }
                        s
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        }
        OutputFormat::Json => serde_json::to_string_pretty(&resp).unwrap_or_default(),
    }
}

/// Format diagnostics response
pub fn format_diagnostics_response(resp: &DiagnosticsResponse, format: OutputFormat) -> String {
    match format {
        OutputFormat::Text => {
            if resp.diagnostics.is_empty() {
                "No diagnostics".to_string()
            } else {
                resp.diagnostics
                    .iter()
                    .map(|d| {
                        format!(
                            "[{}] line {}: {}",
                            d.severity.to_uppercase(),
                            d.range.start.line + 1,
                            d.message
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        }
        OutputFormat::Json => serde_json::to_string_pretty(&resp).unwrap_or_default(),
    }
}

/// Format rename preview for dry-run mode.
pub fn format_rename_preview(resp: &RenameResponse, format: OutputFormat) -> String {
    match format {
        OutputFormat::Text => {
            if resp.changes.is_empty() {
                return "No changes".to_string();
            }

            let mut output = Vec::new();
            for file_edit in &resp.changes {
                output.push(format!("File: {}", file_edit.file_path));
                for edit in &file_edit.edits {
                    output.push(format!(
                        "  Line {}:{} - {}:{}",
                        edit.range.start.line + 1,
                        edit.range.start.character + 1,
                        edit.range.end.line + 1,
                        edit.range.end.character + 1
                    ));
                    output.push(format!("    -> {}", edit.new_text));
                }
            }
            output.join("\n")
        }
        OutputFormat::Json => serde_json::to_string_pretty(&resp).unwrap_or_default(),
    }
}

/// Format rename result summary for normal mode.
pub fn format_rename_result(resp: &RenameResponse, format: OutputFormat) -> String {
    match format {
        OutputFormat::Text => {
            if resp.changes.is_empty() {
                return "No changes".to_string();
            }

            let total_edits: usize = resp.changes.iter().map(|f| f.edits.len()).sum();
            format!("Renamed in {} file(s), {} edit(s)", resp.changes.len(), total_edits)
        }
        OutputFormat::Json => serde_json::to_string_pretty(&resp).unwrap_or_default(),
    }
}
