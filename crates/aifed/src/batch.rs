//! Batch editing support for aifed.
//!
//! This module provides batch operation parsing and execution, allowing multiple
//! edits to be applied in a single invocation.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::file::write_file;

use crate::commands::edit::{Operation, ValidatedOp, validate_operation};
use crate::error::{Error, Result};
use crate::hash::hash_file;
use crate::locator::Locator;
use crate::output::{BatchResult, EditChange, OutputFormat, format_batch_result_with_diff};
use aifed_common::LineDiffDto;
use aifed_daemon_client::DaemonClient;

/// Parsed single operation from batch input
#[derive(Debug, Clone)]
pub struct EditOp {
    pub operation: Operation,
    pub locator: Locator,
    pub content: Option<String>,
}

/// Edit plan that records all modifications based on original line numbers.
///
/// All operations reference the original file state, avoiding index shift issues
/// that occur with sequential modifications.
struct EditPlan {
    /// Lines to delete (1-based line numbers from original file)
    deletions: HashSet<usize>,
    /// Line number -> replacement content (1-based from original file)
    replacements: HashMap<usize, String>,
    /// Line number -> content to insert after that line (1-based from original file)
    inserts: HashMap<usize, Vec<String>>,
    /// Content to insert at the beginning of the file (virtual line 0)
    inserts_at_start: Vec<String>,
}

impl EditPlan {
    fn new() -> Self {
        Self {
            deletions: HashSet::new(),
            replacements: HashMap::new(),
            inserts: HashMap::new(),
            inserts_at_start: Vec::new(),
        }
    }

    /// Add a validated operation to the plan.
    ///
    /// Returns an error if there's a conflict (e.g., same line both deleted and replaced).
    fn add(&mut self, validated: ValidatedOp) -> Result<()> {
        match validated.operation {
            Operation::Delete => {
                // Check conflict: cannot both delete and replace the same line
                if self.replacements.contains_key(&validated.target_line) {
                    return Err(Error::ConflictDeleteAndReplace(validated.target_line));
                }
                self.deletions.insert(validated.target_line);
            }
            Operation::Replace => {
                // Check conflict: cannot both delete and replace the same line
                if self.deletions.contains(&validated.target_line) {
                    return Err(Error::ConflictDeleteAndReplace(validated.target_line));
                }
                self.replacements.insert(validated.target_line, validated.new_content.unwrap());
            }
            Operation::Insert => {
                let content = validated.new_content.unwrap();
                if validated.target_line == 0 {
                    self.inserts_at_start.push(content);
                } else {
                    self.inserts.entry(validated.target_line).or_default().push(content);
                }
            }
        }
        Ok(())
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
            } else if let Some(new_content) = self.replacements.get(&line_num) {
                // Replace: output new content
                new_lines.push(new_content.clone());
                changes.push(EditChange {
                    operation: "replace".to_string(),
                    line: line_num,
                    old_content: Some(original.clone()),
                    new_content: Some(new_content.clone()),
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

    // Validate: edit operations require hash verification
    if let Locator::Line(_) | Locator::LineRange { .. } = &locator {
        return Err(Error::InvalidBatchOp {
            line_number: line_num,
            line_content: line.to_string(),
            reason: "Locator must include hash (e.g., \"42:AB\" or \"[2:AA,5:BB]\")".to_string(),
        });
    }

    // Extract content
    let content = if parts.len() > 2 {
        // Join remaining parts as content, handling quoted strings and escapes
        Some(extract_content(&parts[2..])?)
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

/// Split operation line into parts, respecting quoted strings with escape support
fn split_operation_line(line: &str) -> Result<Vec<&str>> {
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
                    parts.push(&line[start + 1..idx]);
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
                parts.push(&line[start..idx]);
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
        parts.push(&line[start..]);
    }

    Ok(parts)
}

/// Extract and unescape content from parts using JSON escape rules
fn extract_content(parts: &[&str]) -> Result<String> {
    if parts.is_empty() {
        return Ok(String::new());
    }

    let raw = parts.join(" ");

    // Use json-escape to unescape
    match json_escape::unescape(&raw).decode_utf8() {
        Ok(cow) => Ok(cow.into_owned()),
        Err(e) => Err(Error::InvalidEscape { sequence: raw, reason: e.to_string() }),
    }
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
            new_lines: Vec::new(),
            changes: Vec::new(),
            errors: Vec::new(),
        };
        // Empty batch has no original lines to show in diff
        let empty_lines: Vec<String> = Vec::new();
        println!("{}", format_batch_result_with_diff(&result, format, &empty_lines));
        return Ok(());
    }

    let file_content = std::fs::read_to_string(path)
        .map_err(|e| Error::InvalidIo { path: path.to_path_buf(), source: e })?;

    // Compute hash before edit
    let expected_hash = hash_file(file_content.as_bytes());

    let original_had_trailing_newline = file_content.ends_with('\n');
    let lines: Vec<String> = file_content.lines().map(|s| s.to_string()).collect();

    execute_atomic(
        path,
        &lines,
        operations,
        dry_run,
        format,
        original_had_trailing_newline,
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
    trailing_newline: bool,
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
                if plan.replacements.contains_key(&line_num) {
                    return Err(Error::ConflictDeleteAndReplace(line_num));
                }
                plan.deletions.insert(line_num);
            }
        } else {
            // Standard single-line operation
            let content_str = op.content.as_ref().map(|c| format!("\"{}\"", c)).unwrap_or_default();
            let validated =
                validate_operation(lines, op.operation, &op.locator, op.content.as_deref(), path)
                    .map_err(|e| Error::InvalidBatchOp {
                    line_number: idx + 1,
                    line_content: format!("{} {} {}", op.operation_str(), op.locator, content_str),
                    reason: e.to_string(),
                })?;

            // Add to plan (checks for conflicts)
            plan.add(validated)?;
        }
    }

