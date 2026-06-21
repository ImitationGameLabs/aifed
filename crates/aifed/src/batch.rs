//! Batch editing support for aifed.
//!
//! This module provides batch operation parsing and execution, allowing multiple
//! edits to be applied in a single invocation.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::file::write_file;

use crate::edit_view::{EditRow, new_lines_from_rows};
use crate::error::{Error, Result};
use crate::hash::{hash_file, hash_line, is_virtual_hash};
use crate::locator::Locator;
use crate::output::{BatchResult, OutputFormat, changes_from_rows, format_batch_result_with_diff};
use crate::scanner::{PeekResult, Scanner, Token};
use aifed_common::LineDiffDto;
use aifed_daemon_client::DaemonClient;

/// Edit operation types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Operation {
    Insert,
    Delete,
    Replace,
}

/// Validated operation ready to apply
#[derive(Debug)]
pub struct ValidatedOp {
    pub operation: Operation,
    pub target_line: usize,
    pub new_contents: Vec<String>,
}

/// Validate an operation against the current file state
#[allow(clippy::too_many_arguments)]
pub fn validate_operation(
    lines: &[String],
    operation: Operation,
    locator: &Locator,
    contents: &[String],
    path: &Path,
    indent: Option<i32>,
    resolved: &crate::indent::ResolvedIndent,
    assist_enabled: bool,
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

        let new_contents = apply_directive(contents, indent, None, resolved, assist_enabled)?;
        return Ok(ValidatedOp { operation, target_line: 0, new_contents });
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

    let new_contents = apply_directive(
        contents,
        indent,
        Some(&actual_content),
        resolved,
        assist_enabled,
    )?;
    Ok(ValidatedOp { operation, target_line, new_contents })
}

/// Apply an optional indent directive (`@N`) to content lines, returning the
/// transformed lines or a hard error. No directive -> verbatim. The anchor is
/// the line the edit is relative to (`Some` for insert/replace, `None` for the
/// virtual `0:00` line).
fn apply_directive(
    contents: &[String],
    indent: Option<i32>,
    anchor: Option<&str>,
    resolved: &crate::indent::ResolvedIndent,
    assist_enabled: bool,
) -> Result<Vec<String>> {
    use crate::indent::{IndentKind, UnknownReason, apply_indent, leading_indent};

    let Some(n) = indent else {
        return Ok(contents.to_vec());
    };

    let fail = |reason: String| Error::IndentDirective { reason };

    if !assist_enabled {
        return Err(fail(format!(
            "Indent directive @{n} cannot be applied: indent assist is disabled in config. Drop @{n} and provide exact indentation, or enable indent_assist."
        )));
    }

    let anchor_indent = anchor.map(leading_indent).unwrap_or("");

    if n == 0 {
        return Ok(contents
            .iter()
            .map(|c| apply_indent(c, anchor_indent, 0, &resolved.kind))
            .collect());
    }

    if anchor.is_none() {
        return Err(fail(format!(
            "Indent directive @{n} cannot be applied: virtual line 0:00 has no anchor to derive indentation levels from. Drop @{n} and provide exact indentation."
        )));
    }
    if resolved.config_conflict {
        return Err(fail(format!(
            "Indent directive @{n} cannot be applied: configured indent does not match the file. Fix the config or unify the file's indentation."
        )));
    }
    match &resolved.kind {
        IndentKind::Unknown(reason) => Err(fail(match reason {
            UnknownReason::Mixed => format!(
                "Indent directive @{n} cannot be applied: file mixes tabs and spaces. Unify the indentation convention (all tabs or all spaces), or drop @{n} and provide exact indentation."
            ),
            UnknownReason::Undeterminable => format!(
                "Indent directive @{n} cannot be applied: indent width could not be determined (insufficient or inconsistent indentation). Set indent_width in config, or drop @{n} and provide exact indentation."
            ),
        })),
        kind => Ok(contents
            .iter()
            .map(|c| apply_indent(c, anchor_indent, n, kind))
            .collect()),
    }
}

