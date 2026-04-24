//! Batch editing support for aifed.
//!
//! This module provides batch operation parsing and execution, allowing multiple
//! edits to be applied in a single invocation.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::file::write_file;

use crate::error::{Error, Result};
use crate::hash::{hash_file, hash_line, is_virtual_hash};
use crate::locator::Locator;
use crate::output::{BatchResult, EditChange, OutputFormat, format_batch_result_with_diff};
use aifed_common::LineDiffDto;
use aifed_daemon_client::DaemonClient;

/// Edit operation types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Operation {
    Insert,
    Delete,
    Replace,
}

impl Operation {
    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "+" => Ok(Operation::Insert),
            "-" => Ok(Operation::Delete),
            "=" => Ok(Operation::Replace),
            _ => Err(Error::InvalidOperation { input: s.to_string() }),
        }
    }
}

/// Validated operation ready to apply
#[derive(Debug)]
pub struct ValidatedOp {
    pub operation: Operation,
    pub target_line: usize,
    pub new_contents: Vec<String>,
}

/// Validate an operation against the current file state
pub fn validate_operation(
    lines: &[String],
    operation: Operation,
    locator: &Locator,
    contents: &[String],
    path: &Path,
) -> Result<ValidatedOp> {
    // Validate content requirement
    match operation {
        Operation::Insert | Operation::Replace => {
            if contents.is_empty() {
                return Err(Error::InvalidLocator {
                    input: "".to_string(),
                    reason: "Content is required for insert and replace operations".to_string(),
                });
            }
        }
        Operation::Delete => {}
    }

    // Handle virtual line (insert at beginning)
    if locator.is_virtual() {
        if operation != Operation::Insert {
            return Err(Error::InvalidLocator {
                input: locator.to_string(),
                reason: "Virtual line (0:00) is only valid for insert operations".to_string(),
            });
        }

        return Ok(ValidatedOp { operation, target_line: 0, new_contents: contents.to_vec() });
    }

    // Get line number and hash from locator
    let (target_line, expected_hash) = match locator {
        Locator::Hashline { line, hash } => (*line, Some(hash.clone())),
        Locator::Line(line) => (*line, None),
        Locator::LineRange { .. } => {
            return Err(Error::InvalidLocator {
                input: locator.to_string(),
                reason: "Range locators not supported for edit operations".to_string(),
            });
        }
        Locator::HashlineRange { .. } => {
            return Err(Error::InvalidLocator {
                input: locator.to_string(),
                reason: "Hashline range only supported for delete operations".to_string(),
            });
        }
    };

    // Validate line number
    if target_line == 0 || target_line > lines.len() {
        return Err(Error::InvalidLocator {
            input: locator.to_string(),
            reason: format!("Line {} out of range (1-{})", target_line, lines.len()),
        });
    }

    // Get actual content and hash
    let actual_content = lines[target_line - 1].clone();
    let actual_hash = hash_line(&actual_content);

    // Verify hash if provided
    if let Some(expected) = &expected_hash
        && !is_virtual_hash(expected)
        && actual_hash != *expected
    {
        return Err(Error::HashMismatch {
            path: path.to_path_buf(),
            line: target_line,
            expected: expected.clone(),
            actual: actual_hash,
            actual_content,
        });
    }

    Ok(ValidatedOp { operation, target_line, new_contents: contents.to_vec() })
}

/// Parsed single operation from batch input
#[derive(Debug, Clone)]
pub struct EditOp {
    pub operation: Operation,
    pub locator: Locator,
    pub contents: Vec<String>,
}

/// Edit plan that records all modifications based on original line numbers.
///
/// All operations reference the original file state, avoiding index shift issues
/// that occur with sequential modifications.
struct EditPlan {
    /// Lines to delete (1-based line numbers from original file)
    deletions: HashSet<usize>,
    /// Line number -> content to insert after that line (1-based from original file)
    inserts: HashMap<usize, Vec<String>>,
    /// Content to insert at the beginning of the file (virtual line 0)
    inserts_at_start: Vec<String>,
}

impl EditPlan {
    fn new() -> Self {
        Self { deletions: HashSet::new(), inserts: HashMap::new(), inserts_at_start: Vec::new() }
    }

    /// Add a validated operation to the plan.
    fn add(&mut self, validated: ValidatedOp) {
        match validated.operation {
            Operation::Delete => {
                self.deletions.insert(validated.target_line);
            }
            Operation::Replace => {
                self.deletions.insert(validated.target_line);
                self.inserts
                    .entry(validated.target_line)
                    .or_default()
                    .extend(validated.new_contents);
            }
            Operation::Insert => {
                if validated.target_line == 0 {
                    self.inserts_at_start.extend(validated.new_contents);
                } else {
                    self.inserts
                        .entry(validated.target_line)
                        .or_default()
                        .extend(validated.new_contents);
                }
            }
        }
    }

