//! Character-level text editing for LSP operations.
//!
//! This module provides utilities for applying LSP TextEdit operations
//! to file content. LSP uses character-level ranges while aifed's native
//! editing is line-based.

use crate::error::{Error, Result};
use aifed_common::{Position, Range, TextEdit};

/// Convert LSP Position to byte offset in the string.
///
/// LSP positions are 0-indexed with `line` being the line number and
/// `character` being the UTF-16 code unit offset within that line.
///
/// Note: This implementation assumes ASCII content for simplicity.
/// For proper UTF-16 handling, use unicode-segmentation crate.
fn position_to_byte_offset(content: &str, pos: &Position) -> usize {
    let lines: Vec<&str> = content.lines().collect();
    let mut offset = 0;

    // Accumulate byte length of all preceding lines (including newline)
    for i in 0..pos.line as usize {
        offset += lines.get(i).map(|l| l.len() + 1).unwrap_or(0);
    }

    // Add character offset on current line
    let line = lines.get(pos.line as usize).unwrap_or(&"");
    offset += char_offset_to_byte_offset(line, pos.character as usize);

    offset
}

/// Convert character offset to byte offset within a line.
fn char_offset_to_byte_offset(line: &str, char_offset: usize) -> usize {
    line.char_indices().nth(char_offset).map(|(i, _)| i).unwrap_or(line.len())
}

/// Apply multiple TextEdits to file content.
///
/// Process:
/// 1. Validation phase: Check that each range's original content is valid
/// 2. Application phase: Apply all edits from bottom to top
///
/// Note: If the file is externally modified during application, our final
/// result will still overwrite it. This is acceptable because rename is
/// semantically an atomic operation - we validate first, then apply.
pub fn apply_edits(content: &str, mut edits: Vec<TextEdit>) -> Result<String> {
    // Phase 1: Validate all ranges
    // This ensures LSP-returned ranges are valid in the current file
    for edit in &edits {
        extract_range_content(content, &edit.range)?;
    }

    // Phase 2: Sort by range.start descending (apply from end of file)
    // This prevents earlier offsets from shifting due to later modifications
    edits.sort_by(|a, b| {
        b.range
            .start
            .line
            .cmp(&a.range.start.line)
            .then(b.range.start.character.cmp(&a.range.start.character))
    });

    let mut result = content.to_string();
    for edit in edits {
        let start = position_to_byte_offset(&result, &edit.range.start);
        let end = position_to_byte_offset(&result, &edit.range.end);
        result.replace_range(start..end, &edit.new_text);
    }

    Ok(result)
}