/// Parsed single operation from batch input
#[derive(Debug, Clone)]
pub struct EditOp {
    pub operation: Operation,
    pub locator: Locator,
    pub contents: Vec<String>,
    pub indent: Option<i32>,
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

    /// Apply the plan, producing a typed row sequence (see `crate::edit_view`) that is
    /// the single source of truth for the post-edit file (`new_lines_from_rows`) and
    /// the JSON change list (`changes_from_rows`). Each row carries its own correct
    /// coordinate(s), so display code never indexes a foreign array.
    fn apply(&self, original_lines: &[String]) -> Vec<EditRow> {
        let mut rows = Vec::new();
        let mut new_line = 0usize;

        // Inserts at the beginning of the file (virtual line 0).
        for content in &self.inserts_at_start {
            new_line += 1;
            rows.push(EditRow::insert(new_line, content));
        }

        // Walk the original file once, tracking both coordinates.
        for (i, original) in original_lines.iter().enumerate() {
            let old_line = i + 1; // 1-based original line number

            if self.deletions.contains(&old_line) {
                rows.push(EditRow::delete(old_line, original));
            } else {
                new_line += 1;
                rows.push(EditRow::equal(new_line, original));
            }

            // Inserts after this original line.
            if let Some(insert_contents) = self.inserts.get(&old_line) {
                for content in insert_contents {
                    new_line += 1;
                    rows.push(EditRow::insert(new_line, content));
                }
            }
        }

        rows
    }
}

/// Parse operations from string (heredoc/file content).
///
/// Uses a character-level [`Scanner`] so that newlines between tokens (e.g. OP
/// on one line, LOCATOR on the next) are treated as ordinary whitespace.
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
    let mut scanner = Scanner::new(input);
    let mut operations = Vec::new();

    loop {
        let op = match scanner.peek_token() {
            None => break,
            Some(PeekResult::Token(Token::Plus)) => {
                scanner.next_token();
                Operation::Insert
            }
            Some(PeekResult::Token(Token::Minus)) => {
                scanner.next_token();
                Operation::Delete
            }
            Some(PeekResult::Token(Token::Equals)) => {
                scanner.next_token();
                Operation::Replace
            }
            Some(PeekResult::Token(other)) => {
                return Err(invalid_op(
                    scanner.line(),
                    format!("{:?}", other),
                    "Expected operation: +, -, or =",
                ));
            }
            Some(PeekResult::Err(msg)) => {
                return Err(invalid_op(scanner.line(), String::new(), &msg));
            }
        };

        let locator = match op {
            Operation::Delete => parse_delete_locator(&mut scanner)?,
            Operation::Insert | Operation::Replace => {
                let raw = expect_locator_str(&mut scanner, "locator")?;
                Locator::parse(raw)
                    .map_err(|e| invalid_op(scanner.line(), raw.to_string(), &e.to_string()))?
            }
        };

        // Edit operations require hash verification.
        if let Locator::Line(_) | Locator::LineRange { .. } = &locator {
            return Err(invalid_op(
                scanner.line(),
                locator.to_string(),
                "Locator must include hash (e.g. \"42:AB\" or \"[2:AA,5:BB]\")",
            ));
        }

        // Optional indent directive @N (after the locator, before content).
        let indent = match scanner.peek_token() {
            Some(PeekResult::Token(Token::Indent(n))) => {
                scanner.next_token();
                Some(n)
            }
            _ => None,
        };
        // Indent directives apply to insert/replace content, not delete.
        if op == Operation::Delete && indent.is_some() {
            return Err(invalid_op(
                scanner.line(),
                locator.to_string(),
                "Indent directive is not valid for delete operations",
            ));
        }
        // At most one indent directive per operation.
        if matches!(
            scanner.peek_token(),
            Some(PeekResult::Token(Token::Indent(_)))
        ) {
            scanner.next_token();
            return Err(invalid_op(
                scanner.line(),
                locator.to_string(),
                "Only one indent directive (@N) is allowed per operation",
            ));
        }

        // Parse content tokens.
        let contents = collect_content_tokens(&mut scanner)?;

        // Validate content requirement.
        match op {
            Operation::Insert | Operation::Replace if contents.is_empty() => {
                return Err(invalid_op(
                    scanner.line(),
                    String::new(),
                    "Content is required for insert (+) and replace (=) operations",
                ));
            }
            Operation::Replace if contents.len() > 1 => {
                return Err(invalid_op(
                    scanner.line(),
                    String::new(),
                    "Replace (=) accepts only one content line. \
                     For multi-line replacement, use - then +:\n  \
                     - 42:AB\n  \
                     + 42:AB\n    \
                     \"line 1\"\n    \
                     \"line 2\"",
                ));
            }
            _ => {}
        }

        operations.push(EditOp { operation: op, locator, contents, indent });
    }

    Ok(operations)
}

