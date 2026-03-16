use std::path::Path;

use crate::error::{Error, Result};
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

/// Execute the edit command
pub fn execute(
    path: &Path,
    operation_str: &str,
    locator_str: &str,
    content: Option<&str>,
    dry_run: bool,
    format: OutputFormat,
) -> Result<()> {
    if !path.exists() {
        return Err(Error::FileNotFound { path: path.to_path_buf() });
    }

    let operation = Operation::parse(operation_str)?;
    let locator = Locator::parse(locator_str)?;

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

    let file_content = std::fs::read_to_string(path)
        .map_err(|e| Error::IoError { path: path.to_path_buf(), source: e })?;

    let mut lines: Vec<String> = file_content.lines().map(|s| s.to_string()).collect();

    // Handle virtual line (insert at beginning)
    if locator.is_virtual() {
        if operation != Operation::Insert {
            return Err(Error::InvalidLocator {
                input: locator_str.to_string(),
                reason: "Virtual line (0:000000) is only valid for insert operations".to_string(),
            });
        }

        let new_content = content.unwrap().to_string();
        lines.insert(0, new_content.clone());

        let result = EditResult {
            success: true,
            message: if dry_run {
                format!("Would insert at beginning of {}", path.display())
            } else {
                format!("Inserted at beginning of {}", path.display())
            },
            changes: Some(vec![EditChange {
                operation: "insert".to_string(),
                line: 1,
                old_content: None,
                new_content: Some(new_content),
            }]),
        };

        if !dry_run {
            write_file(path, &lines)?;
        }

        println!("{}", format_edit_result(&result, format));
        return Ok(());
    }

    // Get line number and hash from locator
    let (target_line, expected_hash) = match &locator {
        Locator::Hashline { line, hash } => (*line, Some(hash.clone())),
        Locator::LineOnly(line) => (*line, None),
        Locator::Range { .. } => {
            return Err(Error::InvalidLocator {
                input: locator_str.to_string(),
                reason: "Range locators not supported for edit operations".to_string(),
            });
        }
    };

    // Validate line number
    if target_line == 0 || target_line > lines.len() {
        return Err(Error::InvalidLocator {
            input: locator_str.to_string(),
            reason: format!("Line {} out of range (1-{})", target_line, lines.len()),
        });
    }

    // Verify hash if provided
    let actual_content = lines[target_line - 1].clone();
    let actual_hash = hash_line(&actual_content);

    if let Some(expected) = &expected_hash {
        // Skip hash verification for virtual hash
        if !is_virtual_hash(expected) && actual_hash != *expected {
            return Err(Error::HashMismatch {
                path: path.to_path_buf(),
                line: target_line,
                expected: expected.clone(),
                actual: actual_hash,
                actual_content,
            });
        }
    }

    // Apply the operation
    let changes = match operation {
        Operation::Replace => {
            let new_content = content.unwrap().to_string();
            lines[target_line - 1] = new_content.clone();
            vec![EditChange {
                operation: "replace".to_string(),
                line: target_line,
                old_content: Some(actual_content),
                new_content: Some(new_content),
            }]
        }
        Operation::Insert => {
            let new_content = content.unwrap().to_string();
            lines.insert(target_line, new_content.clone());
            vec![EditChange {
                operation: "insert".to_string(),
                line: target_line + 1,
                old_content: None,
                new_content: Some(new_content),
            }]
        }
        Operation::Delete => {
            lines.remove(target_line - 1);
            vec![EditChange {
                operation: "delete".to_string(),
                line: target_line,
                old_content: Some(actual_content),
                new_content: None,
            }]
        }
    };

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
        write_file(path, &lines)?;
    }

    println!("{}", format_edit_result(&result, format));
    Ok(())
}

fn write_file(path: &Path, lines: &[String]) -> Result<()> {
    let content = lines.join("\n");
    std::fs::write(path, content)
        .map_err(|e| Error::IoError { path: path.to_path_buf(), source: e })?;
    Ok(())
}
