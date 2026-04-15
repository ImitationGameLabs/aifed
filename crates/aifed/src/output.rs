use serde::Serialize;
use std::path::Path;

use aifed_common::workspace::detect_workspace;

// Re-export LSP response types for formatting
pub use aifed_common::{
    CompletionsResponse, DefinitionResponse, DiagnosticsResponse, HoverResponse, LineDiffDto,
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

#[derive(Debug, Clone, Serialize)]
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

#[derive(Debug, Clone)]
pub struct RenameFileDiff {
    pub file_path: String,
    pub edit_count: usize,
    pub diffs: Vec<LineDiffDto>,
    pub new_lines: Vec<String>,
}

/// Format hashed lines for output
pub fn format_lines(lines: &[HashedLine], format: OutputFormat, no_hashes: bool) -> String {
    match format {
        OutputFormat::Text => {
            if no_hashes {
                lines
                    .iter()
                    .map(|l| l.content.clone())
                    .collect::<Vec<_>>()
                    .join("\n")
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
            format!(
                "Path: {}\nLines: {}\nSize: {}",
                info.path, info.lines, size_str
            )
        }
        OutputFormat::Json => serde_json::to_string_pretty(&info).unwrap_or_default(),
    }
}

/// Compute diff summary from edit changes
pub fn compute_change_summary(changes: &[EditChange]) -> String {
    let mut insertions = 0;
    let mut deletions = 0;

    for change in changes {
        match change.operation.as_str() {
            "insert" => insertions += 1,
            "delete" => deletions += 1,
            "replace" => {
                deletions += 1;
                insertions += 1;
            }
            _ => {}
        }
    }

    let mut parts = Vec::new();
    if insertions > 0 {
        parts.push(format!(
            "{} insertion{}(+)",
            insertions,
            if insertions > 1 { "s" } else { "" }
        ));
    }
    if deletions > 0 {
        parts.push(format!(
            "{} deletion{}(-)",
            deletions,
            if deletions > 1 { "s" } else { "" }
        ));
    }

    if parts.is_empty() { "no changes".to_string() } else { parts.join(", ") }
}

/// Convert EditChange to LineDiffDto for diff formatting
fn changes_to_diffs(changes: &[EditChange]) -> Vec<aifed_common::LineDiffDto> {
    changes
        .iter()
        .map(|c| aifed_common::LineDiffDto {
            line_num: c.line,
            old_hash: None,
            old_content: c.old_content.clone(),
            new_content: c.new_content.clone(),
        })
        .collect()
}

/// Format batch result with diff view for output
pub fn format_batch_result_with_diff(
    result: &BatchResult,
    format: OutputFormat,
    new_lines: &[String],
) -> String {
    match format {
        OutputFormat::Text => {
            let mut output = Vec::new();

            // Message line (with summary appended)
            if !result.changes.is_empty() {
                let summary = compute_change_summary(&result.changes);
                if summary != "no changes" {
                    output.push(format!("{}, {}", result.message, summary));
                } else {
                    output.push(result.message.clone());
                }

                // Diff view with context (using new file content for context)
                let diffs = changes_to_diffs(&result.changes);
                let diff_view = crate::diff::format_diffs_with_context(&diffs, new_lines, 3);
                if diff_view != "  (no changes)" {
                    output.push(diff_view);
                }
            } else {
                output.push(result.message.clone());
            }

            // Errors
            if !result.errors.is_empty() {
                output.push(String::new());
                output.push("Errors:".to_string());
                for err in &result.errors {
                    output.push(format!(
                        "  Line {}: {} - {}",
                        err.line, err.operation, err.error
                    ));
                }
            }

            output.join("\n")
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
        OutputFormat::Text => resp
            .contents
            .clone()
            .unwrap_or_else(|| "No hover info".to_string()),
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
pub fn format_rename_preview(
    resp: &RenameResponse,
    file_diffs: &[RenameFileDiff],
    format: OutputFormat,
) -> String {
    match format {
        OutputFormat::Text => {
            if resp.changes.is_empty() {
                return "No changes".to_string();
            }
            format_rename_text("Rename preview", file_diffs)
        }
        OutputFormat::Json => serde_json::to_string_pretty(&resp).unwrap_or_default(),
    }
}

/// Format rename result summary for normal mode.
pub fn format_rename_result(
    resp: &RenameResponse,
    file_diffs: &[RenameFileDiff],
    format: OutputFormat,
) -> String {
    match format {
        OutputFormat::Text => {
            if resp.changes.is_empty() {
                return "No changes".to_string();
            }
            format_rename_text("Renamed", file_diffs)
        }
        OutputFormat::Json => serde_json::to_string_pretty(&resp).unwrap_or_default(),
    }
}

fn format_rename_text(verb: &str, file_diffs: &[RenameFileDiff]) -> String {
    let mut file_diffs = file_diffs.to_vec();
    file_diffs.sort_by(|a, b| a.file_path.cmp(&b.file_path));

    let total_edits: usize = file_diffs.iter().map(|f| f.edit_count).sum();
    let mut output = vec![format!(
        "{} in {} file(s), {} edit(s)",
        verb,
        file_diffs.len(),
        total_edits
    )];

    for file_diff in file_diffs {
        output.push(String::new());

        let display_path = display_rename_path(&file_diff.file_path);
        output.push(format!("File: {display_path}"));

        let diff_view =
            crate::diff::format_diffs_with_context(&file_diff.diffs, &file_diff.new_lines, 3);
        if diff_view != "  (no changes)" {
            output.push(diff_view);
        }
    }

    output.join("\n")
}

fn display_rename_path(file_path: &str) -> String {
    let path = Path::new(file_path);

    if let Some(workspace_root) = path
        .parent()
        .and_then(detect_workspace)
        .map(|workspace| workspace.root().to_path_buf())
        && let Ok(relative) = path.strip_prefix(&workspace_root)
    {
        return relative.to_string_lossy().to_string();
    }

    file_path.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use aifed_common::{FileEdit, Position, Range, TextEdit};

    fn make_text_edit(
        start_line: u32,
        start_character: u32,
        end_line: u32,
        end_character: u32,
        new_text: &str,
    ) -> TextEdit {
        TextEdit {
            range: Range {
                start: Position { line: start_line, character: start_character },
                end: Position { line: end_line, character: end_character },
            },
            new_text: new_text.to_string(),
        }
    }

    fn make_line_diff(line_num: usize, old: Option<&str>, new: Option<&str>) -> LineDiffDto {
        LineDiffDto {
            line_num,
            old_hash: None,
            old_content: old.map(str::to_string),
            new_content: new.map(str::to_string),
        }
    }

    #[test]
    fn format_rename_preview_shows_single_file_header() {
        let response = RenameResponse {
            changes: vec![FileEdit {
                file_path: "/workspace/src/main.rs".to_string(),
                edits: vec![make_text_edit(1, 4, 1, 8, "new_name")],
            }],
        };
        let file_diffs = vec![RenameFileDiff {
            file_path: "/workspace/src/main.rs".to_string(),
            edit_count: 1,
            diffs: vec![make_line_diff(2, Some("let old_name = 1;"), Some("let new_name = 1;"))],
            new_lines: vec![
                "fn main() {".to_string(),
                "let new_name = 1;".to_string(),
                "}".to_string(),
            ],
        }];

        let output = format_rename_preview(&response, &file_diffs, OutputFormat::Text);

        assert!(output.contains("Rename preview in 1 file(s), 1 edit(s)"));
        assert!(output.contains("File: /workspace/src/main.rs"));
        assert!(output.contains("-2|let old_name = 1;"));
        assert!(output.contains("+2|let new_name = 1;"));
    }

    #[test]
    fn format_rename_result_sorts_files_for_stable_output() {
        let response = RenameResponse {
            changes: vec![
                FileEdit {
                    file_path: "/workspace/src/z.rs".to_string(),
                    edits: vec![make_text_edit(0, 0, 0, 1, "renamed_z")],
                },
                FileEdit {
                    file_path: "/workspace/src/a.rs".to_string(),
                    edits: vec![make_text_edit(0, 0, 0, 1, "renamed_a")],
                },
            ],
        };
        let file_diffs = vec![
            RenameFileDiff {
                file_path: "/workspace/src/z.rs".to_string(),
                edit_count: 1,
                diffs: vec![make_line_diff(1, Some("z"), Some("renamed_z"))],
                new_lines: vec!["renamed_z".to_string()],
            },
            RenameFileDiff {
                file_path: "/workspace/src/a.rs".to_string(),
                edit_count: 1,
                diffs: vec![make_line_diff(1, Some("a"), Some("renamed_a"))],
                new_lines: vec!["renamed_a".to_string()],
            },
        ];

        let output = format_rename_result(&response, &file_diffs, OutputFormat::Text);
        let a_pos = output.find("File: /workspace/src/a.rs").unwrap();
        let z_pos = output.find("File: /workspace/src/z.rs").unwrap();

        assert!(output.starts_with("Renamed in 2 file(s), 2 edit(s)"));
        assert!(a_pos < z_pos);
    }

    #[test]
    fn format_rename_result_avoids_git_specific_headers() {
        let response = RenameResponse {
            changes: vec![FileEdit {
                file_path: "/workspace/src/main.rs".to_string(),
                edits: vec![make_text_edit(0, 0, 0, 1, "renamed")],
            }],
        };
        let file_diffs = vec![RenameFileDiff {
            file_path: "/workspace/src/main.rs".to_string(),
            edit_count: 1,
            diffs: vec![make_line_diff(1, Some("a"), Some("renamed"))],
            new_lines: vec!["renamed".to_string()],
        }];

        let output = format_rename_result(&response, &file_diffs, OutputFormat::Text);

        assert!(output.contains("File: /workspace/src/main.rs"));
        assert!(!output.contains("diff --git"));
        assert!(!output.contains("\n--- "));
        assert!(!output.contains("\n+++ "));
    }
}