// ── parser helpers ──────────────────────────────────────────────────

/// Parse the locator for a delete operation (single-line or range).
fn parse_delete_locator(scanner: &mut Scanner<'_>) -> Result<Locator> {
    match scanner.peek_token() {
        Some(PeekResult::Token(Token::RangeStart)) => {
            scanner.next_token(); // consume '['
            let start = expect_locator_str(scanner, "start of range")?;
            expect_comma(scanner)?;
            let end = expect_locator_str(scanner, "end of range")?;
            expect_range_end(scanner)?;
            let loc = format!("[{},{}]", start, end);
            Locator::parse(&loc).map_err(|e| invalid_op(scanner.line(), loc, &e.to_string()))
        }
        _ => {
            let raw = expect_locator_str(scanner, "locator")?;
            Locator::parse(raw)
                .map_err(|e| invalid_op(scanner.line(), raw.to_string(), &e.to_string()))
        }
    }
}

/// Expect the next token to be a locator-like token (Unquoted or Quoted)
/// and return its raw text.
fn expect_locator_str<'a>(scanner: &mut Scanner<'a>, context: &str) -> Result<&'a str> {
    match scanner.next_token() {
        Some(Ok(Token::Unquoted(s))) | Some(Ok(Token::Quoted(s))) => Ok(s),
        Some(Ok(other)) => Err(invalid_op(
            scanner.line(),
            format!("{:?}", other),
            &format!("Expected locator ({})", context),
        )),
        Some(Err(e)) => Err(convert_scanner_error(scanner.line(), &e)),
        None => Err(invalid_op(
            scanner.line(),
            String::new(),
            &format!("Expected locator ({})", context),
        )),
    }
}

fn expect_comma(scanner: &mut Scanner<'_>) -> Result<()> {
    match scanner.next_token() {
        Some(Ok(Token::Comma)) => Ok(()),
        Some(Ok(other)) => Err(invalid_op(
            scanner.line(),
            format!("{:?}", other),
            "Expected comma (,) in range",
        )),
        Some(Err(e)) => Err(convert_scanner_error(scanner.line(), &e)),
        None => Err(invalid_op(
            scanner.line(),
            String::new(),
            "Unexpected end of input in range",
        )),
    }
}

fn expect_range_end(scanner: &mut Scanner<'_>) -> Result<()> {
    match scanner.next_token() {
        Some(Ok(Token::RangeEnd)) => Ok(()),
        Some(Ok(other)) => Err(invalid_op(
            scanner.line(),
            format!("{:?}", other),
            "Expected closing bracket (])",
        )),
        Some(Err(e)) => Err(convert_scanner_error(scanner.line(), &e)),
        None => Err(invalid_op(
            scanner.line(),
            String::new(),
            "Unexpected end of input in range",
        )),
    }
}

/// Collect content tokens (Quoted / Unquoted) that follow the locator.
fn collect_content_tokens(scanner: &mut Scanner<'_>) -> Result<Vec<String>> {
    let mut tokens: Vec<Token<'_>> = Vec::new();
    loop {
        match scanner.peek_token() {
            Some(PeekResult::Token(Token::Quoted(_)))
            | Some(PeekResult::Token(Token::Unquoted(_))) => {
                tokens.push(scanner.next_token().unwrap().unwrap());
            }
            Some(PeekResult::Err(msg)) => {
                return Err(invalid_op(scanner.line(), String::new(), &msg));
            }
            _ => break,
        }
    }

    if tokens.is_empty() {
        return Ok(Vec::new());
    }
    parse_contents(&tokens)
}