    /// Apply the plan to build new file content.
    ///
    /// Returns the new lines and a list of changes for output.
    fn apply(&self, original_lines: &[String]) -> (Vec<String>, Vec<EditChange>) {
        let mut new_lines = Vec::new();
        let mut changes = Vec::new();

        // Process inserts at the beginning of file (virtual line 0)
        for content in &self.inserts_at_start {
            new_lines.push(content.clone());
            changes.push(EditChange {
                operation: "insert".to_string(),
                line: new_lines.len(), // Actual line number in new file
                old_content: None,
                new_content: Some(content.clone()),
            });
        }

        // Iterate through original file lines
        for (i, original) in original_lines.iter().enumerate() {
            let line_num = i + 1; // 1-based line number

            // Process the current line first
            if self.deletions.contains(&line_num) {
                // Delete: skip this line, record the change
                changes.push(EditChange {
                    operation: "delete".to_string(),
                    line: line_num,
                    old_content: Some(original.clone()),
                    new_content: None,
                });
            } else {
                // Keep original line
                new_lines.push(original.clone());
            }

            // Then process inserts after this line
            if let Some(insert_contents) = self.inserts.get(&line_num) {
                for content in insert_contents {
                    new_lines.push(content.clone());
                    changes.push(EditChange {
                        operation: "insert".to_string(),
                        line: new_lines.len(), // The actual line number of the new line
                        old_content: None,
                        new_content: Some(content.clone()),
                    });
                }
            }
        }

        (new_lines, changes)
    }
}

/// Parse operations from string (heredoc/file content)
///
/// Format:
/// ```text
/// <OP> <LOCATOR> [<CONTENT>]
///
/// OP:       + | - | =
/// LOCATOR:  LINE:HASH (e.g., "42:AB") or "0:00"
/// CONTENT:  One or more quoted strings (supports escapes) or unquoted text
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

/// Parse a single operation line.
fn parse_single_operation(line_num: usize, line: &str) -> Result<EditOp> {
    // Split into parts: OP LOCATOR [CONTENT]
    let parts = split_operation_line(line).map_err(|e| Error::InvalidBatchOp {
        line_number: line_num,
        line_content: line.to_string(),
        reason: e.to_string(),
    })?;

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

    let operation_str = parts[0].raw;
    let locator_str = parts[1].raw;

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

    // Validate: edit operations require hash verification
    if let Locator::Line(_) | Locator::LineRange { .. } = &locator {
        return Err(Error::InvalidBatchOp {
            line_number: line_num,
            line_content: line.to_string(),
            reason: "Locator must include hash (e.g., \"42:AB\" or \"[2:AA,5:BB]\")".to_string(),
        });
    }

    // Extract content
    let contents = parse_contents(&parts[2..]).map_err(|e| Error::InvalidBatchOp {
        line_number: line_num,
        line_content: line.to_string(),
        reason: e.to_string(),
    })?;

    // Validate content requirement
    match operation {
        Operation::Insert | Operation::Replace => {
            if contents.is_empty() {
                return Err(Error::InvalidBatchOp {
                    line_number: line_num,
                    line_content: line.to_string(),
                    reason: "Content is required for insert (+) and replace (=) operations"
                        .to_string(),
                });
            }
        }
        Operation::Delete => {}
    }

    Ok(EditOp { operation, locator, contents })
}

#[derive(Clone, Copy)]
struct ParsedToken<'a> {
    raw: &'a str,
    quoted: bool,
}

/// Split operation line into parts, respecting quoted strings with escape support.
fn split_operation_line(line: &str) -> Result<Vec<ParsedToken<'_>>> {
    let mut parts = Vec::new();
    let mut current_start = None;
    let mut in_quotes = false;
    let mut idx = 0;

    while idx < line.len() {
        let ch = line[idx..].chars().next().unwrap();

        if in_quotes {
            if ch == '\\' {
                // Skip next character (escape sequence)
                idx += ch.len_utf8();
                if idx < line.len() {
                    idx += line[idx..].chars().next().unwrap().len_utf8();
                }
                continue;
            } else if ch == '"' {
                // End of quoted string - extract content without quotes
                if let Some(start) = current_start {
                    parts.push(ParsedToken { raw: &line[start + 1..idx], quoted: true });
                }
                in_quotes = false;
                current_start = None;
                idx += ch.len_utf8();
                continue;
            }
        } else if ch == '"' {
            // Start of quoted string
            in_quotes = true;
            current_start = Some(idx);
            idx += ch.len_utf8();
            continue;
        } else if ch.is_whitespace() {
            if let Some(start) = current_start {
                parts.push(ParsedToken { raw: &line[start..idx], quoted: false });
                current_start = None;
            }
            idx += ch.len_utf8();
            continue;
        } else if current_start.is_none() {
            current_start = Some(idx);
        }

        idx += ch.len_utf8();
    }

    // Check for unterminated string
    if in_quotes {
        return Err(Error::UnterminatedString);
    }

    // Handle remaining unquoted part
    if let Some(start) = current_start {
        parts.push(ParsedToken { raw: &line[start..], quoted: false });
    }

    Ok(parts)
}

