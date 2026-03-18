//! Batch editing support for aifed.
//!
//! This module provides batch operation parsing and execution, allowing multiple
//! edits to be applied in a single invocation.

use std::path::Path;

use crate::file::write_file;

use crate::commands::edit::{Operation, apply_operation, validate_operation};
use crate::error::{Error, Result};
use crate::locator::Locator;
use crate::output::{BatchResult, OutputFormat, format_batch_result};

/// Parsed single operation from batch input
#[derive(Debug, Clone)]
pub struct EditOp {
    pub operation: Operation,
    pub locator: Locator,
    pub content: Option<String>,
}

/// Parse operations from string (heredoc/file content)
///
/// Format:
/// ```text
/// <OP> <LOCATOR> [<CONTENT>]
///
/// OP:       = | + | -
/// LOCATOR:  LINE:HASH (e.g., "42:AB") or "0:00"
/// CONTENT:  Quoted string (supports escapes) or unquoted (no spaces)
///
/// # Comments and blank lines are ignored
/// ```
pub fn parse_batch_operations(input: &str) -> Result<Vec<EditOp>> {
    let mut operations = Vec::new();

    for (line_idx, line) in input.lines().enumerate() {
        let line_num = line_idx + 1; // 1-based line numbering
        let trimmed = line.trim();

        // Skip empty lines and comments
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let op = parse_single_operation(line_num, trimmed)?;
        operations.push(op);
    }

    Ok(operations)
}

/// Parse a single operation line
fn parse_single_operation(line_num: usize, line: &str) -> Result<EditOp> {
    // Split into parts: OP LOCATOR [CONTENT]
    let parts: Vec<&str> = split_operation_line(line);

    if parts.is_empty() {
        return Err(Error::InvalidBatchOp {
            line_number: line_num,
            line_content: line.to_string(),
            reason: "Empty operation".to_string(),
        });
    }

    if parts.len() < 2 {
        return Err(Error::InvalidBatchOp {
            line_number: line_num,
            line_content: line.to_string(),
            reason: "Missing locator. Format: OP LOCATOR [CONTENT]".to_string(),
        });
    }

    let operation_str = parts[0];
    let locator_str = parts[1];

    // Parse operation
    let operation = Operation::parse(operation_str).map_err(|e| Error::InvalidBatchOp {
        line_number: line_num,
        line_content: line.to_string(),
        reason: e.to_string(),
    })?;

    // Parse locator
    let locator = Locator::parse(locator_str).map_err(|e| Error::InvalidBatchOp {
        line_number: line_num,
        line_content: line.to_string(),
        reason: e.to_string(),
    })?;

    // Extract content
    let content = if parts.len() > 2 {
        // Join remaining parts as content, handling quoted strings
        Some(extract_content(&parts[2..]))
    } else {
        None
    };

    // Validate content requirement
    match operation {
        Operation::Replace | Operation::Insert => {
            if content.is_none() {
                return Err(Error::InvalidBatchOp {
                    line_number: line_num,
                    line_content: line.to_string(),
                    reason: "Content is required for replace (=) and insert (+) operations"
                        .to_string(),
                });
            }
        }
        Operation::Delete => {}
    }

    Ok(EditOp { operation, locator, content })
}

/// Split operation line into parts, respecting quoted strings
fn split_operation_line(line: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut current_start = None;
    let mut in_quotes = false;
    let mut quote_char = ' ';
    let chars = line.char_indices();

    for (idx, ch) in chars {
        if in_quotes {
            if ch == quote_char {
                // End of quoted string
                in_quotes = false;
                // Include the content without quotes
                if let Some(start) = current_start {
                    if start + 1 < idx {
                        parts.push(&line[start + 1..idx]);
                    } else {
                        parts.push(""); // Empty quoted string
                    }
                }
                current_start = None;
            }
        } else if ch == '"' || ch == '\'' {
            // Start of quoted string
            in_quotes = true;
            quote_char = ch;
            current_start = Some(idx);
        } else if ch.is_whitespace() {
            if let Some(start) = current_start {
                parts.push(&line[start..idx]);
                current_start = None;
            }
        } else if current_start.is_none() {
            current_start = Some(idx);
        }
    }

    // Handle remaining part
    if let Some(start) = current_start {
        parts.push(&line[start..]);
    }

    parts
}

/// Extract content from parts, handling escape sequences in quoted strings
fn extract_content(parts: &[&str]) -> String {
    if parts.is_empty() {
        return String::new();
    }

    // If first part was a quoted string, it's already processed
    // Just join parts with spaces for unquoted content
    parts.join(" ")
}

/// Execute batch operations
///
/// All operations must succeed, or none are applied (atomic).
pub fn execute_batch(
    path: &Path,
    operations: Vec<EditOp>,
    dry_run: bool,
    format: OutputFormat,
) -> Result<()> {
    if !path.exists() {
        return Err(Error::FileNotFound { path: path.to_path_buf() });
    }

    if operations.is_empty() {
        // Empty batch is a no-op success
        let result = BatchResult {
            success: true,
            total: 0,
            successful: 0,
            failed: 0,
            message: if dry_run {
                format!("No operations to apply to {}", path.display())
            } else {
                format!("No operations applied to {}", path.display())
            },
            changes: Vec::new(),
            errors: Vec::new(),
        };
        println!("{}", format_batch_result(&result, format));
        return Ok(());
    }

    let file_content = std::fs::read_to_string(path)
        .map_err(|e| Error::InvalidIo { path: path.to_path_buf(), source: e })?;

    let original_had_trailing_newline = file_content.ends_with('\n');
    let mut lines: Vec<String> = file_content.lines().map(|s| s.to_string()).collect();

    execute_atomic(path, &mut lines, operations, dry_run, format, original_had_trailing_newline)
}