fn invalid_op(line: usize, content: String, reason: &str) -> Error {
    Error::InvalidBatchOp { line_number: line, line_content: content, reason: reason.to_string() }
}

fn convert_scanner_error(line: usize, err: &Error) -> Error {
    match err {
        Error::Syntax { reason, .. } => Error::InvalidBatchOp {
            line_number: line,
            line_content: String::new(),
            reason: reason.clone(),
        },
        _ => Error::InvalidBatchOp {
            line_number: line,
            line_content: String::new(),
            reason: err.to_string(),
        },
    }
}

// ── content processing ──────────────────────────────────────────────

fn parse_contents(tokens: &[Token<'_>]) -> Result<Vec<String>> {
    if tokens.is_empty() {
        return Ok(Vec::new());
    }

    // Multiple Quoted tokens → each is a separate content line.
    if tokens.len() > 1 && tokens.iter().all(|t| matches!(t, Token::Quoted(_))) {
        return tokens
            .iter()
            .map(|t| decode_content(token_raw(t)))
            .collect();
    }

    // Single token (Quoted or not), or mixed — decode and join with space.
    let decoded: Result<Vec<_>> = tokens
        .iter()
        .map(|t| decode_content(token_raw(t)))
        .collect();
    let decoded = decoded?;
    let joined = decoded.join(" ");
    validate_content(&joined)?;
    Ok(vec![joined])
}

fn token_raw<'a>(t: &Token<'a>) -> &'a str {
    match t {
        Token::Quoted(s) | Token::Unquoted(s) => s,
        _ => unreachable!("unexpected token in content"),
    }
}

