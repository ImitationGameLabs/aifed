//! Diff application utilities
//!
//! This module provides functions to apply line diffs to file content.

use std::collections::HashSet;

use aifed_common::LineDiffDto;

/// Format diffs with git-style context lines (3 lines before and after changes)
///
/// Returns a string suitable for display after edit operations.
pub fn format_diffs_with_context(
    diffs: &[LineDiffDto],
    original_lines: &[String],
    context_lines: usize,
) -> String {
    if diffs.is_empty() {
        return "  (no changes)".to_string();
    }

    // Build a set of affected line numbers (1-based)
    let mut affected = HashSet::new();
    for diff in diffs {
        affected.insert(diff.line_num);
    }

    // Expand to include context
    let mut show_lines = HashSet::new();
    for &line_num in &affected {
        let start = line_num.saturating_sub(context_lines);
        let end = (line_num + context_lines).min(original_lines.len());
        for l in start..=end {
            if l >= 1 {
                show_lines.insert(l);
            }
        }
    }

    // Sort and format
    let mut sorted: Vec<_> = show_lines.iter().copied().collect();
    sorted.sort();

    let mut output = Vec::new();
    let mut prev: Option<usize> = None;

    for line_num in sorted {
        // Add separator if there's a gap
        if let Some(p) = prev
            && line_num > p + 1
        {
            output.push(String::new()); // Blank line between hunks
        }

        let content = original_lines.get(line_num - 1).map(|s| s.as_str()).unwrap_or("");

        if affected.contains(&line_num) {
            // Find the diff for this line
            if let Some(diff) = diffs.iter().find(|d| d.line_num == line_num) {
                match (&diff.old_content, &diff.new_content) {
                    (None, Some(new)) => {
                        output.push(format!("+{}|{}", line_num, new));
                    }
                    (Some(old), None) => {
                        output.push(format!("-{}|{}", line_num, old));
                    }
                    (Some(_old), Some(new)) => {
                        output.push(format!("-{}|{}", line_num, _old));
                        output.push(format!("+{}|{}", line_num, new));
                    }
                    (None, None) => {}
                }
            }
        } else {
            // Context line
            output.push(format!(" {}|{}", line_num, content));
        }

        prev = Some(line_num);
    }

    output.join("\n")
}

/// Print diffs in a readable format (without context, for undo/redo commands)
pub fn print_diffs(diffs: &[LineDiffDto]) {
    if diffs.is_empty() {
        println!("  (no changes)");
        return;
    }

    for diff in diffs {
        match (&diff.old_content, &diff.new_content) {
            (None, Some(new)) => {
                println!("  +{}: {}", diff.line_num, new);
            }
            (Some(old), None) => {
                println!("  -{}: {}", diff.line_num, old);
            }
            (Some(old), Some(new)) => {
                // Replacement: show as deletion + insertion
                println!("  -{}: {}", diff.line_num, old);
                println!("  +{}: {}", diff.line_num, new);
            }
            (None, None) => {}
        }
    }
}