fn parse_contents(parts: &[ParsedToken<'_>]) -> Result<Vec<String>> {
    if parts.is_empty() {
        return Ok(Vec::new());
    }

    if parts.len() > 1 && parts.iter().all(|part| part.quoted) {
        return parts.iter().map(|part| decode_content(part.raw)).collect();
    }

    let decoded = parts
        .iter()
        .map(|part| decode_content(part.raw))
        .collect::<Result<Vec<_>>>()?;
    let joined = decoded.join(" ");
    validate_content(&joined)?;
    Ok(vec![joined])
}

fn decode_content(raw: &str) -> Result<String> {
    match json_escape::unescape(&raw).decode_utf8() {
        Ok(cow) => {
            let content = cow.into_owned();
            validate_content(&content)?;
            Ok(content)
        }
        Err(e) => Err(Error::InvalidEscape { sequence: raw.to_string(), reason: e.to_string() }),
    }
}

fn validate_content(content: &str) -> Result<()> {
    if content.contains('\n') {
        return Err(Error::InvalidLocator {
            input: content.to_string(),
            reason: "Content must not contain newline (\\n). Each edit operation represents a single line."
                .to_string(),
        });
    }
    Ok(())
}

/// Execute batch operations
///
/// All operations must succeed, or none are applied (atomic).
pub async fn execute_batch(
    path: &Path,
    operations: Vec<EditOp>,
    dry_run: bool,
    format: OutputFormat,
    daemon_client: Option<&DaemonClient>,
) -> Result<()> {
    if !path.exists() {
        return Err(Error::FileNotFound {
            path: crate::file::to_absolute(path),
            cwd: std::env::current_dir().unwrap_or_default(),
        });
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
            new_lines: Vec::new(),
            changes: Vec::new(),
            errors: Vec::new(),
        };
        // Empty batch has no original lines to show in diff
        let empty_lines: Vec<String> = Vec::new();
        println!(
            "{}",
            format_batch_result_with_diff(&result, format, &empty_lines)
        );
        return Ok(());
    }

    let file_content = crate::file::read_text_file(path)?;

    // Compute hash before edit
    let expected_hash = hash_file(file_content.as_bytes());

    let lines = crate::file::split_lines_owned(&file_content);

    execute_atomic(
        path,
        &lines,
        operations,
        dry_run,
        format,
        daemon_client,
        &expected_hash,
    )
    .await
}

/// Execute in atomic mode: validate all, build edit plan, then apply all
#[allow(clippy::too_many_arguments)]
async fn execute_atomic(
    path: &Path,
    lines: &[String],
    operations: Vec<EditOp>,
    dry_run: bool,
    format: OutputFormat,
    daemon_client: Option<&DaemonClient>,
    expected_hash: &str,
) -> Result<()> {
    // Phase 1: Validate all operations and build edit plan
    let mut plan = EditPlan::new();
    for (idx, op) in operations.iter().enumerate() {
        // Handle HashlineRange by expanding into individual delete operations
        if let Locator::HashlineRange { start, start_hash, end, end_hash } = &op.locator {
            // Only Delete operation is supported for HashlineRange
            if op.operation != Operation::Delete {
                return Err(Error::InvalidBatchOp {
                    line_number: idx + 1,
                    line_content: format!("{} {}", op.operation_str(), op.locator),
                    reason: "HashlineRange only supported for delete operations".to_string(),
                });
            }

            // Validate start and end line numbers
            if *start == 0 || *start > lines.len() {
                return Err(Error::InvalidBatchOp {
                    line_number: idx + 1,
                    line_content: op.locator.to_string(),
                    reason: format!("Range start {} out of bounds (1-{})", start, lines.len()),
                });
            }
            if *end > lines.len() {
                return Err(Error::InvalidBatchOp {
                    line_number: idx + 1,
                    line_content: op.locator.to_string(),
                    reason: format!("Range end {} out of bounds (1-{})", end, lines.len()),
                });
            }

            // Verify start hash
            let actual_start_hash = crate::hash::hash_line(&lines[start - 1]);
            if actual_start_hash != *start_hash && !crate::hash::is_virtual_hash(start_hash) {
                return Err(Error::InvalidBatchOp {
                    line_number: idx + 1,
                    line_content: op.locator.to_string(),
                    reason: format!(
                        "Hash mismatch at line {}: expected {}, got {}",
                        start, start_hash, actual_start_hash
                    ),
                });
            }

            // Verify end hash
            let actual_end_hash = crate::hash::hash_line(&lines[end - 1]);
            if actual_end_hash != *end_hash && !crate::hash::is_virtual_hash(end_hash) {
                return Err(Error::InvalidBatchOp {
                    line_number: idx + 1,
                    line_content: op.locator.to_string(),
                    reason: format!(
                        "Hash mismatch at line {}: expected {}, got {}",
                        end, end_hash, actual_end_hash
                    ),
                });
            }

            // Add all lines in range to the deletion set
            for line_num in *start..=*end {
                plan.deletions.insert(line_num);
            }
        } else {
            // Standard single-line operation
            let validated =
                validate_operation(lines, op.operation, &op.locator, &op.contents, path).map_err(
                    |e| Error::InvalidBatchOp {
                        line_number: idx + 1,
                        line_content: op.to_string(),
                        reason: e.to_string(),
                    },
                )?;

            plan.add(validated);
        }
    }

    // Phase 2: Apply the edit plan to build new content
    let (new_lines, changes) = plan.apply(lines);

    // Phase 3: Write file
    if !dry_run {
        write_file(path, &new_lines)?;

        // Record edit with daemon (for history tracking)
        if let Some(client) = daemon_client {
            // Compute new hash after edit (must match what write_file writes to disk)
            let new_content = new_lines.join("\n");
            let new_hash = crate::hash::hash_file(new_content.as_bytes());

            // Convert changes to LineDiffDto
            let diffs: Vec<LineDiffDto> = changes
                .iter()
                .map(|c| LineDiffDto {
                    line_num: c.line,
                    old_hash: None, // We don't track line hashes in history
                    old_content: c.old_content.clone(),
                    new_content: c.new_content.clone(),
                })
                .collect();

            // Use canonical path to ensure consistency with daemon
            let canonical = path
                .canonicalize()
                .map_err(|e| Error::InvalidIo { path: path.to_path_buf(), source: e })?;
            let file_str = canonical.to_string_lossy().to_string();
            let _ = client
                .record_edit(&file_str, expected_hash, &new_hash, diffs)
                .await;
        }
    }

    let result = BatchResult {
        success: true,
        total: operations.len(),
        successful: operations.len(),
        failed: 0,
        message: if dry_run {
            format!(
                "Would apply {} operations to {}",
                operations.len(),
                path.display()
            )
        } else {
            format!(
                "Applied {} operations to {}",
                operations.len(),
                path.display()
            )
        },
        new_lines,
        changes,
        errors: Vec::new(),
    };

    println!(
        "{}",
        format_batch_result_with_diff(&result, format, &result.new_lines)
    );
    Ok(())
}

