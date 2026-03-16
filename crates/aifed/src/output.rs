use serde::Serialize;

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
    pub hash: String,
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

/// Format hashed lines for output
pub fn format_lines(lines: &[HashedLine], format: OutputFormat, no_hashes: bool) -> String {
    match format {
        OutputFormat::Text => {
            if no_hashes {
                lines.iter().map(|l| l.content.clone()).collect::<Vec<_>>().join("\n")
            } else {
                lines
                    .iter()
                    .map(|l| format!("{}:{}|{}", l.line, l.hash, l.content))
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        }
        OutputFormat::Json => {
            if no_hashes {
                #[derive(Serialize)]
                struct Output {
                    lines: Vec<OutputLine>,
                }
                #[derive(Serialize)]
                struct OutputLine {
                    line: usize,
                    content: String,
                }
                let output = Output {
                    lines: lines
                        .iter()
                        .map(|l| OutputLine { line: l.line, content: l.content.clone() })
                        .collect(),
                };
                serde_json::to_string_pretty(&output).unwrap_or_default()
            } else {
                #[derive(Serialize)]
                struct Output {
                    lines: Vec<HashedLine>,
                }
                let output = Output { lines: lines.to_vec() };
                serde_json::to_string_pretty(&output).unwrap_or_default()
            }
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