    // Phase 2: Apply the edit plan to build new content
    let (new_lines, changes) = plan.apply(lines);

    // Phase 3: Write file
    if !dry_run {
        write_file(path, &new_lines, trailing_newline)?;

        // Record edit with daemon (for history tracking)
        if let Some(client) = daemon_client {
            // Compute new hash after edit (must match what write_file writes to disk)
            let mut new_content = new_lines.join("\n");
            if trailing_newline {
                new_content.push('\n');
            }
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
            let _ = client.record_edit(&file_str, expected_hash, &new_hash, diffs).await;
        }
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
        new_lines,
        changes,
        errors: Vec::new(),
    };

    println!("{}", format_batch_result_with_diff(&result, format, &result.new_lines));
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

    #[test]
    fn test_edit_plan_delete() {
        // Delete lines 2 and 4 - order doesn't matter since all ops based on original
        let mut plan = EditPlan::new();
        plan.add(ValidatedOp {
            operation: Operation::Delete,
            target_line: 2,
            old_content: Some("line2".to_string()),
            new_content: None,
        })
        .unwrap();
        plan.add(ValidatedOp {
            operation: Operation::Delete,
            target_line: 4,
            old_content: Some("line4".to_string()),
            new_content: None,
        })
        .unwrap();

        let original: Vec<String> =
            ["1", "2", "3", "4", "5"].into_iter().map(String::from).collect();
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
            old_content: None,
            new_content: Some("a".to_string()),
        })
        .unwrap();
        plan.add(ValidatedOp {
            operation: Operation::Insert,
            target_line: 1,
            old_content: None,
            new_content: Some("b".to_string()),
        })
        .unwrap();
        plan.add(ValidatedOp {
            operation: Operation::Insert,
            target_line: 1,
            old_content: None,
            new_content: Some("c".to_string()),
        })
        .unwrap();

        let original: Vec<String> = ["start"].into_iter().map(String::from).collect();
        let (new_lines, _) = plan.apply(&original);

        assert_eq!(new_lines, vec!["start", "a", "b", "c"]);
    }

    #[test]
    fn test_edit_plan_conflict() {
        // Same line cannot be both deleted and replaced
        let mut plan = EditPlan::new();
        plan.add(ValidatedOp {
            operation: Operation::Delete,
            target_line: 3,
            old_content: Some("line3".to_string()),
            new_content: None,
        })
        .unwrap();

        let result = plan.add(ValidatedOp {
            operation: Operation::Replace,
            target_line: 3,
            old_content: Some("line3".to_string()),
            new_content: Some("new".to_string()),
        });

        assert!(result.is_err());
    }

    #[test]
    fn test_edit_plan_mixed() {
        // Mixed operations: delete line 2, replace line 3, insert after line 1
        let mut plan = EditPlan::new();
        plan.add(ValidatedOp {
            operation: Operation::Delete,
            target_line: 2,
            old_content: Some("L2".to_string()),
            new_content: None,
        })
        .unwrap();
        plan.add(ValidatedOp {
            operation: Operation::Replace,
            target_line: 3,
            old_content: Some("L3".to_string()),
            new_content: Some("NEW3".to_string()),
        })
        .unwrap();
        plan.add(ValidatedOp {
            operation: Operation::Insert,
            target_line: 1,
            old_content: None,
            new_content: Some("A".to_string()),
        })
        .unwrap();

        let original: Vec<String> =
            ["L1", "L2", "L3", "L4"].into_iter().map(String::from).collect();
        let (new_lines, _) = plan.apply(&original);

        // L1, insert A after L1, skip L2 (deleted), replace L3 with NEW3, keep L4
        assert_eq!(new_lines, vec!["L1", "A", "NEW3", "L4"]);
    }

    #[test]
    fn test_edit_plan_insert_at_start() {
        // Insert at the beginning (virtual line 0)
        let mut plan = EditPlan::new();
        plan.add(ValidatedOp {
            operation: Operation::Insert,
            target_line: 0,
            old_content: None,
            new_content: Some("first".to_string()),
        })
        .unwrap();
        plan.add(ValidatedOp {
            operation: Operation::Insert,
            target_line: 0,
            old_content: None,
            new_content: Some("second".to_string()),
        })
        .unwrap();

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

        // Create a 1-line file
        let lines: Vec<String> = ["start"].into_iter().map(String::from).collect();
        write_file(&path, &lines, true).unwrap();

        // Read file content and hash
        let content = std::fs::read_to_string(&path).unwrap();
        let file_lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
        let hash = crate::hash::hash_line(&file_lines[0]);

        // Parse insert operations at the same position (line 1)
        let input = format!(
            r#"+ 1:{hash} "a"
+ 1:{hash} "b"
+ 1:{hash} "c"
"#
        );
        let ops = parse_batch_operations(&input).unwrap();

        execute_batch(&path, ops, false, OutputFormat::Text, None).await.unwrap();

        // Verify result: start, a, b, c (in that order)
        let result = std::fs::read_to_string(&path).unwrap();
        assert_eq!(result, "start\na\nb\nc\n");
    }

    #[tokio::test]
    async fn test_batch_delete_multiple_lines() {
        // Integration test: delete multiple lines without index shift issues
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.txt");

        // Create a 5-line file
        let lines: Vec<String> = ["1", "2", "3", "4", "5"].into_iter().map(String::from).collect();
        write_file(&path, &lines, true).unwrap();

        // Read file content and get hashes
        let content = std::fs::read_to_string(&path).unwrap();
        let file_lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
        let hash2 = crate::hash::hash_line(&file_lines[1]);
        let hash4 = crate::hash::hash_line(&file_lines[3]);

        // Delete lines 2 and 4
        let input = format!(
            r#"- 2:{hash2}
- 4:{hash4}
"#
        );
        let ops = parse_batch_operations(&input).unwrap();

        execute_batch(&path, ops, false, OutputFormat::Text, None).await.unwrap();

        // Verify result: 1, 3, 5
        let result = std::fs::read_to_string(&path).unwrap();
        assert_eq!(result, "1\n3\n5\n");
    }

    #[tokio::test]
    async fn test_batch_mixed_operations() {
        // Integration test: delete, replace, and insert in one batch
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.txt");

        // Create a file
        let lines: Vec<String> =
            ["L1", "L2", "L3", "L4", "L5"].into_iter().map(String::from).collect();
        write_file(&path, &lines, true).unwrap();

        // Read file content and get hashes
        let content = std::fs::read_to_string(&path).unwrap();
        let file_lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
        let hash1 = crate::hash::hash_line(&file_lines[0]);
        let hash2 = crate::hash::hash_line(&file_lines[1]);
        let hash3 = crate::hash::hash_line(&file_lines[2]);
        let _hash4 = crate::hash::hash_line(&file_lines[3]);

        // Delete L2, Replace L3 with NEW3, Insert A after L1
        let input = format!(
            r#"- 2:{hash2}
= 3:{hash3} "NEW3"
+ 1:{hash1} "A"
"#
        );
        let ops = parse_batch_operations(&input).unwrap();

        execute_batch(&path, ops, false, OutputFormat::Text, None).await.unwrap();

        // Verify result: L1, A, NEW3, L4, L5
        let result = std::fs::read_to_string(&path).unwrap();
        assert_eq!(result, "L1\nA\nNEW3\nL4\nL5\n");
    }

    #[tokio::test]
    async fn test_batch_delete_then_replace() {
        // Verify that "delete line 2 + replace line 3" works correctly
        // without hash mismatch due to line number offset
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.txt");

        // Create a 4-line file
        let lines: Vec<String> = ["L1", "L2", "L3", "L4"].into_iter().map(String::from).collect();
        write_file(&path, &lines, true).unwrap();

        // Read file content and get hashes
        let content = std::fs::read_to_string(&path).unwrap();
        let file_lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
        let hash2 = crate::hash::hash_line(&file_lines[1]);
        let hash3 = crate::hash::hash_line(&file_lines[2]);

        // Delete line 2, Replace line 3 (operations based on original line numbers)
        let input = format!(
            r#"- 2:{hash2}
= 3:{hash3} "NEW3"
"#
        );
        let ops = parse_batch_operations(&input).unwrap();

        execute_batch(&path, ops, false, OutputFormat::Text, None).await.unwrap();

        // Verify result: L1, NEW3, L4
        let result = std::fs::read_to_string(&path).unwrap();
        assert_eq!(result, "L1\nNEW3\nL4\n");
    }

    // ============================================================
    // Quote escaping tests - currently failing (documenting issues)
    // ============================================================

    #[test]
    fn test_parse_content_double_quote_inside_double() {
        // Double quote inside double-quoted content
        let input = r#"+ 10:AB "say \"hello\"""#;
        let ops = parse_batch_operations(input).unwrap();
        assert_eq!(ops[0].content, Some(r#"say "hello""#.to_string()));
    }

    #[test]
    fn test_parse_content_backslash() {
        // Backslash escaping
        let input = r#"+ 10:AB "path\\to\\file""#;
        let ops = parse_batch_operations(input).unwrap();
        assert_eq!(ops[0].content, Some(r#"path\to\file"#.to_string()));
    }

    #[test]
    fn test_parse_content_json_with_quotes() {
        // JSON string content
        let input = r#"+ 10:AB "{\"key\": \"value\"}""#;
        let ops = parse_batch_operations(input).unwrap();
        assert_eq!(ops[0].content, Some(r#"{"key": "value"}"#.to_string()));
    }

    #[test]
    fn test_parse_content_rust_code() {
        // Rust code containing raw string literal
        let input = r##"+ 10:AB "let s = r#\"hello\"#;""##;
        let ops = parse_batch_operations(input).unwrap();
        assert_eq!(ops[0].content, Some(r###"let s = r#"hello"#;"###.to_string()));
    }

    #[test]
    fn test_parse_content_newline_escape() {
        // Newline escape sequence
        let input = r#"+ 10:AB "line1\nline2""#;
        let ops = parse_batch_operations(input).unwrap();
        assert_eq!(ops[0].content, Some("line1\nline2".to_string()));
    }

    #[test]
    fn test_parse_content_tab_escape() {
        // Tab escape sequence
        let input = r#"+ 10:AB "col1\tcol2""#;
        let ops = parse_batch_operations(input).unwrap();
        assert_eq!(ops[0].content, Some("col1\tcol2".to_string()));
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
        assert_eq!(ops[0].content, Some("it's".to_string()));
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
    // HashlineRange delete tests
    // ============================================================

    #[tokio::test]
    async fn test_batch_range_delete() {
        // Delete lines 2-9 from a 10-line file, keeping L1 and L10
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.txt");

        // Create a 10-line file
        let lines: Vec<String> = (1..=10).map(|i| format!("L{}", i)).collect();
        write_file(&path, &lines, true).unwrap();

        // Read file content and get hashes
        let content = std::fs::read_to_string(&path).unwrap();
        let file_lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
        let hash2 = crate::hash::hash_line(&file_lines[1]); // line 2 (0-indexed)
        let hash9 = crate::hash::hash_line(&file_lines[8]); // line 9 (0-indexed)

        // Delete lines 2-9 using hashline range
        let input = format!("- [2:{hash2},9:{hash9}]");
        let ops = parse_batch_operations(&input).unwrap();

        execute_batch(&path, ops, false, OutputFormat::Text, None).await.unwrap();

        // Verify result: L1, L10
        let result = std::fs::read_to_string(&path).unwrap();
        assert_eq!(result, "L1\nL10\n");
    }

    #[tokio::test]
    async fn test_batch_range_delete_single_line() {
        // [3:hash3,3:hash3] - delete only one line
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.txt");

        let lines: Vec<String> = (1..=5).map(|i| format!("L{}", i)).collect();
        write_file(&path, &lines, true).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let file_lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
        let hash3 = crate::hash::hash_line(&file_lines[2]); // line 3 (0-indexed)

        let input = format!("- [3:{hash3},3:{hash3}]");
        let ops = parse_batch_operations(&input).unwrap();

        execute_batch(&path, ops, false, OutputFormat::Text, None).await.unwrap();

        // Verify result: L1, L2, L4, L5
        let result = std::fs::read_to_string(&path).unwrap();
        assert_eq!(result, "L1\nL2\nL4\nL5\n");
    }

    #[tokio::test]
    async fn test_batch_range_delete_hash_mismatch() {
        // Boundary line hash mismatch should fail
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.txt");

        let lines: Vec<String> = (1..=10).map(|i| format!("L{}", i)).collect();
        write_file(&path, &lines, true).unwrap();

        // Use wrong hash (VV is valid format but wrong value)
        let input = "- [2:VV,9:VV]";
        let ops = parse_batch_operations(input).unwrap();

        let result = execute_batch(&path, ops, false, OutputFormat::Text, None).await;

        // Should fail with hash mismatch error
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Hash mismatch") || err.contains("line"), "Error: {}", err);
    }
}