/// Execute in atomic mode: validate all, then apply all
fn execute_atomic(
    path: &Path,
    lines: &mut Vec<String>,
    operations: Vec<EditOp>,
    dry_run: bool,
    format: OutputFormat,
    trailing_newline: bool,
) -> Result<()> {
    // Phase 1: Validate all operations
    let mut validated_ops = Vec::with_capacity(operations.len());
    for (idx, op) in operations.iter().enumerate() {
        let content_str = op.content.as_ref().map(|c| format!("\"{}\"", c)).unwrap_or_default();
        let validated =
            validate_operation(lines, op.operation, &op.locator, op.content.as_deref(), path)
                .map_err(|e| Error::InvalidBatchOp {
                    line_number: idx + 1,
                    line_content: format!("{} {} {}", op.operation_str(), op.locator, content_str),
                    reason: e.to_string(),
                })?;
        validated_ops.push(validated);
    }

    // Phase 2: Apply all operations
    let mut changes = Vec::with_capacity(operations.len());
    for validated in validated_ops {
        let change = apply_operation(lines, validated);
        changes.push(change);
    }

    // Phase 3: Write file
    if !dry_run {
        write_file(path, lines, trailing_newline)?;
    }

    let result = BatchResult {
        success: true,
        total: operations.len(),
        successful: operations.len(),
        failed: 0,
        message: if dry_run {
            format!("Would apply {} operations to {}", operations.len(), path.display())
        } else {
            format!("Applied {} operations to {}", operations.len(), path.display())
        },
        changes,
        errors: Vec::new(),
    };

    println!("{}", format_batch_result(&result, format));
    Ok(())
}

impl EditOp {
    /// Get string representation of the operation
    pub fn operation_str(&self) -> String {
        match self.operation {
            Operation::Replace => "=".to_string(),
            Operation::Insert => "+".to_string(),
            Operation::Delete => "-".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_operations() {
        let input = r#"
= 42:AB "new content"
+ 10:3K "inserted line"
- 15:7M
"#;
        let ops = parse_batch_operations(input).unwrap();
        assert_eq!(ops.len(), 3);
        assert_eq!(ops[0].operation, Operation::Replace);
        assert_eq!(ops[1].operation, Operation::Insert);
        assert_eq!(ops[2].operation, Operation::Delete);
    }

    #[test]
    fn test_parse_skip_comments_and_blanks() {
        let input = r#"
# This is a comment
= 42:AB "content"

# Another comment
+ 10:3K "more"
"#;
        let ops = parse_batch_operations(input).unwrap();
        assert_eq!(ops.len(), 2);
    }

    #[test]
    fn test_parse_unquoted_content() {
        let input = "= 42:AB simple_content";
        let ops = parse_batch_operations(input).unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].content, Some("simple_content".to_string()));
    }

    #[test]
    fn test_parse_missing_content_for_replace() {
        let input = "= 42:AB";
        let result = parse_batch_operations(input);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_invalid_operation() {
        let input = "* 42:AB content";
        let result = parse_batch_operations(input);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_invalid_locator() {
        let input = "= invalid content";
        let result = parse_batch_operations(input);
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_batch() {
        let input = "";
        let ops = parse_batch_operations(input).unwrap();
        assert!(ops.is_empty());
    }

    #[test]
    fn test_only_comments() {
        let input = "# comment\n# another\n";
        let ops = parse_batch_operations(input).unwrap();
        assert!(ops.is_empty());
    }

    #[test]
    fn test_parse_empty_quoted_string() {
        let input = r#"= 42:AB """#;
        let ops = parse_batch_operations(input).unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].content, Some("".to_string()));
    }

    #[test]
    fn test_parse_quoted_with_spaces() {
        let input = r#"= 42:AB "content with spaces""#;
        let ops = parse_batch_operations(input).unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].content, Some("content with spaces".to_string()));
    }

    #[test]
    fn test_write_file_preserves_trailing_newline() {
        use std::io::Read;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.txt");

        // Test WITH trailing newline
        let lines: Vec<String> = ["line1", "line2"].into_iter().map(String::from).collect();
        write_file(&path, &lines, true).unwrap();
        let mut content = String::new();
        std::fs::File::open(&path).unwrap().read_to_string(&mut content).unwrap();
        assert_eq!(content, "line1\nline2\n");

        // Test WITHOUT trailing newline
        write_file(&path, &lines, false).unwrap();
        let mut content = String::new();
        std::fs::File::open(&path).unwrap().read_to_string(&mut content).unwrap();
        assert_eq!(content, "line1\nline2");
    }

    #[test]
    fn test_write_file_preserves_multiple_trailing_newlines() {
        use std::io::Read;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.txt");

        // Multiple trailing newlines are represented as empty strings in lines
        let lines: Vec<String> = ["line1", "line2", "", ""].into_iter().map(String::from).collect();
        write_file(&path, &lines, true).unwrap();

        let mut content = String::new();
        std::fs::File::open(&path).unwrap().read_to_string(&mut content).unwrap();
        // line1\n + line2\n + \n + \n + trailing\n = line1\nline2\n\n\n
        assert_eq!(content, "line1\nline2\n\n\n");
    }
}