impl EditOp {
    /// Get string representation of the operation
    pub fn operation_str(&self) -> String {
        match self.operation {
            Operation::Insert => "+".to_string(),
            Operation::Delete => "-".to_string(),
            Operation::Replace => "=".to_string(),
        }
    }
}

impl std::fmt::Display for EditOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.operation_str(), self.locator)?;
        for content in &self.contents {
            write!(f, " \"{}\"", escape_content(content))?;
        }
        Ok(())
    }
}

fn escape_content(content: &str) -> String {
    content
        .replace('\\', "\\\\")
        .replace('\"', "\\\"")
        .replace('\t', "\\t")
        .replace('\r', "\\r")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_operations() {
        let input = r#"
+ 42:AB "new content"
- 15:7M
= 10:3K "replacement"
"#;
        let ops = parse_batch_operations(input).unwrap();
        assert_eq!(ops.len(), 3);
        assert_eq!(ops[0].operation, Operation::Insert);
        assert_eq!(ops[0].contents, vec!["new content"]);
        assert_eq!(ops[1].operation, Operation::Delete);
        assert_eq!(ops[2].operation, Operation::Replace);
        assert_eq!(ops[2].contents, vec!["replacement"]);
    }

    #[test]
    fn test_parse_skip_comments_and_blanks() {
        let input = r#"
# This is a comment
+ 42:AB "content"

# Another comment
+ 10:3K "more"
"#;
        let ops = parse_batch_operations(input).unwrap();
        assert_eq!(ops.len(), 2);
    }

    #[test]
    fn test_parse_unquoted_content() {
        let input = "+ 42:AB simple_content";
        let ops = parse_batch_operations(input).unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].contents, vec!["simple_content"]);
    }

    #[test]
    fn test_parse_unquoted_tokens_joined_into_single_content() {
        let input = "+ 42:AB content with spaces";
        let ops = parse_batch_operations(input).unwrap();
        assert_eq!(ops[0].contents, vec!["content with spaces"]);
    }

    #[test]
    fn test_parse_missing_content_for_insert() {
        let input = "+ 42:AB";
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
        let input = "+ invalid content";
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
        let input = r#"+ 42:AB """#;
        let ops = parse_batch_operations(input).unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].contents, vec![""]);
    }

    #[test]
    fn test_parse_quoted_with_spaces() {
        let input = r#"+ 42:AB "content with spaces""#;
        let ops = parse_batch_operations(input).unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].contents, vec!["content with spaces"]);
    }

    #[test]
    fn test_parse_multiple_quoted_contents() {
        let input = r#"+ 42:AB "a" "b" "c""#;
        let ops = parse_batch_operations(input).unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].contents, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_write_file_preserves_trailing_newline() {
        use std::io::Read;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.txt");

        // Test WITH trailing newline (trailing empty string indicates newline)
        let lines: Vec<String> = ["line1", "line2", ""]
            .into_iter()
            .map(String::from)
            .collect();
        write_file(&path, &lines).unwrap();
        let mut content = String::new();
        std::fs::File::open(&path)
            .unwrap()
            .read_to_string(&mut content)
            .unwrap();
        assert_eq!(content, "line1\nline2\n");

        // Test WITHOUT trailing newline (no trailing empty string)
        let lines: Vec<String> = ["line1", "line2"].into_iter().map(String::from).collect();
        write_file(&path, &lines).unwrap();
        let mut content = String::new();
        std::fs::File::open(&path)
            .unwrap()
            .read_to_string(&mut content)
            .unwrap();
        assert_eq!(content, "line1\nline2");
    }

    #[test]
    fn test_write_file_preserves_multiple_trailing_newlines() {
        use std::io::Read;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.txt");

        // Multiple trailing newlines are represented as empty strings in lines
        // ["line1", "line2", "", ""] -> "line1\nline2\n\n"
        let lines: Vec<String> = ["line1", "line2", "", ""]
            .into_iter()
            .map(String::from)
            .collect();
        write_file(&path, &lines).unwrap();

        let mut content = String::new();
        std::fs::File::open(&path)
            .unwrap()
            .read_to_string(&mut content)
            .unwrap();
        assert_eq!(content, "line1\nline2\n\n");
    }

    #[test]
    fn test_edit_plan_delete() {
        // Delete lines 2 and 4 - order doesn't matter since all ops based on original
        let mut plan = EditPlan::new();
        plan.add(ValidatedOp {
            operation: Operation::Delete,
            target_line: 2,
            new_contents: vec![],
        });
        plan.add(ValidatedOp {
            operation: Operation::Delete,
            target_line: 4,
            new_contents: vec![],
        });

        let original: Vec<String> = ["1", "2", "3", "4", "5"]
            .into_iter()
            .map(String::from)
            .collect();
        let (new_lines, _) = plan.apply(&original);

        assert_eq!(new_lines, vec!["1", "3", "5"]);
    }

    #[test]
    fn test_edit_plan_insert_order() {
        // Insert a, b, c after line 1 - order should be preserved
        let mut plan = EditPlan::new();
        plan.add(ValidatedOp {
            operation: Operation::Insert,
            target_line: 1,
            new_contents: vec!["a".to_string()],
        });
        plan.add(ValidatedOp {
            operation: Operation::Insert,
            target_line: 1,
            new_contents: vec!["b".to_string()],
        });
        plan.add(ValidatedOp {
            operation: Operation::Insert,
            target_line: 1,
            new_contents: vec!["c".to_string()],
        });

        let original: Vec<String> = ["start"].into_iter().map(String::from).collect();
        let (new_lines, _) = plan.apply(&original);

        assert_eq!(new_lines, vec!["start", "a", "b", "c"]);
    }

    #[test]
    fn test_edit_plan_delete_and_insert_share_anchor() {
        // Delete and insert may share the same original anchor line.
        let mut plan = EditPlan::new();
        plan.add(ValidatedOp {
            operation: Operation::Delete,
            target_line: 3,
            new_contents: vec![],
        });
        plan.add(ValidatedOp {
            operation: Operation::Insert,
            target_line: 3,
            new_contents: vec!["new".to_string()],
        });

        let original: Vec<String> = ["1", "2", "3", "4"].into_iter().map(String::from).collect();
        let (new_lines, _) = plan.apply(&original);

        assert_eq!(new_lines, vec!["1", "2", "new", "4"]);
    }

    #[test]
    fn test_edit_plan_mixed() {
        // Mixed operations: delete lines 2-3, then insert after deleted line 3 and after line 1.
        let mut plan = EditPlan::new();
        plan.add(ValidatedOp {
            operation: Operation::Delete,
            target_line: 2,
            new_contents: vec![],
        });
        plan.add(ValidatedOp {
            operation: Operation::Delete,
            target_line: 3,
            new_contents: vec![],
        });
        plan.add(ValidatedOp {
            operation: Operation::Insert,
            target_line: 3,
            new_contents: vec!["NEW3".to_string()],
        });
        plan.add(ValidatedOp {
            operation: Operation::Insert,
            target_line: 1,
            new_contents: vec!["A".to_string()],
        });

        let original: Vec<String> = ["L1", "L2", "L3", "L4"]
            .into_iter()
            .map(String::from)
            .collect();
        let (new_lines, _) = plan.apply(&original);

        // L1, insert A after L1, skip deleted lines 2-3, insert NEW3 before L4.
        assert_eq!(new_lines, vec!["L1", "A", "NEW3", "L4"]);
    }

    #[test]
    fn test_edit_plan_replace_single_line() {
        let mut plan = EditPlan::new();
        plan.add(ValidatedOp {
            operation: Operation::Replace,
            target_line: 3,
            new_contents: vec!["new".to_string()],
        });

        let original: Vec<String> = ["1", "2", "3", "4"].into_iter().map(String::from).collect();
        let (new_lines, _) = plan.apply(&original);

        assert_eq!(new_lines, vec!["1", "2", "new", "4"]);
    }

    #[test]
    fn test_edit_plan_replace_multi_line() {
        let mut plan = EditPlan::new();
        plan.add(ValidatedOp {
            operation: Operation::Replace,
            target_line: 2,
            new_contents: vec!["a".to_string(), "b".to_string()],
        });

        let original: Vec<String> = ["1", "2", "3"].into_iter().map(String::from).collect();
        let (new_lines, _) = plan.apply(&original);

        assert_eq!(new_lines, vec!["1", "a", "b", "3"]);
    }

    #[test]
    fn test_edit_plan_insert_at_start() {
        // Insert at the beginning (virtual line 0)
        let mut plan = EditPlan::new();
        plan.add(ValidatedOp {
            operation: Operation::Insert,
            target_line: 0,
            new_contents: vec!["first".to_string()],
        });
        plan.add(ValidatedOp {
            operation: Operation::Insert,
            target_line: 0,
            new_contents: vec!["second".to_string()],
        });

        let original: Vec<String> = ["existing"].into_iter().map(String::from).collect();
        let (new_lines, _) = plan.apply(&original);

        assert_eq!(new_lines, vec!["first", "second", "existing"]);
    }

    #[tokio::test]
    async fn test_batch_insert_same_position() {
        // Integration test: insert multiple lines at the same position
        // Should preserve input order
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.txt");

        // Create a 1-line file with trailing newline
        let lines: Vec<String> = ["start", ""].into_iter().map(String::from).collect();
        write_file(&path, &lines).unwrap();

        // Read file content and hash
        let content = std::fs::read_to_string(&path).unwrap();
        let file_lines = crate::file::split_lines_owned(&content);
        let hash = crate::hash::hash_line(&file_lines[0]);

        // Parse insert operations at the same position (line 1)
        let input = format!(r#"+ 1:{hash} "a" "b" "c""#);
        let ops = parse_batch_operations(&input).unwrap();

        execute_batch(&path, ops, false, OutputFormat::Text, None)
            .await
            .unwrap();

        // Verify result: start, a, b, c (in that order) with trailing newline
        let result = std::fs::read_to_string(&path).unwrap();
        assert_eq!(result, "start\na\nb\nc\n");
    }

    #[tokio::test]
    async fn test_batch_delete_multiple_lines() {
        // Integration test: delete multiple lines without index shift issues
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.txt");

        // Create a 5-line file with trailing newline
        let lines: Vec<String> = ["1", "2", "3", "4", "5", ""]
            .into_iter()
            .map(String::from)
            .collect();
        write_file(&path, &lines).unwrap();

        // Read file content and get hashes
        let content = std::fs::read_to_string(&path).unwrap();
        let file_lines = crate::file::split_lines_owned(&content);
        let hash2 = crate::hash::hash_line(&file_lines[1]);
        let hash4 = crate::hash::hash_line(&file_lines[3]);

        // Delete lines 2 and 4
        let input = format!(
            r#"- 2:{hash2}
- 4:{hash4}
"#
        );
        let ops = parse_batch_operations(&input).unwrap();

        execute_batch(&path, ops, false, OutputFormat::Text, None)
            .await
            .unwrap();

        // Verify result: 1, 3, 5 with trailing newline
        let result = std::fs::read_to_string(&path).unwrap();
        assert_eq!(result, "1\n3\n5\n");
    }

    #[tokio::test]
    async fn test_batch_mixed_operations() {
        // Integration test: delete, replacement-via-delete+insert, and insert in one batch
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.txt");

        // Create a file with trailing newline
        let lines: Vec<String> = ["L1", "L2", "L3", "L4", "L5", ""]
            .into_iter()
            .map(String::from)
            .collect();
        write_file(&path, &lines).unwrap();

        // Read file content and get hashes
        let content = std::fs::read_to_string(&path).unwrap();
        let file_lines = crate::file::split_lines_owned(&content);
        let hash1 = crate::hash::hash_line(&file_lines[0]);
        let hash2 = crate::hash::hash_line(&file_lines[1]);
        let hash3 = crate::hash::hash_line(&file_lines[2]);
        // Delete L2, replace L3 with NEW3, insert A after L1
        let input = format!(
            r#"- 2:{hash2}
- 3:{hash3}
+ 3:{hash3} "NEW3"
+ 1:{hash1} "A"
"#
        );
        let ops = parse_batch_operations(&input).unwrap();

        execute_batch(&path, ops, false, OutputFormat::Text, None)
            .await
            .unwrap();

        // Verify result: L1, A, NEW3, L4, L5 with trailing newline
        let result = std::fs::read_to_string(&path).unwrap();
        assert_eq!(result, "L1\nA\nNEW3\nL4\nL5\n");
    }

    #[tokio::test]
    async fn test_batch_delete_then_replace() {
        // Verify replacement-via-delete+insert works correctly
        // without hash mismatch due to line number offset
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.txt");

        // Create a 4-line file with trailing newline
        let lines: Vec<String> = ["L1", "L2", "L3", "L4", ""]
            .into_iter()
            .map(String::from)
            .collect();
        write_file(&path, &lines).unwrap();

        // Read file content and get hashes
        let content = std::fs::read_to_string(&path).unwrap();
        let file_lines = crate::file::split_lines_owned(&content);
        let hash2 = crate::hash::hash_line(&file_lines[1]);
        let hash3 = crate::hash::hash_line(&file_lines[2]);

        // Delete line 2, replace line 3 (operations based on original line numbers)
        let input = format!(
            r#"- 2:{hash2}
- 3:{hash3}
+ 3:{hash3} "NEW3"
"#
        );
        let ops = parse_batch_operations(&input).unwrap();

        execute_batch(&path, ops, false, OutputFormat::Text, None)
            .await
            .unwrap();

        // Verify result: L1, NEW3, L4 with trailing newline
        let result = std::fs::read_to_string(&path).unwrap();
        assert_eq!(result, "L1\nNEW3\nL4\n");
    }

    #[tokio::test]
    async fn test_batch_replace_single_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.txt");

        let lines: Vec<String> = ["L1", "L2", "L3", ""]
            .into_iter()
            .map(String::from)
            .collect();
        write_file(&path, &lines).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let file_lines = crate::file::split_lines_owned(&content);
        let hash2 = crate::hash::hash_line(&file_lines[1]);

        let input = format!(r#"= 2:{hash2} "REPLACED""#);
        let ops = parse_batch_operations(&input).unwrap();

        execute_batch(&path, ops, false, OutputFormat::Text, None)
            .await
            .unwrap();

        let result = std::fs::read_to_string(&path).unwrap();
        assert_eq!(result, "L1\nREPLACED\nL3\n");
    }

    #[tokio::test]
    async fn test_batch_replace_multi_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.txt");

        let lines: Vec<String> = ["L1", "L2", "L3", ""]
            .into_iter()
            .map(String::from)
            .collect();
        write_file(&path, &lines).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let file_lines = crate::file::split_lines_owned(&content);
        let hash2 = crate::hash::hash_line(&file_lines[1]);

        let input = format!(r#"= 2:{hash2} "A" "B""#);
        let ops = parse_batch_operations(&input).unwrap();

        execute_batch(&path, ops, false, OutputFormat::Text, None)
            .await
            .unwrap();

        let result = std::fs::read_to_string(&path).unwrap();
        assert_eq!(result, "L1\nA\nB\nL3\n");
    }

    // ============================================================
    // Quote escaping tests - currently failing (documenting issues)
    // ============================================================

    #[test]
    fn test_parse_content_double_quote_inside_double() {
        // Double quote inside double-quoted content
        let input = r#"+ 10:AB "say \"hello\"""#;
        let ops = parse_batch_operations(input).unwrap();
        assert_eq!(ops[0].contents, vec![r#"say "hello""#]);
    }

    #[test]
    fn test_parse_content_backslash() {
        // Backslash escaping
        let input = r#"+ 10:AB "path\\to\\file""#;
        let ops = parse_batch_operations(input).unwrap();
        assert_eq!(ops[0].contents, vec![r#"path\to\file"#]);
    }

    #[test]
    fn test_parse_content_json_with_quotes() {
        // JSON string content
        let input = r#"+ 10:AB "{\"key\": \"value\"}""#;
        let ops = parse_batch_operations(input).unwrap();
        assert_eq!(ops[0].contents, vec![r#"{"key": "value"}"#]);
    }

    #[test]
    fn test_parse_content_rust_code() {
        // Rust code containing raw string literal
        let input = r##"+ 10:AB "let s = r#\"hello\"#;""##;
        let ops = parse_batch_operations(input).unwrap();
        assert_eq!(ops[0].contents, vec![r###"let s = r#"hello"#;"###]);
    }

    #[test]
    fn test_parse_content_newline_escape_rejected() {
        // Newline escape sequence should be rejected
        let input = r#"+ 10:AB "line1\nline2""#;
        let result = parse_batch_operations(input);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("newline"),
            "error should mention newline: {err}"
        );
    }

    #[test]
    fn test_parse_content_tab_escape() {
        // Tab escape sequence
        let input = r#"+ 10:AB "col1\tcol2""#;
        let ops = parse_batch_operations(input).unwrap();
        assert_eq!(ops[0].contents, vec!["col1\tcol2"]);
    }

    #[test]
    fn test_parse_content_cr_at_end_allowed() {
        // \r at end of line is allowed (CRLF support)
        let input = r#"+ 10:AB "line1\r""#;
        let ops = parse_batch_operations(input).unwrap();
        assert_eq!(ops[0].contents, vec!["line1\r"]);
    }

    #[test]
    fn test_parse_content_cr_in_middle_allowed() {
        // \r in the middle of content is allowed (no restriction on \r placement)
        // Only \n is rejected since lines are split on \n
        let input = r#"+ 10:AB "line1\rline2""#;
        let ops = parse_batch_operations(input).unwrap();
        assert_eq!(ops[0].contents, vec!["line1\rline2"]);
    }

    #[test]
    fn test_parse_content_invalid_escape() {
        // Invalid escape sequence should return error
        let input = r#"+ 10:AB "unknown \x escape""#;
        let result = parse_batch_operations(input);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_content_single_quote_no_escape_needed() {
        // Single quote inside double quotes doesn't need escaping
        let input = r#"+ 10:AB "it's""#;
        let ops = parse_batch_operations(input).unwrap();
        assert_eq!(ops[0].contents, vec!["it's"]);
    }

    #[test]
    fn test_parse_content_unterminated_string() {
        // Unterminated string should return error
        let input = r#"+ 10:AB "unterminated"#;
        let result = parse_batch_operations(input);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Unterminated string"));
    }

    // ============================================================
    // Replace (=) operator tests
    // ============================================================

    #[test]
    fn test_parse_replace_operation() {
        let input = r#"= 42:AB "new content""#;
        let ops = parse_batch_operations(input).unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].operation, Operation::Replace);
        assert_eq!(ops[0].contents, vec!["new content"]);
    }

    #[test]
    fn test_parse_replace_multi_line() {
        let input = r#"= 42:AB "line1" "line2""#;
        let ops = parse_batch_operations(input).unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].operation, Operation::Replace);
        assert_eq!(ops[0].contents, vec!["line1", "line2"]);
    }

    #[test]
    fn test_parse_replace_missing_content() {
        let input = "= 42:AB";
        let result = parse_batch_operations(input);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_batch_range_delete_allows_noncanonical_insert_anchor() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.txt");

        let mut lines: Vec<String> = (1..=6).map(|i| format!("L{}", i)).collect();
        lines.push("".to_string());
        write_file(&path, &lines).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let file_lines = crate::file::split_lines_owned(&content);
        let hash1 = crate::hash::hash_line(&file_lines[0]);
        let hash2 = crate::hash::hash_line(&file_lines[1]);
        let hash4 = crate::hash::hash_line(&file_lines[3]);

        let input = format!(
            r#"- [2:{hash2},4:{hash4}]
+ 1:{hash1} "A"
+ 2:{hash2} "B"
"#
        );
        let ops = parse_batch_operations(&input).unwrap();

        execute_batch(&path, ops, false, OutputFormat::Text, None)
            .await
            .unwrap();

        let result = std::fs::read_to_string(&path).unwrap();
        assert_eq!(result, "L1\nA\nB\nL5\nL6\n");
    }

    // ============================================================
    // HashlineRange delete tests
    // ============================================================

    #[tokio::test]
    async fn test_batch_range_delete() {
        // Delete lines 2-9 from a 10-line file, keeping L1 and L10
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.txt");

        // Create a 10-line file with trailing newline
        let mut lines: Vec<String> = (1..=10).map(|i| format!("L{}", i)).collect();
        lines.push("".to_string()); // trailing newline
        write_file(&path, &lines).unwrap();

        // Read file content and get hashes
        let content = std::fs::read_to_string(&path).unwrap();
        let file_lines = crate::file::split_lines_owned(&content);
        let hash2 = crate::hash::hash_line(&file_lines[1]); // line 2 (0-indexed)
        let hash9 = crate::hash::hash_line(&file_lines[8]); // line 9 (0-indexed)

        // Delete lines 2-9 using hashline range
        let input = format!("- [2:{hash2},9:{hash9}]");
        let ops = parse_batch_operations(&input).unwrap();

        execute_batch(&path, ops, false, OutputFormat::Text, None)
            .await
            .unwrap();

        // Verify result: L1, L10 with trailing newline
        let result = std::fs::read_to_string(&path).unwrap();
        assert_eq!(result, "L1\nL10\n");
    }

    #[tokio::test]
    async fn test_batch_range_delete_single_line() {
        // [3:hash3,3:hash3] - delete only one line
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.txt");

        let mut lines: Vec<String> = (1..=5).map(|i| format!("L{}", i)).collect();
        lines.push("".to_string()); // trailing newline
        write_file(&path, &lines).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let file_lines = crate::file::split_lines_owned(&content);
        let hash3 = crate::hash::hash_line(&file_lines[2]); // line 3 (0-indexed)

        let input = format!("- [3:{hash3},3:{hash3}]");
        let ops = parse_batch_operations(&input).unwrap();

        execute_batch(&path, ops, false, OutputFormat::Text, None)
            .await
            .unwrap();

        // Verify result: L1, L2, L4, L5 with trailing newline
        let result = std::fs::read_to_string(&path).unwrap();
        assert_eq!(result, "L1\nL2\nL4\nL5\n");
    }

    #[tokio::test]
    async fn test_batch_range_delete_hash_mismatch() {
        // Boundary line hash mismatch should fail
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.txt");

        let mut lines: Vec<String> = (1..=10).map(|i| format!("L{}", i)).collect();
        lines.push("".to_string()); // trailing newline
        write_file(&path, &lines).unwrap();

        // Use wrong hash (VV is valid format but wrong value)
        let input = "- [2:VV,9:VV]";
        let ops = parse_batch_operations(input).unwrap();

        let result = execute_batch(&path, ops, false, OutputFormat::Text, None).await;

        // Should fail with hash mismatch error
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("Hash mismatch") || err.contains("line"),
            "Error: {}",
            err
        );
    }
}