/// Extract content at the specified range (for validation).
fn extract_range_content(content: &str, range: &Range) -> Result<String> {
    let start = position_to_byte_offset(content, &range.start);
    let end = position_to_byte_offset(content, &range.end);

    if start > end {
        return Err(Error::InvalidRange {
            start: range.start,
            end: range.end,
            reason: "Start position is after end position".to_string(),
        });
    }

    // Validate that positions are within content bounds
    if end > content.len() {
        return Err(Error::InvalidRange {
            start: range.start,
            end: range.end,
            reason: format!("End position {} exceeds content length {}", end, content.len()),
        });
    }

    // Validate that character offsets are within their respective lines
    let lines: Vec<&str> = content.lines().collect();
    if let Some(line) = lines.get(range.start.line as usize) {
        let char_count = line.chars().count();
        if range.start.character as usize > char_count {
            return Err(Error::InvalidRange {
                start: range.start,
                end: range.end,
                reason: format!(
                    "Start character {} exceeds line {} length {}",
                    range.start.character, range.start.line, char_count
                ),
            });
        }
    }
    if let Some(line) = lines.get(range.end.line as usize) {
        let char_count = line.chars().count();
        if range.end.character as usize > char_count {
            return Err(Error::InvalidRange {
                start: range.start,
                end: range.end,
                reason: format!(
                    "End character {} exceeds line {} length {}",
                    range.end.character, range.end.line, char_count
                ),
            });
        }
    }

    Ok(content[start..end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_position(line: u32, character: u32) -> Position {
        Position { line, character }
    }

    fn make_range(start: Position, end: Position) -> Range {
        Range { start, end }
    }

    fn make_text_edit(range: Range, new_text: &str) -> TextEdit {
        TextEdit { range, new_text: new_text.to_string() }
    }

    #[test]
    fn test_apply_single_edit() {
        // Content: "let args = Args::parse_args();"
        //          Position 0,4 to 0,8 is "args"
        let content = "let args = Args::parse_args();";
        let edit = make_text_edit(make_range(make_position(0, 4), make_position(0, 8)), "cli_args");

        let result = apply_edits(content, vec![edit]).unwrap();
        assert_eq!(result, "let cli_args = Args::parse_args();");
    }

    #[test]
    fn test_apply_multiple_edits_same_line() {
        // Replace "foo" with "bar" and second "foo" with "baz"
        let content = "foo foo foo";
        let edits = vec![
            make_text_edit(make_range(make_position(0, 0), make_position(0, 3)), "bar"),
            make_text_edit(make_range(make_position(0, 4), make_position(0, 7)), "baz"),
        ];

        let result = apply_edits(content, edits).unwrap();
        assert_eq!(result, "bar baz foo");
    }

    #[test]
    fn test_apply_edits_multiline() {
        let content = "line1\nlet args = 1;\nline3";
        let edit = make_text_edit(make_range(make_position(1, 4), make_position(1, 8)), "foo");

        let result = apply_edits(content, vec![edit]).unwrap();
        assert_eq!(result, "line1\nlet foo = 1;\nline3");
    }

    #[test]
    fn test_invalid_range_out_of_bounds() {
        let content = "hello";
        let edit = make_text_edit(make_range(make_position(0, 0), make_position(0, 100)), "x");

        let result = apply_edits(content, vec![edit]);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_range_line_out_of_bounds() {
        let content = "hello";
        let edit = make_text_edit(make_range(make_position(5, 0), make_position(5, 5)), "x");

        let result = apply_edits(content, vec![edit]);
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_edits() {
        let content = "hello";
        let result = apply_edits(content, vec![]).unwrap();
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_position_to_byte_offset() {
        let content = "abc\ndef\nghi";

        // First line, character 0
        assert_eq!(position_to_byte_offset(content, &make_position(0, 0)), 0);

        // First line, character 2
        assert_eq!(position_to_byte_offset(content, &make_position(0, 2)), 2);

        // Second line, character 0 (after "abc\n")
        assert_eq!(position_to_byte_offset(content, &make_position(1, 0)), 4);

        // Second line, character 1
        assert_eq!(position_to_byte_offset(content, &make_position(1, 1)), 5);

        // Third line, character 0 (after "abc\ndef\n")
        assert_eq!(position_to_byte_offset(content, &make_position(2, 0)), 8);
    }

    #[test]
    fn test_extract_range_content() {
        let content = "hello world";

        let range = make_range(make_position(0, 0), make_position(0, 5));
        let extracted = extract_range_content(content, &range).unwrap();
        assert_eq!(extracted, "hello");

        let range = make_range(make_position(0, 6), make_position(0, 11));
        let extracted = extract_range_content(content, &range).unwrap();
        assert_eq!(extracted, "world");
    }

    #[test]
    fn test_edits_applied_in_reverse_order() {
        // Test that edits are applied from end to beginning
        // This prevents offset shifts from affecting subsequent edits
        let content = "aaa bbb ccc";
        let edits = vec![
            make_text_edit(make_range(make_position(0, 0), make_position(0, 3)), "111"),
            make_text_edit(make_range(make_position(0, 4), make_position(0, 7)), "222"),
            make_text_edit(make_range(make_position(0, 8), make_position(0, 11)), "333"),
        ];

        let result = apply_edits(content, edits).unwrap();
        assert_eq!(result, "111 222 333");
    }
}
