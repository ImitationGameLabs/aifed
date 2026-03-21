use std::io::{self, IsTerminal, Read};
use std::path::Path;

use crate::batch;
use crate::error::{Error, Result};
use crate::file::write_file;
use crate::hash::{hash_line, is_virtual_hash};
use crate::locator::Locator;
use crate::output::{EditChange, EditResult, OutputFormat, format_edit_result};

/// Edit operation types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Operation {
    Replace,
    Insert,
    Delete,
}

impl Operation {
    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "=" => Ok(Operation::Replace),
            "+" => Ok(Operation::Insert),
            "-" => Ok(Operation::Delete),
            _ => Err(Error::InvalidOperation { input: s.to_string() }),
        }
    }
}

/// Validated operation ready to apply
#[derive(Debug)]
pub struct ValidatedOp {
    pub operation: Operation,
    pub target_line: usize,
    pub old_content: Option<String>,
    pub new_content: Option<String>,
}

/// Validate an operation against the current file state
pub fn validate_operation(
    lines: &[String],
    operation: Operation,
    locator: &Locator,
    content: Option<&str>,
    path: &Path,
) -> Result<ValidatedOp> {
    // Validate content requirement
    match operation {
        Operation::Replace | Operation::Insert => {
            if content.is_none() {
                return Err(Error::InvalidLocator {
                    input: "".to_string(),
                    reason: "Content is required for replace and insert operations".to_string(),
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

        return Ok(ValidatedOp {
            operation,
            target_line: 0,
            old_content: None,
            new_content: content.map(|s| s.to_string()),
        });
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
                reason: "Hashline range only supported in batch mode".to_string(),
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

    Ok(ValidatedOp {
        operation,
        target_line,
        old_content: Some(actual_content),
        new_content: content.map(|s| s.to_string()),
    })
}

/// Apply a validated operation to lines
pub fn apply_operation(lines: &mut Vec<String>, validated: ValidatedOp) -> EditChange {
    match validated.operation {
        Operation::Replace => {
            let new_content = validated.new_content.clone().unwrap();
            let old_content = validated.old_content.clone().unwrap_or_default();
            lines[validated.target_line - 1] = new_content.clone();
            EditChange {
                operation: "replace".to_string(),
                line: validated.target_line,
                old_content: Some(old_content),
                new_content: Some(new_content),
            }
        }
        Operation::Insert => {
            let new_content = validated.new_content.clone().unwrap();
            if validated.target_line == 0 {
                // Insert at beginning
                lines.insert(0, new_content.clone());
                EditChange {
                    operation: "insert".to_string(),
                    line: 1,
                    old_content: None,
                    new_content: Some(new_content),
                }
            } else {
                // Insert after target line
                lines.insert(validated.target_line, new_content.clone());
                EditChange {
                    operation: "insert".to_string(),
                    line: validated.target_line + 1,
                    old_content: None,
                    new_content: Some(new_content),
                }
            }
        }
        Operation::Delete => {
            let old_content = lines.remove(validated.target_line - 1);
            EditChange {
                operation: "delete".to_string(),
                line: validated.target_line,
                old_content: Some(old_content),
                new_content: None,
            }
        }
    }
}

/// Execute the edit command
///
/// Supports both single-edit mode (when operation and locator are provided)
/// and batch mode (when stdin contains operations).
pub fn execute(
    path: &Path,
    operation_str: Option<&str>,
    locator_str: Option<&str>,
    content: Option<&str>,
    dry_run: bool,
    format: OutputFormat,
) -> Result<()> {
    if !path.exists() {
        return Err(Error::FileNotFound { path: path.to_path_buf() });
    }

    // Determine if we're in batch mode or single-edit mode
    if operation_str.is_none() && locator_str.is_none() {
        // Batch mode from stdin (if not a tty)
        if io::stdin().is_terminal() {
            return Err(Error::StdinNotAvailable);
        }
        let mut input = String::new();
        io::stdin().read_to_string(&mut input).map_err(|_| Error::StdinNotAvailable)?;
        let operations = batch::parse_batch_operations(&input)?;
        batch::execute_batch(path, operations, dry_run, format)
    } else {
        // Single-edit mode
        let operation_str =
            operation_str.ok_or_else(|| Error::InvalidOperation { input: "".to_string() })?;
        let locator_str = locator_str.ok_or_else(|| Error::InvalidLocator {
            input: "".to_string(),
            reason: "Locator is required".to_string(),
        })?;

        execute_single(path, operation_str, locator_str, content, dry_run, format)
    }
}

/// Execute a single edit operation
fn execute_single(
    path: &Path,
    operation_str: &str,
    locator_str: &str,
    content: Option<&str>,
    dry_run: bool,
    format: OutputFormat,
) -> Result<()> {
    let operation = Operation::parse(operation_str)?;
    let locator = Locator::parse(locator_str)?;

    let file_content = std::fs::read_to_string(path)
        .map_err(|e| Error::InvalidIo { path: path.to_path_buf(), source: e })?;

    let original_had_trailing_newline = file_content.ends_with('\n');
    let mut lines: Vec<String> = file_content.lines().map(|s| s.to_string()).collect();

    // Validate
    let validated = validate_operation(&lines, operation, &locator, content, path)?;

    // Apply
    let changes = vec![apply_operation(&mut lines, validated)];

    let result = EditResult {
        success: true,
        message: if dry_run {
            format!("Would apply {} to {}", operation_str, path.display())
        } else {
            format!("Applied {} to {}", operation_str, path.display())
        },
        changes: Some(changes),
    };

    if !dry_run {
        write_file(path, &lines, original_had_trailing_newline)?;
    }

    println!("{}", format_edit_result(&result, format));
    Ok(())
}