/// Apply diffs to lines, modifying them in place.
///
/// The diffs represent the changes needed to transform the file content.
///
/// # Order of operations
/// 1. Deletions and replacements are applied in reverse line order (to avoid index shift)
/// 2. Insertions are applied in forward line order
pub fn apply_diffs(lines: &mut Vec<String>, diffs: &[LineDiffDto]) {
    // First pass: handle replacements and deletions (in reverse order by line number)
    let mut sorted_diffs: Vec<_> = diffs.iter().collect();
    sorted_diffs.sort_by(|a, b| b.line_num.cmp(&a.line_num));

    for diff in &sorted_diffs {
        match (&diff.old_content, &diff.new_content) {
            (Some(_), None) => {
                // Deletion: remove line at line_num (1-based)
                if diff.line_num > 0 && diff.line_num <= lines.len() {
                    lines.remove(diff.line_num - 1);
                }
            }
            (Some(_), Some(new)) => {
                // Replacement: update line content at line_num (1-based)
                if diff.line_num > 0 && diff.line_num <= lines.len() {
                    lines[diff.line_num - 1] = new.clone();
                }
            }
            _ => {}
        }
    }

    // Second pass: handle insertions (in forward order by line number)
    let mut insert_diffs: Vec<_> = diffs.iter().collect();
    insert_diffs.sort_by(|a, b| a.line_num.cmp(&b.line_num));

    for diff in &insert_diffs {
        if let (None, Some(new)) = (&diff.old_content, &diff.new_content) {
            // Insertion: insert at line_num (1-based means insert before this line)
            if diff.line_num == 0 {
                lines.insert(0, new.clone());
            } else if diff.line_num <= lines.len() + 1 {
                lines.insert(diff.line_num - 1, new.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_diff(line_num: usize, old: Option<&str>, new: Option<&str>) -> LineDiffDto {
        LineDiffDto {
            line_num,
            old_hash: None,
            old_content: old.map(|s| s.to_string()),
            new_content: new.map(|s| s.to_string()),
        }
    }

    #[test]
    fn test_apply_diffs_replacement() {
        // Replace line 2
        let mut lines = vec!["line1".to_string(), "line2".to_string(), "line3".to_string()];
        let diffs = vec![make_diff(2, Some("line2"), Some("modified"))];
        apply_diffs(&mut lines, &diffs);
        assert_eq!(lines, vec!["line1", "modified", "line3"]);
    }

    #[test]
    fn test_apply_diffs_insertion() {
        // Insert at line 3 (after line 2)
        let mut lines = vec!["line1".to_string(), "line2".to_string(), "line3".to_string()];
        let diffs = vec![make_diff(3, None, Some("inserted"))];
        apply_diffs(&mut lines, &diffs);
        assert_eq!(lines, vec!["line1", "line2", "inserted", "line3"]);
    }

    #[test]
    fn test_apply_diffs_insertion_at_start() {
        // Insert at line 1 (beginning of file)
        let mut lines = vec!["line1".to_string(), "line2".to_string()];
        let diffs = vec![make_diff(1, None, Some("new first"))];
        apply_diffs(&mut lines, &diffs);
        assert_eq!(lines, vec!["new first", "line1", "line2"]);
    }

    #[test]
    fn test_apply_diffs_deletion() {
        // Delete line 2
        let mut lines = vec!["line1".to_string(), "line2".to_string(), "line3".to_string()];
        let diffs = vec![make_diff(2, Some("line2"), None)];
        apply_diffs(&mut lines, &diffs);
        assert_eq!(lines, vec!["line1", "line3"]);
    }

    #[test]
    fn test_apply_diffs_multiple_replacements() {
        // Replace lines 1 and 3 (should work in any order due to reverse processing)
        let mut lines = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let diffs = vec![make_diff(1, Some("a"), Some("A")), make_diff(3, Some("c"), Some("C"))];
        apply_diffs(&mut lines, &diffs);
        assert_eq!(lines, vec!["A", "b", "C"]);
    }

    #[test]
    fn test_apply_diffs_multiple_deletions() {
        // Delete lines 1 and 3 (reverse order to avoid index shift)
        let mut lines = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let diffs = vec![make_diff(1, Some("a"), None), make_diff(3, Some("c"), None)];
        apply_diffs(&mut lines, &diffs);
        assert_eq!(lines, vec!["b"]);
    }

    #[test]
    fn test_apply_diffs_multiple_insertions() {
        // Insert multiple lines at different positions
        let mut lines = vec!["a".to_string(), "b".to_string()];
        let diffs = vec![make_diff(1, None, Some("before_a")), make_diff(3, None, Some("after_a"))];
        apply_diffs(&mut lines, &diffs);
        assert_eq!(lines, vec!["before_a", "a", "after_a", "b"]);
    }

    #[test]
    fn test_apply_diffs_mixed_operations() {
        // Replace line 2, delete line 4, insert at line 3
        let mut lines =
            vec!["L1".to_string(), "L2".to_string(), "L3".to_string(), "L4".to_string()];
        let diffs = vec![
            make_diff(2, Some("L2"), Some("MODIFIED")),
            make_diff(4, Some("L4"), None),
            make_diff(3, None, Some("INSERTED")),
        ];
        apply_diffs(&mut lines, &diffs);
        // After operations: L1, MODIFIED, INSERTED, L3 (L4 deleted)
        assert_eq!(lines, vec!["L1", "MODIFIED", "INSERTED", "L3"]);
    }

    #[test]
    fn test_apply_diffs_empty() {
        let mut lines = vec!["a".to_string(), "b".to_string()];
        let diffs: Vec<LineDiffDto> = vec![];
        apply_diffs(&mut lines, &diffs);
        assert_eq!(lines, vec!["a", "b"]);
    }

    #[test]
    fn test_apply_diffs_undo_insertion() {
        // Undo an insertion: delete the inserted line
        // File state after insertion: line1, line2, inserted, line3
        // Undo should result in: line1, line2, line3
        let mut lines = vec![
            "line1".to_string(),
            "line2".to_string(),
            "inserted".to_string(),
            "line3".to_string(),
        ];
        let diffs = vec![make_diff(3, Some("inserted"), None)]; // Deletion at line 3
        apply_diffs(&mut lines, &diffs);
        assert_eq!(lines, vec!["line1", "line2", "line3"]);
    }

    #[test]
    fn test_apply_diffs_undo_deletion() {
        // Undo a deletion: insert the deleted line back
        // File state after deletion: line1, line3
        // Undo should result in: line1, line2, line3
        let mut lines = vec!["line1".to_string(), "line3".to_string()];
        let diffs = vec![make_diff(2, None, Some("line2"))]; // Insertion at line 2
        apply_diffs(&mut lines, &diffs);
        assert_eq!(lines, vec!["line1", "line2", "line3"]);
    }

    #[test]
    fn test_apply_diffs_undo_replacement() {
        // Undo a replacement: restore the old content
        // File state after replacement: line1, modified, line3
        // Undo should result in: line1, line2, line3
        let mut lines = vec!["line1".to_string(), "modified".to_string(), "line3".to_string()];
        let diffs = vec![make_diff(2, Some("modified"), Some("line2"))]; // Replace with original
        apply_diffs(&mut lines, &diffs);
        assert_eq!(lines, vec!["line1", "line2", "line3"]);
    }

    // ============================================================
    // format_diffs_with_context tests
    // ============================================================

    #[test]
    fn test_format_diffs_with_context_empty() {
        let diffs: Vec<LineDiffDto> = vec![];
        let lines: Vec<String> = vec!["line1".to_string()];
        let result = format_diffs_with_context(&diffs, &lines, 3);
        assert_eq!(result, "  (no changes)");
    }

    #[test]
    fn test_format_diffs_with_context_replacement() {
        let diffs = vec![make_diff(3, Some("line3"), Some("modified"))];
        let lines: Vec<String> = (1..=5).map(|i| format!("line{}", i)).collect();
        let result = format_diffs_with_context(&diffs, &lines, 3);
        // Lines 1-5 with line 3 replaced
        // Expected: context (1,2), -3, +3, context (4,5)
        let expected =
            vec![" 1|line1", " 2|line2", "-3|line3", "+3|modified", " 4|line4", " 5|line5"];
        assert_eq!(result, expected.join("\n"));
    }

    #[test]
    fn test_format_diffs_with_context_insertion() {
        let diffs = vec![make_diff(3, None, Some("inserted"))];
        let lines: Vec<String> = (1..=5).map(|i| format!("line{}", i)).collect();
        let result = format_diffs_with_context(&diffs, &lines, 3);
        let expected = vec![" 1|line1", " 2|line2", "+3|inserted", " 4|line4", " 5|line5"];
        assert_eq!(result, expected.join("\n"));
    }

    #[test]
    fn test_format_diffs_with_context_deletion() {
        let diffs = vec![make_diff(3, Some("line3"), None)];
        let lines: Vec<String> = (1..=5).map(|i| format!("line{}", i)).collect();
        let result = format_diffs_with_context(&diffs, &lines, 3);
        let expected = vec![" 1|line1", " 2|line2", "-3|line3", " 4|line4", " 5|line5"];
        assert_eq!(result, expected.join("\n"));
    }

    #[test]
    fn test_format_diffs_with_context_near_boundary() {
        // Change at line 2, context should include line 1
        let diffs = vec![make_diff(2, Some("line2"), Some("modified"))];
        let lines: Vec<String> = (1..=5).map(|i| format!("line{}", i)).collect();
        let result = format_diffs_with_context(&diffs, &lines, 3);
        let expected =
            vec![" 1|line1", "-2|line2", "+2|modified", " 3|line3", " 4|line4", " 5|line5"];
        assert_eq!(result, expected.join("\n"));
    }

    #[test]
    fn test_format_diffs_with_context_overlapping() {
        // Two changes 5 lines apart (line 3 and line 8)
        // With 3 lines of context each:
        // - Line 3 context: lines 1-6
        // - Line 8 context: lines 5-11
        // They overlap at lines 5-6, should not duplicate
        let diffs = vec![
            make_diff(3, Some("line3"), Some("modified3")),
            make_diff(8, Some("line8"), Some("modified8")),
        ];
        let lines: Vec<String> = (1..=12).map(|i| format!("line{}", i)).collect();
        let result = format_diffs_with_context(&diffs, &lines, 3);

        // Expected: single continuous block with no duplicates
        let expected = vec![
            " 1|line1",
            " 2|line2",
            "-3|line3",
            "+3|modified3",
            " 4|line4",
            " 5|line5",
            " 6|line6",
            " 7|line7",
            "-8|line8",
            "+8|modified8",
            " 9|line9",
            " 10|line10",
            " 11|line11",
        ];
        assert_eq!(result, expected.join("\n"));
    }
}