fn decode_content(raw: &str) -> Result<String> {
    let normalized = crate::escape::normalize_hex_escapes(raw);
    match json_escape::unescape(normalized.as_ref()).decode_utf8() {
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
    indent_settings: &crate::indent::IndentSettings,
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
        println!("{}", format_batch_result_with_diff(&result, format, &[]));
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
        indent_settings,
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
    indent_settings: &crate::indent::IndentSettings,
) -> Result<()> {
    // Phase 1: Validate all operations and build edit plan
    let resolved = crate::indent::resolve(lines, indent_settings);
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
            let validated = validate_operation(
                lines,
                op.operation,
                &op.locator,
                &op.contents,
                path,
                op.indent,
                &resolved,
                indent_settings.assist_enabled,
            )
            .map_err(|e| Error::InvalidBatchOp {
                line_number: idx + 1,
                line_content: op.to_string(),
                reason: e.to_string(),
            })?;

            plan.add(validated);
        }
    }

    // Phase 2: Apply the edit plan to build new content
    let rows = plan.apply(lines);
    let new_lines = new_lines_from_rows(&rows);
    let changes = changes_from_rows(&rows);

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

    println!("{}", format_batch_result_with_diff(&result, format, &rows));
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
        if let Some(n) = self.indent {
            write!(f, " @{n}")?;
        }
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
    use crate::indent::IndentSettings;
    use aifed_common::IndentStyleConfig;

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
        let rows = plan.apply(&original);
        let new_lines = new_lines_from_rows(&rows);

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
        let rows = plan.apply(&original);
        let new_lines = new_lines_from_rows(&rows);

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
        let rows = plan.apply(&original);
        let new_lines = new_lines_from_rows(&rows);

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
        let rows = plan.apply(&original);
        let new_lines = new_lines_from_rows(&rows);

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
        let rows = plan.apply(&original);
        let new_lines = new_lines_from_rows(&rows);

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
        let rows = plan.apply(&original);
        let new_lines = new_lines_from_rows(&rows);

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
        let rows = plan.apply(&original);
        let new_lines = new_lines_from_rows(&rows);

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

        execute_batch(
            &path,
            ops,
            false,
            OutputFormat::Text,
            None,
            &IndentSettings::detecting(),
        )
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

        execute_batch(
            &path,
            ops,
            false,
            OutputFormat::Text,
            None,
            &IndentSettings::detecting(),
        )
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

        execute_batch(
            &path,
            ops,
            false,
            OutputFormat::Text,
            None,
            &IndentSettings::detecting(),
        )
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

        execute_batch(
            &path,
            ops,
            false,
            OutputFormat::Text,
            None,
            &IndentSettings::detecting(),
        )
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

        execute_batch(
            &path,
            ops,
            false,
            OutputFormat::Text,
            None,
            &IndentSettings::detecting(),
        )
        .await
        .unwrap();

        let result = std::fs::read_to_string(&path).unwrap();
        assert_eq!(result, "L1\nREPLACED\nL3\n");
    }

    #[tokio::test]
    async fn test_batch_replace_multi_line_via_delete_insert() {
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

        let input = format!(
            r#"- 2:{hash2}
+ 2:{hash2} "A" "B""#
        );
        let ops = parse_batch_operations(&input).unwrap();

        execute_batch(
            &path,
            ops,
            false,
            OutputFormat::Text,
            None,
            &IndentSettings::detecting(),
        )
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
        // \x followed by a non-hex character (space) is not a valid \xNN escape
        // and should still produce an error.
        let input = r#"+ 10:AB "unknown \x escape""#;
        let result = parse_batch_operations(input);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_content_hex_escape_x1b() {
        // \x1b (ANSI escape byte) should be accepted and decoded
        let input = r#"+ 10:AB "\x1b[0m""#;
        let ops = parse_batch_operations(input).unwrap();
        assert_eq!(ops[0].contents, vec!["\x1b[0m"]);
    }

    #[test]
    fn test_parse_content_hex_escape_ascii() {
        // \x41 == 'A'
        let input = r#"+ 10:AB "\x41""#;
        let ops = parse_batch_operations(input).unwrap();
        assert_eq!(ops[0].contents, vec!["A"]);
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
    fn test_parse_replace_rejects_multi_content() {
        let input = r#"= 42:AB "line1" "line2""#;
        let result = parse_batch_operations(input);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("-") && err.contains("+"),
            "error should hint at - + syntax: {err}"
        );
    }

    #[test]
    fn test_parse_replace_missing_content() {
        let input = "= 42:AB";
        let result = parse_batch_operations(input);
        assert!(result.is_err());
    }

    // ============================================================
    // Multi-line operation tests (newlines between tokens)
    // ============================================================

    #[test]
    fn test_parse_multiline_op() {
        let input = "+\n42:AB\n\"content\"";
        let ops = parse_batch_operations(input).unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].operation, Operation::Insert);
        assert_eq!(ops[0].locator.to_string(), "42:AB");
        assert_eq!(ops[0].contents, vec!["content"]);
    }

    #[test]
    fn test_parse_multiline_replace() {
        let input = "=\n42:AB\n\"new\"";
        let ops = parse_batch_operations(input).unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].operation, Operation::Replace);
        assert_eq!(ops[0].contents, vec!["new"]);
    }

    #[test]
    fn test_parse_multiline_virtual_line() {
        let input = "+\n0:00\n\"first\"";
        let ops = parse_batch_operations(input).unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].operation, Operation::Insert);
        assert_eq!(ops[0].locator.to_string(), "0:00");
        assert_eq!(ops[0].contents, vec!["first"]);
    }

    #[test]
    fn test_parse_multiline_delete() {
        let input = "-\n42:AB";
        let ops = parse_batch_operations(input).unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].operation, Operation::Delete);
        assert_eq!(ops[0].locator.to_string(), "42:AB");
    }

    #[test]
    fn test_parse_multiline_range() {
        let input = "-\n[2:AA\n,\n5:BB]";
        let ops = parse_batch_operations(input).unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].operation, Operation::Delete);
        assert!(matches!(ops[0].locator, Locator::HashlineRange { .. }));
    }

    #[test]
    fn test_parse_multiline_mixed() {
        // Mix single-line and multi-line ops
        let input = "+ 1:AA \"x\"\n=\n2:BB\n\"y\"";
        let ops = parse_batch_operations(input).unwrap();
        assert_eq!(ops.len(), 2);
        assert_eq!(ops[0].operation, Operation::Insert);
        assert_eq!(ops[0].contents, vec!["x"]);
        assert_eq!(ops[1].operation, Operation::Replace);
        assert_eq!(ops[1].contents, vec!["y"]);
    }

    #[test]
    fn test_parse_multiline_with_comments() {
        let input = "+\n# note: line 42\n42:AB\n\"content\"";
        let ops = parse_batch_operations(input).unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].operation, Operation::Insert);
        assert_eq!(ops[0].contents, vec!["content"]);
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

        execute_batch(
            &path,
            ops,
            false,
            OutputFormat::Text,
            None,
            &IndentSettings::detecting(),
        )
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

        execute_batch(
            &path,
            ops,
            false,
            OutputFormat::Text,
            None,
            &IndentSettings::detecting(),
        )
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

        execute_batch(
            &path,
            ops,
            false,
            OutputFormat::Text,
            None,
            &IndentSettings::detecting(),
        )
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

        let result = execute_batch(
            &path,
            ops,
            false,
            OutputFormat::Text,
            None,
            &IndentSettings::detecting(),
        )
        .await;

        // Should fail with hash mismatch error
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("Hash mismatch") || err.contains("line"),
            "Error: {}",
            err
        );
    }

    #[tokio::test]
    async fn directive_at_zero_copies_anchor() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.txt");
        let lines: Vec<String> = ["    a", "        b", ""]
            .into_iter()
            .map(String::from)
            .collect();
        write_file(&path, &lines).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        let hash = crate::hash::hash_line(&crate::file::split_lines_owned(&content)[0]);
        let ops = parse_batch_operations(&format!("= 1:{hash} @0 REPLACED")).unwrap();
        execute_batch(
            &path,
            ops,
            false,
            OutputFormat::Text,
            None,
            &IndentSettings::detecting(),
        )
        .await
        .unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "    REPLACED\n        b\n"
        );
    }

    #[tokio::test]
    async fn directive_at_plus_one_tab() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.txt");
        let lines: Vec<String> = ["\ta", ""].into_iter().map(String::from).collect();
        write_file(&path, &lines).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        let hash = crate::hash::hash_line(&crate::file::split_lines_owned(&content)[0]);
        let ops = parse_batch_operations(&format!("+ 1:{hash} @+1 b")).unwrap();
        execute_batch(
            &path,
            ops,
            false,
            OutputFormat::Text,
            None,
            &IndentSettings::detecting(),
        )
        .await
        .unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "\ta\n\t\tb\n");
    }

    #[tokio::test]
    async fn directive_at_plus_one_unknown_errors_atomic() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.txt");
        // 4 and 6 spaces are not a common multiple -> Unknown -> @+1 hard-errors.
        let lines: Vec<String> = ["a", "    b", "      c", ""]
            .into_iter()
            .map(String::from)
            .collect();
        write_file(&path, &lines).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        let original = content.clone();
        let hash = crate::hash::hash_line(&crate::file::split_lines_owned(&content)[0]);
        let ops = parse_batch_operations(&format!("+ 1:{hash} @+1 d")).unwrap();
        let result = execute_batch(
            &path,
            ops,
            false,
            OutputFormat::Text,
            None,
            &IndentSettings::detecting(),
        )
        .await;
        assert!(
            result.is_err(),
            "@+1 on an inconsistent file must hard-error"
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
    }

    #[tokio::test]
    async fn directive_on_delete_errors() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.txt");
        let lines: Vec<String> = ["a", "b", ""].into_iter().map(String::from).collect();
        write_file(&path, &lines).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        let hash = crate::hash::hash_line(&crate::file::split_lines_owned(&content)[0]);
        let err = parse_batch_operations(&format!("- 1:{hash} @0")).unwrap_err();
        assert!(err.to_string().contains("not valid for delete"));
    }

    #[test]
    fn directive_rejects_second_directive() {
        let err = parse_batch_operations("+ 1:AA @0 @1 body").unwrap_err();
        assert!(err.to_string().contains("Only one indent directive"));
    }

    #[tokio::test]
    async fn directive_assist_disabled_rejects() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.txt");
        let lines: Vec<String> = ["    a", "        b", ""]
            .into_iter()
            .map(String::from)
            .collect();
        write_file(&path, &lines).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        let hash = crate::hash::hash_line(&crate::file::split_lines_owned(&content)[0]);
        let ops = parse_batch_operations(&format!("= 1:{hash} @0 c")).unwrap();
        let disabled =
            IndentSettings { assist_enabled: false, forced_style: None, forced_width: None };
        let result = execute_batch(&path, ops, false, OutputFormat::Text, None, &disabled).await;
        let err = result.unwrap_err().to_string();
        assert!(err.contains("assist is disabled"), "got: {err}");
    }

    #[tokio::test]
    async fn directive_on_virtual_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.txt");
        let lines: Vec<String> = ["    a", "        b", ""]
            .into_iter()
            .map(String::from)
            .collect();
        write_file(&path, &lines).unwrap();
        // @0 on the virtual line succeeds, inserting at column 0.
        let ops = parse_batch_operations("+ 0:00 @0 TOP").unwrap();
        execute_batch(
            &path,
            ops,
            false,
            OutputFormat::Text,
            None,
            &IndentSettings::detecting(),
        )
        .await
        .unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "TOP\n    a\n        b\n"
        );
        // @+1 on the virtual line has no anchor -> hard error.
        let ops = parse_batch_operations("+ 0:00 @+1 x").unwrap();
        let result = execute_batch(
            &path,
            ops,
            false,
            OutputFormat::Text,
            None,
            &IndentSettings::detecting(),
        )
        .await;
        assert!(result.is_err(), "@+1 on virtual line must hard-error");
    }

    #[tokio::test]
    async fn directive_forced_space_config_consistent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.txt");
        let lines: Vec<String> = ["fn() {", "    x", "}", ""]
            .into_iter()
            .map(String::from)
            .collect();
        write_file(&path, &lines).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        let hash = crate::hash::hash_line(&crate::file::split_lines_owned(&content)[1]);
        // One distinct space count -> undetectable; only the forced Space{4} resolves,
        // so this guards the forced-config path (detection would error instead).
        let forced = IndentSettings {
            assist_enabled: true,
            forced_style: Some(IndentStyleConfig::Space),
            forced_width: Some(4),
        };
        let ops = parse_batch_operations(&format!("+ 2:{hash} @+1 y")).unwrap();
        execute_batch(&path, ops, false, OutputFormat::Text, None, &forced)
            .await
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "fn() {\n    x\n        y\n}\n"
        );
    }

    #[tokio::test]
    async fn directive_forced_space_config_conflict_atomic() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.txt");
        let lines: Vec<String> = ["fn() {", "\tx", "}"]
            .into_iter()
            .map(String::from)
            .collect();
        write_file(&path, &lines).unwrap();
        let original = std::fs::read_to_string(&path).unwrap();
        let hash = crate::hash::hash_line(&crate::file::split_lines_owned(&original)[1]);
        // Forced config is an assertion: a tab-indented file conflicts with forced
        // Space -> hard-error, and the file is left untouched (atomic).
        let forced = IndentSettings {
            assist_enabled: true,
            forced_style: Some(IndentStyleConfig::Space),
            forced_width: Some(4),
        };
        let ops = parse_batch_operations(&format!("+ 2:{hash} @+1 y")).unwrap();
        let result = execute_batch(&path, ops, false, OutputFormat::Text, None, &forced).await;
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("configured indent does not match"),
            "got: {err}"
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
    }

    #[tokio::test]
    async fn directive_applies_to_every_payload() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.txt");
        let lines: Vec<String> = ["\ta", ""].into_iter().map(String::from).collect();
        write_file(&path, &lines).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        let hash = crate::hash::hash_line(&crate::file::split_lines_owned(&content)[0]);
        let ops = parse_batch_operations(&format!("+ 1:{hash} @+1 \"b\" \"c\"")).unwrap();
        execute_batch(
            &path,
            ops,
            false,
            OutputFormat::Text,
            None,
            &IndentSettings::detecting(),
        )
        .await
        .unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "\ta\n\t\tb\n\t\tc\n"
        );
    }
}
